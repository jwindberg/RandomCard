//! A connection's values, and signing requests with them. Each user's
//! values are saved in their copy of the stack, beside the author's
//! (`model::connections`), as an address book is filled in by whoever has
//! it. Secrets are never in the stack: they go in the system's secure store
//! (the Keychain on a Mac, Credential Manager on Windows, the Secret Service
//! on Linux), named by the stack, the connection and the setting. Where
//! there's none, a secret lasts only until RandomCard quits: it's never
//! written to a file.

use std::collections::BTreeMap;
use std::sync::{LazyLock, Mutex, MutexGuard};

use crate::model::Stack;
use crate::model::connections::{BEARER, Connection, Field, Given, HEADER, HEADERS, KEY_NAME, QUERY, SEND_AS, TYPE, Type, URL};
use crate::web::http::Credentials;

/// The name RandomCard's secrets go under in the secure store.
const SERVICE: &str = "RandomCard";

/// The secrets of every stack's connections.
pub struct Store {
    /// Use the system's secure store (not in tests).
    keychain: bool,
    /// Secrets kept until RandomCard quits, where there's no secure store.
    session: BTreeMap<String, String>,
    /// What the secure store said for each secret, read once a session:
    /// a Mac asks the user before each read RandomCard isn't trusted for,
    /// and every request under a connection needs its secrets. What the
    /// user refused stays refused until the secret is set again.
    read: BTreeMap<String, Result<Option<String>, String>>,
    /// How a secret is read from the secure store (counted in tests).
    fetch: fn(&str) -> Result<Option<String>, String>,
}

/// The secrets, through the system's secure store.
pub fn store() -> MutexGuard<'static, Store> {
    static STORE: LazyLock<Mutex<Store>> = LazyLock::new(|| {
        // Tests never touch the user's own secure store.
        Mutex::new(Store { keychain: !cfg!(test), session: BTreeMap::new(), read: BTreeMap::new(), fetch: keychain_read })
    });
    STORE.lock().unwrap_or_else(|e| e.into_inner())
}

/// A key ignoring case, as HyperTalk names are.
fn key(s: &str) -> String {
    s.trim().to_lowercase()
}

/// The value this copy's user gave for an ordinary element, if any.
pub fn given(conn: &Connection, field: &Field) -> String {
    conn.values.get(&key(&field.name)).cloned().unwrap_or_default()
}

/// An ordinary element's value: the author's when it's fixed; else the
/// user's, or the author's default, or a choice's first. A type left open
/// has none until the user picks one. A URL always comes with its scheme.
pub fn value(conn: &Connection, field: &Field) -> String {
    let v = match field.given {
        Given::Fixed => field.value.clone(),
        _ => Some(given(conn, field)).filter(|v| !v.is_empty()).unwrap_or_else(|| {
            if field.given == Given::Open && field.name == TYPE { String::new() } else { field.fallback().to_string() }
        }),
    };
    if field.name == URL { with_scheme(&v) } else { v }
}

/// The type a connection has, if it has one yet.
pub fn kind(conn: &Connection) -> Option<Type> {
    conn.find(TYPE).and_then(|f| Type::parse(&value(conn, &f)))
}

/// Give an element a value: an ordinary one in the connection, so the stack
/// changes; a secret in the secure store. An empty value forgets it. A
/// choice takes only its choices; a fixed element takes nothing. Returns
/// whether the stack changed.
pub fn set(store: &mut Store, stack: &str, conn: &mut Connection, field: &Field, value: &str) -> Result<bool, String> {
    if field.given == Given::Fixed {
        return Err(format!("Setting \"{}\" of connection \"{}\" is fixed by the stack", field.name, conn.name));
    }
    if field.secret {
        return store.set_secret(stack, conn, field, value).map(|()| false);
    }
    let mut value = value.to_string();
    if !value.is_empty() && !field.choices.is_empty() {
        match field.choices.iter().find(|c| c.eq_ignore_ascii_case(value.trim())) {
            Some(c) => value = c.to_string(),
            None => {
                return Err(format!(
                    "Setting \"{}\" of connection \"{}\" is one of {}",
                    field.name,
                    conn.name,
                    field.choices.join(", ")
                ));
            }
        }
    }
    if field.name == HEADERS {
        value = headers_text(&value)?;
    }
    let k = key(&field.name);
    Ok(if value.is_empty() {
        conn.values.remove(&k).is_some()
    } else {
        conn.values.insert(k, value.clone()).as_deref() != Some(value.as_str())
    })
}

/// A connection's Headers as lines of `Name: value`, or why not: each name
/// a token (RFC 9110), and none that signs in, which is the type's job.
fn header_lines(text: &str) -> Result<Vec<(String, String)>, String> {
    let mut out = Vec::new();
    for line in text.lines().map(str::trim).filter(|l| !l.is_empty()) {
        let Some((name, value)) = line.split_once(':') else {
            return Err(format!("A header is Name: value, not \"{line}\""));
        };
        let name = name.trim();
        let token = |c: char| c.is_ascii_alphanumeric() || "!#$%&'*+-.^_`|~".contains(c);
        if name.is_empty() || !name.chars().all(token) {
            return Err(format!("\"{name}\" isn't a header's name"));
        }
        if name.eq_ignore_ascii_case("Authorization") {
            return Err("Authorization is the type's to send, not a header of the connection's".into());
        }
        dynamic(value.trim())?;
        out.push((name.to_string(), value.trim().to_string()));
    }
    Ok(out)
}

/// Headers as they're kept: a line each, `Name: value`.
fn headers_text(text: &str) -> Result<String, String> {
    Ok(header_lines(text)?.iter().map(|(n, v)| format!("{n}: {v}")).collect::<Vec<_>>().join("\n"))
}

/// A header's value with its dynamic values filled in, afresh for each
/// request, named as Postman names them: `{{$guid}}` and `{{$randomUUID}}`
/// (a random UUID, RFC 9562 version 4), `{{$timestamp}}` (Unix time in
/// seconds) and `{{$randomInt}}` (0 to 1000).
pub fn dynamic(value: &str) -> Result<String, String> {
    let mut out = String::new();
    let mut rest = value;
    while let Some(open) = rest.find("{{$") {
        let Some(close) = rest[open..].find("}}").map(|c| open + c) else { break };
        let name = &rest[open + 3..close];
        out.push_str(&rest[..open]);
        match name {
            "guid" | "randomUUID" => out.push_str(&uuid_v4()),
            "timestamp" => out.push_str(
                &std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs()).to_string(),
            ),
            "randomInt" => out.push_str(&rand::random_range(0..=1000u32).to_string()),
            _ => {
                return Err(format!(
                    "{{{{${name}}}}} isn't a dynamic value: there's {{{{$guid}}}}, {{{{$randomUUID}}}}, {{{{$timestamp}}}} and {{{{$randomInt}}}}"
                ));
            }
        }
        rest = &rest[close + 2..];
    }
    out.push_str(rest);
    Ok(out)
}

/// A random UUID, version 4 (RFC 9562), in lower case.
fn uuid_v4() -> String {
    let mut b: [u8; 16] = rand::random();
    b[6] = (b[6] & 0x0f) | 0x40;
    b[8] = (b[8] & 0x3f) | 0x80;
    let h: String = b.iter().map(|x| format!("{x:02x}")).collect();
    format!("{}-{}-{}-{}-{}", &h[0..8], &h[8..12], &h[12..16], &h[16..20], &h[20..32])
}

impl Store {
    /// A store with no secure store, for tests.
    #[cfg(test)]
    pub fn in_memory() -> Store {
        Store { keychain: false, session: BTreeMap::new(), read: BTreeMap::new(), fetch: keychain_read }
    }

    /// Whether an element has a value: for a secret, whether one is kept.
    pub fn is_set(&mut self, stack: &str, conn: &Connection, field: &Field) -> bool {
        if field.secret {
            matches!(self.secret(stack, conn, field), Ok(Some(_)))
        } else {
            !value(conn, field).trim().is_empty()
        }
    }

    /// The elements of a connection the user still has to give.
    pub fn missing(&mut self, stack: &str, conn: &Connection) -> Vec<String> {
        let Some(t) = kind(conn) else {
            return vec![TYPE.to_string()];
        };
        let fields = conn.fields(t);
        let mut out: Vec<String> =
            fields.iter().filter(|f| !f.optional && !self.is_set(stack, conn, f)).map(|f| f.name.clone()).collect();
        // A key sent as a header or a query parameter needs its name.
        if t == Type::Key
            && let (Some(send), Some(name)) = (conn.find(SEND_AS), conn.find(KEY_NAME))
            && !value(conn, &send).eq_ignore_ascii_case(BEARER)
            && !self.is_set(stack, conn, &name)
        {
            out.push(name.name);
        }
        out
    }

    /// Where a secret is kept: the stack, the connection and the setting,
    /// by name, as anyone looking in the secure store would know them.
    fn secret_name(stack: &str, conn: &Connection, field: &Field) -> String {
        format!("{} / {} / {}", stack.trim(), conn.name.trim(), field.name.trim())
    }

    /// A secret, for RandomCard's own use (adding it to a request); never
    /// for scripts.
    pub fn secret(&mut self, stack: &str, conn: &Connection, field: &Field) -> Result<Option<String>, String> {
        let name = Self::secret_name(stack, conn, field);
        if let Some(v) = self.session.get(&name) {
            return Ok(Some(v.clone()));
        }
        if !self.keychain {
            return Ok(None);
        }
        let fetch = self.fetch;
        self.read.entry(name).or_insert_with_key(|name| fetch(name)).clone()
    }

    fn set_secret(&mut self, stack: &str, conn: &Connection, field: &Field, value: &str) -> Result<(), String> {
        let name = Self::secret_name(stack, conn, field);
        self.session.remove(&name);
        // Read afresh next time: the new value, or why it couldn't be saved.
        self.read.remove(&name);
        if self.keychain {
            match keychain_entry(&name) {
                Ok(entry) => {
                    let done = if value.is_empty() {
                        match entry.delete_credential() {
                            Err(keyring_core::Error::NoEntry) => Ok(()),
                            other => other,
                        }
                    } else {
                        entry.set_password(value)
                    };
                    return done.map_err(|e| format!("Can't save to the {}: {e}", secure_store_name()));
                }
                // No secure store: keep it for this session only, and say so.
                Err(why) if !value.is_empty() => {
                    self.session.insert(name, value.to_string());
                    return Err(format!("{why}, so \"{}\" is kept only until RandomCard quits", field.name));
                }
                Err(_) => return Ok(()),
            }
        }
        if !value.is_empty() {
            self.session.insert(name, value.to_string());
        }
        Ok(())
    }

    /// An element's value for a request: a secret from the secure store.
    fn for_request(&mut self, stack: &str, conn: &Connection, name: &str) -> Result<String, SignError> {
        let Some(f) = conn.find(name) else {
            return Err(SignError::Refused(format!("Connection \"{}\" has no setting \"{name}\"", conn.name)));
        };
        if f.secret {
            Ok(self.secret(stack, conn, &f).map_err(SignError::Refused)?.unwrap_or_default())
        } else {
            Ok(value(conn, &f))
        }
    }
}

/// What the system's secure store is called, for messages.
fn secure_store_name() -> &'static str {
    if cfg!(target_os = "macos") {
        "Keychain"
    } else if cfg!(windows) {
        "Credential Manager"
    } else {
        "secure store"
    }
}

/// The system's secure store, set up the first time a secret is used.
static KEYCHAIN: LazyLock<Result<(), String>> = LazyLock::new(|| {
    #[cfg(target_os = "macos")]
    let store = apple_native_keyring_store::keychain::Store::new();
    #[cfg(windows)]
    let store = windows_native_keyring_store::Store::new();
    #[cfg(target_os = "linux")]
    let store = zbus_secret_service_keyring_store::Store::new();
    #[cfg(any(target_os = "macos", windows, target_os = "linux"))]
    {
        match store {
            Ok(s) => {
                keyring_core::set_default_store(s);
                Ok(())
            }
            Err(e) => Err(format!("There's no secure store to keep secrets in ({e})")),
        }
    }
    #[cfg(not(any(target_os = "macos", windows, target_os = "linux")))]
    Err("This system has no secure store RandomCard can keep secrets in".to_string())
});

/// A secret from the system's secure store, or None when there's none.
fn keychain_read(name: &str) -> Result<Option<String>, String> {
    match keychain_entry(name)?.get_password() {
        Ok(v) => Ok(Some(v)),
        Err(keyring_core::Error::NoEntry) => Ok(None),
        Err(e) => Err(format!("Can't read from the {}: {e}", secure_store_name())),
    }
}

fn keychain_entry(name: &str) -> Result<keyring_core::Entry, String> {
    KEYCHAIN.clone()?;
    keyring_core::Entry::new(SERVICE, name).map_err(|e| format!("There's no secure store to keep secrets in ({e})"))
}

/// A URL as typed, with its scheme: https, as browsers take an address
/// with none, but http for this computer (localhost, 127.0.0.1, [::1]),
/// which W3C's Secure Contexts counts as secure already, and where a server
/// being written seldom has a certificate.
pub fn with_scheme(address: &str) -> String {
    let a = address.trim();
    if a.is_empty() || a.contains("://") {
        return a.to_string();
    }
    let host = a.split(['/', '?', '#']).next().unwrap_or("");
    let host = match host.strip_prefix('[') {
        Some(v6) => v6.split(']').next().unwrap_or(""),
        None => host.rsplit_once(':').map_or(host, |(h, _)| h),
    }
    .to_ascii_lowercase();
    let local = host == "localhost"
        || host.ends_with(".localhost")
        || host.parse::<std::net::IpAddr>().is_ok_and(|ip| ip.is_loopback());
    format!("{}://{a}", if local { "http" } else { "https" })
}

/// A stack's connection by name, or why not.
pub fn declared<'a>(stack: &'a Stack, name: &str) -> Result<&'a Connection, String> {
    stack.connection(name).ok_or_else(|| format!("This stack has no connection \"{}\"", name.trim()))
}

/// A connection's element by name, or why not.
fn declared_field(conn: &Connection, name: &str) -> Result<Field, String> {
    conn.find(name).ok_or_else(|| format!("Connection \"{}\" has no setting \"{}\"", conn.name, name.trim()))
}

/// `setting "URL" of connection "VideoDB"`: the user's value, or the
/// author's. Secrets are never given to scripts.
pub fn read(stack: &Stack, conn: &str, setting: &str) -> Result<String, String> {
    let conn = declared(stack, conn)?;
    let f = declared_field(conn, setting)?;
    if f.secret {
        return Err(format!("Setting \"{}\" of connection \"{}\" is secret: scripts can set it but not read it", f.name, conn.name));
    }
    Ok(value(conn, &f))
}

/// `set setting "URL" of connection "VideoDB" to x`: saved in this copy of
/// the stack, or a secret in the secure store. Returns whether the stack
/// changed.
pub fn write(store: &mut Store, stack: &mut Stack, conn: &str, setting: &str, value: &str) -> Result<bool, String> {
    let name = stack.name.clone();
    let f = declared_field(declared(stack, conn)?, setting)?;
    let Some(c) = stack.connection_mut(conn) else { return Ok(false) };
    set(store, &name, c, &f, value)
}

/// `create connection "videodb-basic"`: a new connection in the stack, its
/// type a default of None and its URL open, for a script to set up.
pub fn create(stack: &mut Stack, name: &str) -> Result<(), String> {
    let name = name.trim();
    if name.is_empty() {
        return Err("A connection needs a name".into());
    }
    if stack.connection(name).is_some() {
        return Err(format!("This stack already has a connection \"{name}\""));
    }
    stack.connections.push(Connection::new(name));
    Ok(())
}

/// `delete connection "videodb-basic"`: gone from the stack, and its
/// secrets from the secure store, so none is left behind under its name.
pub fn delete(store: &mut Store, stack: &mut Stack, name: &str) -> Result<(), String> {
    let conn = declared(stack, name)?.clone();
    let mut notes = Vec::new();
    let secrets: Vec<Field> = Type::ALL.iter().flat_map(|t| conn.fields(*t)).filter(|f| f.secret).collect();
    for f in &secrets {
        if let Err(why) = store.set_secret(&stack.name, &conn, f, "") {
            notes.push(why);
        }
    }
    stack.connections.retain(|c| !c.name.eq_ignore_ascii_case(name.trim()));
    if notes.is_empty() { Ok(()) } else { Err(notes.join("; ")) }
}

/// A web address made with a connection: a path (`/graphql`) is joined to
/// its URL, and a full address is used as it is.
pub fn url(store: &mut Store, stack: &Stack, conn: &str, url: &str) -> Result<String, String> {
    let c = declared(stack, conn)?;
    let missing = store.missing(&stack.name, c);
    if !missing.is_empty() {
        return Err(format!("Connection \"{}\" isn't set up: it needs {}", c.name, missing.join(", ")));
    }
    let url = url.trim();
    if super::movies::is_url(url) {
        return Ok(url.to_string());
    }
    let base = value(c, &declared_field(c, URL)?);
    let base = base.trim_end_matches('/');
    Ok(if url.is_empty() {
        base.to_string()
    } else if url.starts_with('?') || url.starts_with('#') {
        format!("{base}{url}")
    } else {
        format!("{base}/{}", url.trim_start_matches('/'))
    })
}

/// Why a request can't carry a connection's credentials.
#[derive(Debug, PartialEq)]
pub enum SignError {
    /// The connection still needs values: the user can be asked.
    NotSetUp { connection: String, why: String },
    /// Never sent this way, such as a password over plain http.
    Refused(String),
}

impl SignError {
    pub fn why(self) -> String {
        match self {
            SignError::NotSetUp { why, .. } | SignError::Refused(why) => why,
        }
    }
}

/// An address on this computer or the local network, where plain http
/// can carry credentials: loopback, `.local` (mDNS) and `.localhost`
/// names, and the private and link-local ranges (RFC 1918, RFC 4193).
fn local_host(host: &url::Host<&str>) -> bool {
    match host {
        url::Host::Domain(d) => {
            let d = d.to_ascii_lowercase();
            d == "localhost" || d.ends_with(".localhost") || d.ends_with(".local")
        }
        url::Host::Ipv4(ip) => ip.is_loopback() || ip.is_private() || ip.is_link_local(),
        url::Host::Ipv6(ip) => ip.is_loopback() || ip.is_unique_local() || ip.is_unicast_link_local(),
    }
}

/// Whether a request's address is under a connection's URL: the same
/// scheme, host and port, and a path at or below the URL's. Gives how
/// specific the match is (the URL's path length), so the closest of
/// several wins.
fn under(base: &url::Url, request: &url::Url) -> Option<usize> {
    if base.scheme() != request.scheme()
        || base.host_str().map(str::to_ascii_lowercase) != request.host_str().map(str::to_ascii_lowercase)
        || base.port_or_known_default() != request.port_or_known_default()
    {
        return None;
    }
    let base = base.path().trim_end_matches('/');
    let path = request.path();
    (base.is_empty() || path == base || path.starts_with(&format!("{base}/"))).then_some(base.len())
}

/// What a request to `url` carries: the headers of the stack's connection
/// whose URL it's under, and what its type adds to sign in. None when no
/// connection's URL covers it, or the connection adds nothing. They go only
/// under that URL, and a sign-in never over plain http beyond this computer
/// and the local network.
///
/// A request that names its connection (`with connection "videodb-basic"`)
/// is signed by that one when it's under its URL, so several connections
/// can share a server; otherwise the closest URL's connection signs it.
pub fn sign(store: &mut Store, stack: &Stack, url: &str, named: Option<&str>) -> Result<Option<Credentials>, SignError> {
    let Ok(request) = url::Url::parse(url.trim()) else { return Ok(None) };
    let name = &stack.name;
    let named = named.and_then(|n| stack.connection(n));
    let mut best: Option<(usize, &Connection)> = None;
    // The named connection first: when it covers the request, it signs it.
    for c in named.into_iter().chain(&stack.connections) {
        let Some(base) = c.find(URL).map(|f| value(c, &f)) else { continue };
        let Ok(base) = url::Url::parse(&base) else { continue };
        if let Some(n) = under(&base, &request)
            && best.is_none_or(|(m, _)| n > m)
        {
            best = Some((n, c));
            if named.is_some_and(|n| std::ptr::eq(n, c)) {
                break;
            }
        }
    }
    let Some((_, conn)) = best else { return Ok(None) };
    let kind = kind(conn);
    let own = conn.find(HEADERS).map(|f| value(conn, &f)).unwrap_or_default();
    if kind == Some(Type::None) && conn.headers.is_empty() && own.trim().is_empty() {
        return Ok(None);
    }
    let missing = store.missing(name, conn);
    if !missing.is_empty() {
        return Err(SignError::NotSetUp {
            connection: conn.name.clone(),
            why: format!("Connection \"{}\" isn't set up: it needs {}", conn.name, missing.join(", ")),
        });
    }
    let kind = kind.unwrap_or_default();
    if kind != Type::None && request.scheme() == "http" && !request.host().is_some_and(|h| local_host(&h)) {
        return Err(SignError::Refused(format!(
            "Connection \"{}\" won't sign in over plain http to {}: its URL needs https",
            conn.name,
            request.host_str().unwrap_or("")
        )));
    }
    let mut out = Credentials::default();
    for h in &conn.headers {
        let value = if h.element.trim().is_empty() {
            h.value.clone()
        } else {
            if conn.find(&h.element).is_some_and(|f| f.secret) {
                return Err(SignError::Refused(format!(
                    "Connection \"{}\" won't send its secret \"{}\" as a header of its own",
                    conn.name,
                    h.element.trim()
                )));
            }
            store.for_request(name, conn, &h.element)?
        };
        out.headers.push((h.name.trim().to_string(), dynamic(&value).map_err(SignError::Refused)?));
    }
    for (n, v) in header_lines(&own).map_err(SignError::Refused)? {
        out.headers.push((n, dynamic(&v).map_err(SignError::Refused)?));
    }
    match kind {
        Type::None => {}
        Type::Key => {
            let k = store.for_request(name, conn, "Key")?;
            let key_name = store.for_request(name, conn, KEY_NAME)?.trim().to_string();
            let send = store.for_request(name, conn, SEND_AS)?;
            if send.eq_ignore_ascii_case(HEADER) {
                out.headers.push((key_name, k));
            } else if send.eq_ignore_ascii_case(QUERY) {
                out.query.push((key_name, k));
            } else {
                out.headers.push(("Authorization".into(), format!("Bearer {k}")));
            }
        }
        Type::Basic => {
            use base64::Engine;
            let pair = format!("{}:{}", store.for_request(name, conn, "User name")?, store.for_request(name, conn, "Password")?);
            let encoded = base64::engine::general_purpose::STANDARD.encode(pair.as_bytes());
            out.headers.push(("Authorization".into(), format!("Basic {encoded}")));
        }
        Type::OAuth => {
            return Err(SignError::Refused(format!(
                "Connection \"{}\" signs in with OAuth, which RandomCard can't do yet",
                conn.name
            )));
        }
    }
    Ok(Some(out))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::connections::{Element, Header};

    /// A server of the user's own: its type and URL open, a usual URL as
    /// the default.
    fn stack() -> Stack {
        let mut s = Stack::new("Movies");
        let mut c = Connection::new("VideoDB");
        c.declare(TYPE, Given::Open, "");
        c.declare(URL, Given::Default, "http://media.local:9090");
        s.connections.push(c);
        s
    }

    #[test]
    fn values_and_defaults() {
        let (mut st, mut s) = (Store::in_memory(), stack());
        assert_eq!(read(&s, "videodb", "url").unwrap(), "http://media.local:9090");
        assert_eq!(read(&s, "VideoDB", "Type").unwrap(), "");
        assert_eq!(st.missing(&s.name, &s.connections[0]), ["Type"]);
        write(&mut st, &mut s, "VideoDB", "type", "basic").unwrap();
        assert_eq!(read(&s, "VideoDB", "type").unwrap(), "Basic");
        assert!(write(&mut st, &mut s, "VideoDB", "Type", "Kerberos").unwrap_err().contains("one of None, Key, Basic, OAuth"));
        write(&mut st, &mut s, "VideoDB", "User name", "ada").unwrap();
        assert_eq!(read(&s, "VideoDB", "user name").unwrap(), "ada");
        // Another stack declares its own connections.
        let other = Stack::new("Other");
        assert!(read(&other, "VideoDB", "URL").unwrap_err().contains("no connection"));
        assert!(read(&s, "VideoDB", "Color").unwrap_err().contains("no setting \"Color\""));
    }

    #[test]
    fn fixed_is_fixed() {
        let mut s = stack();
        s.connections[0].declare(TYPE, Given::Fixed, "Key");
        s.connections[0].declare(URL, Given::Fixed, "https://api.example.com");
        let mut st = Store::in_memory();
        assert!(write(&mut st, &mut s, "VideoDB", "URL", "https://evil.example.com").unwrap_err().contains("fixed by the stack"));
        assert_eq!(read(&s, "VideoDB", "URL").unwrap(), "https://api.example.com");
        // A fixed Key has no user name.
        assert!(read(&s, "VideoDB", "User name").is_err());
        assert_eq!(st.missing(&s.name, &s.connections[0]), ["Key"]);
    }

    #[test]
    fn secrets_are_kept_but_not_read() {
        let (mut st, mut s) = (Store::in_memory(), stack());
        write(&mut st, &mut s, "VideoDB", "Type", "Basic").unwrap();
        assert_eq!(st.missing(&s.name, &s.connections[0]), ["User name", "Password"]);
        // A secret doesn't change the stack: it's in the secure store.
        assert!(!write(&mut st, &mut s, "VideoDB", "Password", "hunter2").unwrap());
        assert!(!s.connections[0].values.contains_key("password"));
        assert!(read(&s, "VideoDB", "Password").unwrap_err().contains("secret"));
        let c = &s.connections[0];
        let p = c.find("password").unwrap();
        assert_eq!(st.secret(&s.name, c, &p).unwrap().as_deref(), Some("hunter2"));
        assert_eq!(st.missing(&s.name, c), ["User name"]);
        // Named plainly: the stack, the connection and the setting.
        assert_eq!(Store::secret_name(&s.name, c, &p), "Movies / VideoDB / Password");
        write(&mut st, &mut s, "VideoDB", "Password", "").unwrap();
        assert_eq!(st.secret(&s.name, &s.connections[0], &p).unwrap(), None);
    }

    /// Ordinary values are saved in this copy of the stack.
    #[test]
    fn values_are_in_the_stack() {
        let (mut st, mut s) = (Store::in_memory(), stack());
        assert!(write(&mut st, &mut s, "VideoDB", "User name", "ada").unwrap());
        assert!(!write(&mut st, &mut s, "VideoDB", "User name", "ada").unwrap());
        let saved = serde_json::to_string(&s.connections[0]).unwrap();
        assert!(saved.contains(r#""values":{"user name":"ada"}"#), "{saved}");
        assert!(write(&mut st, &mut s, "VideoDB", "User name", "").unwrap());
        assert!(!serde_json::to_string(&s.connections[0]).unwrap().contains("values"));
    }

    /// A secret is read from the secure store once, however many requests
    /// need it (a Mac may ask the user each time), until it's set again.
    #[test]
    fn secrets_are_read_once() {
        use std::sync::atomic::{AtomicUsize, Ordering};
        static READS: AtomicUsize = AtomicUsize::new(0);
        fn counted(_: &str) -> Result<Option<String>, String> {
            READS.fetch_add(1, Ordering::SeqCst);
            Ok(Some("hunter2".into()))
        }
        let s = stack();
        let c = &s.connections[0];
        let p = c.find("Password").unwrap();
        let mut st = Store { keychain: true, fetch: counted, ..Store::in_memory() };
        for _ in 0..5 {
            assert_eq!(st.secret(&s.name, c, &p).unwrap().as_deref(), Some("hunter2"));
            assert!(st.is_set(&s.name, c, &p));
        }
        assert_eq!(READS.load(Ordering::SeqCst), 1);
        // A refusal stays refused, without asking again, until the secret
        // is set (which forgets what was read).
        st.read.insert(Store::secret_name(&s.name, c, &p), Err("refused".into()));
        assert!(st.secret(&s.name, c, &p).is_err());
        st.read.clear();
        assert!(st.secret(&s.name, c, &p).is_ok());
        assert_eq!(READS.load(Ordering::SeqCst), 2);
    }

    /// The system's real secure store: saves, reads and forgets one secret.
    /// Run by hand (`--ignored`), as it needs a secure store running.
    #[test]
    #[ignore]
    fn the_system_keychain() {
        let s = stack();
        let c = &s.connections[0];
        let p = c.find("Password").unwrap();
        let mut st = Store { keychain: true, ..Store::in_memory() };
        let mut c = c.clone();
        set(&mut st, "Movies", &mut c, &p, "keychain test").unwrap();
        assert_eq!(st.secret("Movies", &c, &p).unwrap().as_deref(), Some("keychain test"));
        set(&mut st, "Movies", &mut c, &p, "").unwrap();
        assert_eq!(st.secret("Movies", &c, &p).unwrap(), None);
    }

    #[test]
    fn each_type_signs_its_own_way() {
        let mut s = Stack::new("Signed");
        let mut c = Connection::new("API");
        c.declare(URL, Given::Default, "https://api.example.com/v2/");
        c.headers.push(Header { name: "Accept".into(), value: "application/json".into(), element: String::new() });
        c.headers.push(Header { name: "X-Tenant".into(), element: "Tenant".into(), ..Header::default() });
        c.elements.push(Element { name: "Tenant".into(), value: "acme".into(), given: Given::Default, ..Element::default() });
        s.connections.push(c);
        let mut st = Store::in_memory();
        let at = "https://api.example.com/v2/movies";
        let headers = |c: &Credentials| c.headers.iter().map(|(n, v)| format!("{n}: {v}")).collect::<Vec<_>>();
        // None: only the headers.
        let c = sign(&mut st, &s, at, None).unwrap().unwrap();
        assert_eq!(headers(&c), ["Accept: application/json", "X-Tenant: acme"]);
        // Basic.
        write(&mut st, &mut s, "API", "Type", "Basic").unwrap();
        let e = sign(&mut st, &s, at, None).unwrap_err();
        assert_eq!(e, SignError::NotSetUp {
            connection: "API".into(),
            why: "Connection \"API\" isn't set up: it needs User name, Password".into()
        });
        write(&mut st, &mut s, "API", "User name", "ada").unwrap();
        write(&mut st, &mut s, "API", "Password", "pw:1").unwrap();
        let c = sign(&mut st, &s, "https://API.example.com:443/v2/movies?page=1", None).unwrap().unwrap();
        assert_eq!(headers(&c)[2], "Authorization: Basic YWRhOnB3OjE=");
        // Key, as Bearer, then a header and a query parameter by name.
        write(&mut st, &mut s, "API", "Type", "Key").unwrap();
        write(&mut st, &mut s, "API", "Key", "k&1").unwrap();
        assert_eq!(headers(&sign(&mut st, &s, at, None).unwrap().unwrap())[2], "Authorization: Bearer k&1");
        write(&mut st, &mut s, "API", "Send as", "header").unwrap();
        assert!(matches!(sign(&mut st, &s, at, None), Err(SignError::NotSetUp { why, .. }) if why.ends_with("needs Key name")));
        write(&mut st, &mut s, "API", "Key name", "X-API-Key").unwrap();
        assert_eq!(headers(&sign(&mut st, &s, at, None).unwrap().unwrap())[2], "X-API-Key: k&1");
        write(&mut st, &mut s, "API", "Send as", "Query").unwrap();
        write(&mut st, &mut s, "API", "Key name", "key").unwrap();
        assert_eq!(sign(&mut st, &s, at, None).unwrap().unwrap().query, [("key".to_string(), "k&1".to_string())]);
        // OAuth: declared, not yet signed in.
        write(&mut st, &mut s, "API", "Type", "OAuth").unwrap();
        for (e, v) in [("Authorization URL", "https://a.example.com"), ("Token URL", "https://a.example.com/t"), ("Client ID", "x")] {
            write(&mut st, &mut s, "API", e, v).unwrap();
        }
        assert!(matches!(sign(&mut st, &s, at, None), Err(SignError::Refused(w)) if w.contains("OAuth")));
    }

    /// Two connections to one server: a request that names one is signed
    /// by it.
    #[test]
    fn the_named_connection_signs() {
        let mut s = Stack::new("VideoDB");
        for (name, t) in [("videodb-open", "None"), ("videodb-basic", "Basic")] {
            let mut c = Connection::new(name);
            c.declare(TYPE, Given::Fixed, t);
            c.declare(URL, Given::Default, "http://localhost:8080");
            s.connections.push(c);
        }
        let mut st = Store::in_memory();
        write(&mut st, &mut s, "videodb-basic", "User name", "ada").unwrap();
        write(&mut st, &mut s, "videodb-basic", "Password", "pw").unwrap();
        let at = "http://localhost:8080/graphql";
        assert_eq!(sign(&mut st, &s, at, None), Ok(None));
        assert_eq!(sign(&mut st, &s, at, Some("videodb-open")), Ok(None));
        let c = sign(&mut st, &s, at, Some("VIDEODB-BASIC")).unwrap().unwrap();
        assert_eq!(c.headers[0].1, "Basic YWRhOnB3");
        // Named, but not under its URL: the usual rule.
        assert_eq!(sign(&mut st, &s, "http://localhost:9090/x", Some("videodb-basic")), Ok(None));
    }

    /// Headers of the connection's own, with dynamic values made afresh.
    #[test]
    fn headers_and_dynamic_values() {
        let (mut st, mut s) = (Store::in_memory(), stack());
        write(&mut st, &mut s, "VideoDB", "Type", "None").unwrap();
        let at = "http://media.local:9090/graphql";
        assert_eq!(sign(&mut st, &s, at, None), Ok(None));
        write(&mut st, &mut s, "VideoDB", "Headers", "x-client-app:RandomCard\n\n  x-request-id: {{$guid}}  ").unwrap();
        assert_eq!(read(&s, "VideoDB", "headers").unwrap(), "x-client-app: RandomCard\nx-request-id: {{$guid}}");
        let one = sign(&mut st, &s, at, None).unwrap().unwrap().headers;
        let two = sign(&mut st, &s, at, None).unwrap().unwrap().headers;
        assert_eq!(one[0], ("x-client-app".to_string(), "RandomCard".to_string()));
        let id = &one[1].1;
        assert_eq!((id.len(), &id[14..15], id.matches('-').count()), (36, "4", 4), "{id}");
        assert_ne!(one[1], two[1]);
        // Refused when set: not Name: value, a bad name, Authorization, an
        // unknown dynamic value.
        for (bad, why) in [
            ("no colon", "Name: value"),
            ("bad name: x", "isn't a header's name"),
            ("Authorization: Bearer x", "the type's"),
            ("X-Id: {{$uuid}}", "isn't a dynamic value"),
        ] {
            assert!(write(&mut st, &mut s, "VideoDB", "Headers", bad).unwrap_err().contains(why), "{bad}");
        }
        let n = dynamic("t={{$timestamp}} n={{$randomInt}} {{$randomUUID}} {{plain}}").unwrap();
        assert!(n.starts_with("t=1") && n.ends_with(" {{plain}}"), "{n}");
    }

    #[test]
    fn credentials_go_only_under_their_url() {
        let mut s = Stack::new("Signed");
        let mut c = Connection::new("API");
        c.declare(TYPE, Given::Fixed, "Basic");
        c.declare(URL, Given::Default, "https://api.example.com/v2/");
        s.connections.push(c);
        let mut st = Store::in_memory();
        write(&mut st, &mut s, "API", "User name", "ada").unwrap();
        write(&mut st, &mut s, "API", "Password", "pw").unwrap();
        assert!(sign(&mut st, &s, "https://api.example.com/v2", None).unwrap().is_some());
        // Anywhere else: another host, port, scheme or path gets nothing.
        for other in [
            "https://evil.example.com/v2/movies",
            "https://api.example.com.evil.net/v2",
            "https://api.example.com:8443/v2",
            "https://api.example.com/v20",
            "https://api.example.com/",
            "http://api.example.com/v2",
            "/v2/movies",
        ] {
            assert_eq!(sign(&mut st, &s, other, None), Ok(None), "{other}");
        }
        // Plain http signs in only on this computer and the local network.
        write(&mut st, &mut s, "API", "URL", "http://videos.example.com").unwrap();
        assert!(matches!(sign(&mut st, &s, "http://videos.example.com/graphql", None), Err(SignError::Refused(w)) if w.contains("plain http")));
        for local in ["http://localhost:8080", "http://127.0.0.1", "http://media.local:9090", "http://192.168.1.44", "http://[::1]:80"] {
            write(&mut st, &mut s, "API", "URL", local).unwrap();
            assert!(sign(&mut st, &s, &format!("{local}/graphql"), None).unwrap().is_some(), "{local}");
        }
    }

    #[test]
    fn addresses_from_a_connection() {
        let (mut st, mut s) = (Store::in_memory(), stack());
        assert!(url(&mut st, &s, "VideoDB", "/graphql").unwrap_err().contains("isn't set up: it needs Type"));
        write(&mut st, &mut s, "VideoDB", "Type", "None").unwrap();
        assert_eq!(url(&mut st, &s, "VideoDB", "/graphql").unwrap(), "http://media.local:9090/graphql");
        assert_eq!(url(&mut st, &s, "VideoDB", "videos/1").unwrap(), "http://media.local:9090/videos/1");
        assert_eq!(url(&mut st, &s, "VideoDB", "?page=2").unwrap(), "http://media.local:9090?page=2");
        assert_eq!(url(&mut st, &s, "VideoDB", "https://x.org/a").unwrap(), "https://x.org/a");
        write(&mut st, &mut s, "VideoDB", "URL", "videodb.example.com/").unwrap();
        assert_eq!(url(&mut st, &s, "VideoDB", "/graphql").unwrap(), "https://videodb.example.com/graphql");
        assert_eq!(read(&s, "VideoDB", "url").unwrap(), "https://videodb.example.com/");
        for (typed, full) in [
            ("localhost:8080", "http://localhost:8080"),
            ("127.0.0.1/api", "http://127.0.0.1/api"),
            ("[::1]:9000", "http://[::1]:9000"),
            ("app.localhost", "http://app.localhost"),
            ("10.0.0.5:9090", "https://10.0.0.5:9090"),
            ("localhost.example.com", "https://localhost.example.com"),
            ("ftp://x", "ftp://x"),
            ("", ""),
        ] {
            assert_eq!(with_scheme(typed), full);
        }
    }
}
