//! Each user's half of a stack's connections: the values they give for its
//! settings. A stack declares its connections (`model::connections`); the
//! values are never in the stack. Ordinary ones are kept in
//! `connections.json` in RandomCard's settings folder, under the stack's
//! identity and the connection's name. Secrets go in the system's secure
//! store: the Keychain on a Mac, Credential Manager on Windows, the Secret
//! Service on Linux. Where there's none, a secret lasts only until
//! RandomCard quits: it's never written to a plain file.

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::{LazyLock, Mutex, MutexGuard};

use crate::model::Stack;
use crate::model::connections::{ADDRESS, Connection, Place, Setting};
use crate::web::http::Credentials;

/// Values by stack identity, then connection, then setting.
type Values = BTreeMap<String, BTreeMap<String, BTreeMap<String, String>>>;

/// The name RandomCard's secrets go under in the secure store.
const SERVICE: &str = "RandomCard";

pub struct Store {
    /// Where ordinary values are saved; None keeps them in memory (tests).
    file: Option<PathBuf>,
    values: Values,
    /// Use the system's secure store for secrets (not in tests).
    keychain: bool,
    /// Secrets kept until RandomCard quits, where there's no secure store.
    session: BTreeMap<String, String>,
}

/// The user's values, loaded the first time they're needed.
pub fn store() -> MutexGuard<'static, Store> {
    static STORE: LazyLock<Mutex<Store>> = LazyLock::new(|| {
        // Tests never touch the user's own settings or secure store.
        let file = super::settings::config_dir().map(|d| d.join("connections.json")).filter(|_| !cfg!(test));
        let values = file.as_ref().and_then(|f| std::fs::read_to_string(f).ok()).and_then(|t| serde_json::from_str(&t).ok());
        Mutex::new(Store { file, values: values.unwrap_or_default(), keychain: !cfg!(test), session: BTreeMap::new() })
    });
    STORE.lock().unwrap_or_else(|e| e.into_inner())
}

/// A key ignoring case, as HyperTalk names are.
fn key(s: &str) -> String {
    s.trim().to_lowercase()
}

impl Store {
    /// A store with nothing saved and no secure store, for tests.
    #[cfg(test)]
    pub fn in_memory() -> Store {
        Store { file: None, values: Values::new(), keychain: false, session: BTreeMap::new() }
    }

    /// A user's value for an ordinary setting, or the setting's default.
    /// An Address always comes with its scheme.
    fn value(&self, uid: &str, conn: &Connection, setting: &Setting) -> String {
        let v = self
            .values
            .get(uid)
            .and_then(|c| c.get(&key(&conn.name)))
            .and_then(|s| s.get(&key(&setting.name)))
            .cloned()
            .unwrap_or_else(|| setting.default.clone());
        if setting.name.eq_ignore_ascii_case(ADDRESS) { with_scheme(&v) } else { v }
    }

    /// The value the user gave for an ordinary setting, if any: what the
    /// setup dialog shows, with the default only as a hint.
    pub fn given(&self, uid: &str, conn: &Connection, setting: &Setting) -> String {
        self.values
            .get(uid)
            .and_then(|c| c.get(&key(&conn.name)))
            .and_then(|s| s.get(&key(&setting.name)))
            .cloned()
            .unwrap_or_default()
    }

    /// Whether the user has given a value: for a secret, whether one is kept.
    pub fn is_set(&mut self, uid: &str, conn: &Connection, setting: &Setting) -> bool {
        if setting.secret {
            matches!(self.secret(uid, conn, setting), Ok(Some(_)))
        } else {
            !self.value(uid, conn, setting).trim().is_empty()
        }
    }

    /// The settings of a connection the user still has to give.
    pub fn missing(&mut self, uid: &str, conn: &Connection) -> Vec<String> {
        conn.settings.iter().filter(|s| !s.optional && !self.is_set(uid, conn, s)).map(|s| s.name.clone()).collect()
    }

    /// Save a user's value: an ordinary one to `connections.json`, a secret
    /// to the secure store. An empty value forgets it.
    pub fn set(&mut self, uid: &str, conn: &Connection, setting: &Setting, value: &str) -> Result<(), String> {
        if setting.secret {
            return self.set_secret(uid, conn, setting, value);
        }
        let settings = self.values.entry(uid.to_string()).or_default().entry(key(&conn.name)).or_default();
        if value.is_empty() {
            settings.remove(&key(&setting.name));
        } else {
            settings.insert(key(&setting.name), value.to_string());
        }
        self.save()
    }

    fn save(&self) -> Result<(), String> {
        let Some(file) = &self.file else { return Ok(()) };
        if let Some(dir) = file.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        let json = serde_json::to_string_pretty(&self.values).map_err(|e| e.to_string())?;
        std::fs::write(file, json).map_err(|e| format!("Can't save the connection settings: {e}"))
    }

    /// Where a secret is kept: the stack, the connection and the setting.
    fn secret_name(uid: &str, conn: &Connection, setting: &Setting) -> String {
        format!("{uid}/{}/{}", key(&conn.name), key(&setting.name))
    }

    /// A secret, for RandomCard's own use (adding it to a request); never
    /// for scripts.
    pub fn secret(&mut self, uid: &str, conn: &Connection, setting: &Setting) -> Result<Option<String>, String> {
        let name = Self::secret_name(uid, conn, setting);
        if let Some(v) = self.session.get(&name) {
            return Ok(Some(v.clone()));
        }
        if !self.keychain {
            return Ok(None);
        }
        let entry = keychain_entry(&name)?;
        match entry.get_password() {
            Ok(v) => Ok(Some(v)),
            Err(keyring_core::Error::NoEntry) => Ok(None),
            Err(e) => Err(format!("Can't read from the {}: {e}", secure_store_name())),
        }
    }

    fn set_secret(&mut self, uid: &str, conn: &Connection, setting: &Setting, value: &str) -> Result<(), String> {
        let name = Self::secret_name(uid, conn, setting);
        self.session.remove(&name);
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
                    return Err(format!("{why}, so \"{}\" is kept only until RandomCard quits", setting.name));
                }
                Err(_) => return Ok(()),
            }
        }
        if !value.is_empty() {
            self.session.insert(name, value.to_string());
        }
        Ok(())
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

fn keychain_entry(name: &str) -> Result<keyring_core::Entry, String> {
    KEYCHAIN.clone()?;
    keyring_core::Entry::new(SERVICE, name).map_err(|e| format!("There's no secure store to keep secrets in ({e})"))
}

/// An address as typed, with its scheme: https, as browsers take an address
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

/// A connection's setting by name, or why not.
fn declared_setting<'a>(conn: &'a Connection, name: &str) -> Result<&'a Setting, String> {
    conn.setting(name).ok_or_else(|| format!("Connection \"{}\" has no setting \"{}\"", conn.name, name.trim()))
}

/// `setting "Address" of connection "VideoDB"`: the user's value, or the
/// default. Secrets are never given to scripts.
pub fn read(store: &Store, stack: &Stack, conn: &str, setting: &str) -> Result<String, String> {
    let conn = declared(stack, conn)?;
    let s = declared_setting(conn, setting)?;
    if s.secret {
        return Err(format!("Setting \"{}\" of connection \"{}\" is secret: scripts can set it but not read it", s.name, conn.name));
    }
    Ok(store.value(&stack.uid, conn, s))
}

/// `set setting "Address" of connection "VideoDB" to x`: saved for this
/// user, never in the stack.
pub fn write(store: &mut Store, stack: &Stack, conn: &str, setting: &str, value: &str) -> Result<(), String> {
    let conn = declared(stack, conn)?;
    let s = declared_setting(conn, setting)?;
    store.set(&stack.uid, conn, s, value)
}

/// A web address made with a connection: a path (`/graphql`) is taken from
/// its Address, and a full address is used as it is.
pub fn url(store: &mut Store, stack: &Stack, conn: &str, url: &str) -> Result<String, String> {
    let c = declared(stack, conn)?;
    let missing = store.missing(&stack.uid, c);
    if !missing.is_empty() {
        return Err(format!("Connection \"{}\" isn't set up: it needs {}", c.name, missing.join(", ")));
    }
    let url = url.trim();
    if super::movies::is_url(url) {
        return Ok(url.to_string());
    }
    let address = store.value(&stack.uid, c, declared_setting(c, ADDRESS)?);
    let address = address.trim_end_matches('/');
    Ok(if url.is_empty() {
        address.to_string()
    } else if url.starts_with('?') || url.starts_with('#') {
        format!("{address}{url}")
    } else {
        format!("{address}/{}", url.trim_start_matches('/'))
    })
}

/// A settings file, which fills in a stack's settings for a user: given by
/// whoever runs the service, so each person needn't type them. It's JSON,
/// with the settings of each connection by name:
///
/// ```json
/// { "VideoDB": { "Address": "http://videos.local:9090", "Password": "…" } }
/// ```
///
/// The values are saved as if typed, secrets to the secure store; the file
/// itself is only read. Returns what was filled in, one line for each
/// connection, and the names this stack doesn't have.
pub fn import(store: &mut Store, stack: &Stack, text: &str) -> Result<Vec<String>, String> {
    let file: serde_json::Value = serde_json::from_str(text).map_err(|e| format!("That isn't a settings file: {e}"))?;
    let serde_json::Value::Object(conns) = file else {
        return Err("That isn't a settings file: it should list connections by name".into());
    };
    let mut report = Vec::new();
    let mut problems = Vec::new();
    for (name, settings) in &conns {
        let Some(conn) = stack.connection(name) else {
            report.push(format!("This stack has no connection \"{name}\""));
            continue;
        };
        let serde_json::Value::Object(settings) = settings else {
            report.push(format!("\"{name}\" should list settings by name"));
            continue;
        };
        let mut filled = Vec::new();
        for (setting, value) in settings {
            let Some(s) = conn.setting(setting) else {
                report.push(format!("Connection \"{}\" has no setting \"{setting}\"", conn.name));
                continue;
            };
            let value = match value {
                serde_json::Value::String(v) => v.clone(),
                serde_json::Value::Null => String::new(),
                other => other.to_string(),
            };
            match store.set(&stack.uid, conn, s, &value) {
                Ok(()) => filled.push(s.name.clone()),
                Err(why) => {
                    filled.push(s.name.clone());
                    problems.push(why);
                }
            }
        }
        if !filled.is_empty() {
            report.insert(0, format!("{}: {}", conn.name, filled.join(", ")));
        }
    }
    report.extend(problems);
    Ok(report)
}

/// Why a request can't carry a connection's credentials.
#[derive(Debug, PartialEq)]
pub enum SignError {
    /// The connection still needs settings: the user can be asked.
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

/// Whether a request's address is under a connection's Address: the same
/// scheme, host and port, and a path at or below the Address's. Gives how
/// specific the match is (the Address's path length), so the closest of
/// several wins.
fn under(address: &url::Url, request: &url::Url) -> Option<usize> {
    if address.scheme() != request.scheme()
        || address.host_str().map(str::to_ascii_lowercase) != request.host_str().map(str::to_ascii_lowercase)
        || address.port_or_known_default() != request.port_or_known_default()
    {
        return None;
    }
    let base = address.path().trim_end_matches('/');
    let path = request.path();
    (base.is_empty() || path == base || path.starts_with(&format!("{base}/"))).then_some(base.len())
}

/// The credentials a request to `url` carries: those of the stack's
/// connection whose Address it's under, filled in from the user's values.
/// None when no connection's Address covers it, or the connection has no
/// credentials. They go only to that Address, never over plain http
/// beyond this computer and the local network.
pub fn sign(store: &mut Store, stack: &Stack, url: &str) -> Result<Option<Credentials>, SignError> {
    let Ok(request) = url::Url::parse(url.trim()) else { return Ok(None) };
    let mut best: Option<(usize, &Connection)> = None;
    for c in &stack.connections {
        let Some(address) = c.setting(ADDRESS).map(|a| store.value(&stack.uid, c, a)) else { continue };
        let Ok(address) = url::Url::parse(&address) else { continue };
        if let Some(n) = under(&address, &request)
            && best.is_none_or(|(m, _)| n > m)
        {
            best = Some((n, c));
        }
    }
    let Some((_, conn)) = best else { return Ok(None) };
    if conn.credentials.is_empty() {
        return Ok(None);
    }
    let missing = store.missing(&stack.uid, conn);
    if !missing.is_empty() {
        return Err(SignError::NotSetUp {
            connection: conn.name.clone(),
            why: format!("Connection \"{}\" isn't set up: it needs {}", conn.name, missing.join(", ")),
        });
    }
    if request.scheme() == "http" && !request.host().is_some_and(|h| local_host(&h)) {
        return Err(SignError::Refused(format!(
            "Connection \"{}\" won't sign in over plain http to {}: its Address needs https",
            conn.name,
            request.host_str().unwrap_or("")
        )));
    }
    let mut out = Credentials::default();
    'each: for cred in &conn.credentials {
        // The value, with each {Setting} filled in.
        let (mut value, mut rest) = (String::new(), cred.value.as_str());
        while let Some(open) = rest.find('{') {
            let Some(close) = rest[open..].find('}').map(|c| open + c) else { break };
            let name = rest[open + 1..close].trim();
            let Some(setting) = conn.setting(name) else {
                return Err(SignError::Refused(format!("Connection \"{}\" has no setting \"{name}\"", conn.name)));
            };
            let given = if setting.secret {
                store.secret(&stack.uid, conn, setting).map_err(SignError::Refused)?.unwrap_or_default()
            } else {
                store.value(&stack.uid, conn, setting)
            };
            // An optional setting left empty leaves its credential off.
            if given.is_empty() && setting.optional {
                continue 'each;
            }
            value.push_str(&rest[..open]);
            value.push_str(&given);
            rest = &rest[close + 1..];
        }
        value.push_str(rest);
        match cred.place {
            Place::Basic => {
                use base64::Engine;
                let encoded = base64::engine::general_purpose::STANDARD.encode(value.as_bytes());
                out.headers.push(("Authorization".into(), format!("Basic {encoded}")));
            }
            Place::Header => out.headers.push((cred.name.trim().to_string(), value)),
            Place::Query => out.query.push((cred.name.trim().to_string(), value)),
        }
    }
    Ok(Some(out))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::connections::Setting;

    fn stack() -> Stack {
        let mut s = Stack::new("Movies");
        let mut c = Connection::new("VideoDB");
        c.settings[0].default = "http://trashcan.local:9090".into();
        c.settings.push(Setting { name: "User name".into(), ..Setting::default() });
        c.settings.push(Setting { name: "Password".into(), secret: true, ..Setting::default() });
        s.connections.push(c);
        s
    }

    #[test]
    fn values_and_defaults() {
        let (mut st, s) = (Store::in_memory(), stack());
        assert_eq!(read(&st, &s, "videodb", "address").unwrap(), "http://trashcan.local:9090");
        assert_eq!(read(&st, &s, "VideoDB", "User name").unwrap(), "");
        write(&mut st, &s, "VideoDB", "User name", "ada").unwrap();
        assert_eq!(read(&st, &s, "VideoDB", "user name").unwrap(), "ada");
        // Another stack declares its own connections.
        let other = Stack::new("Other");
        assert!(read(&st, &other, "VideoDB", "Address").unwrap_err().contains("no connection"));
        assert!(read(&st, &s, "VideoDB", "Color").unwrap_err().contains("no setting \"Color\""));
    }

    #[test]
    fn secrets_are_kept_but_not_read() {
        let (mut st, s) = (Store::in_memory(), stack());
        let c = &s.connections[0];
        assert_eq!(st.missing(&s.uid, c), vec!["User name", "Password"]);
        write(&mut st, &s, "VideoDB", "Password", "hunter2").unwrap();
        assert!(read(&st, &s, "VideoDB", "Password").unwrap_err().contains("secret"));
        assert_eq!(st.secret(&s.uid, c, &c.settings[2]).unwrap().as_deref(), Some("hunter2"));
        assert_eq!(st.missing(&s.uid, c), vec!["User name"]);
        write(&mut st, &s, "VideoDB", "Password", "").unwrap();
        assert_eq!(st.secret(&s.uid, c, &c.settings[2]).unwrap(), None);
    }

    /// The system's real secure store: saves, reads and forgets one secret.
    /// Run by hand (`--ignored`), as it needs a secure store running.
    #[test]
    #[ignore]
    fn the_system_keychain() {
        let s = stack();
        let c = &s.connections[0];
        let mut st = Store { keychain: true, ..Store::in_memory() };
        st.set(&s.uid, c, &c.settings[2], "keychain test").unwrap();
        assert_eq!(st.secret(&s.uid, c, &c.settings[2]).unwrap().as_deref(), Some("keychain test"));
        st.set(&s.uid, c, &c.settings[2], "").unwrap();
        assert_eq!(st.secret(&s.uid, c, &c.settings[2]).unwrap(), None);
    }

    #[test]
    fn settings_files() {
        let stack = stack();
        let mut store = Store::in_memory();
        let file = r#"{ "videodb": { "address": "http://videos.local", "Password": "pw", "Port": 9 }, "Other": {} }"#;
        let report = import(&mut store, &stack, file).unwrap();
        assert_eq!(report[0], "VideoDB: Address, Password");
        assert!(report.iter().any(|l| l.contains("no setting \"Port\"")));
        assert!(report.iter().any(|l| l.contains("no connection \"Other\"")));
        assert_eq!(read(&store, &stack, "VideoDB", "Address").unwrap(), "http://videos.local");
        let c = stack.connection("VideoDB").unwrap();
        assert_eq!(store.secret(&stack.uid, c, c.setting("Password").unwrap()).unwrap().as_deref(), Some("pw"));
        assert!(import(&mut store, &stack, "[1]").is_err() && import(&mut store, &stack, "{").is_err());
    }

    #[test]
    fn credentials_go_only_to_their_address() {
        use crate::model::connections::{Credential, Place};
        let mut s = Stack::new("Signed");
        let mut c = Connection::new("API");
        c.settings[0].default = "https://api.example.com/v2/".into();
        c.add_basic();
        c.settings.push(Setting { name: "Key".into(), secret: true, optional: true, ..Setting::default() });
        c.credentials.push(Credential { place: Place::Header, name: "X-API-Key".into(), value: "{ Key }".into() });
        c.credentials.push(Credential { place: Place::Query, name: "key".into(), value: "{Key}".into() });
        s.connections.push(c);
        let mut st = Store::in_memory();
        // Not set up: the Key is optional, the rest isn't.
        let e = sign(&mut st, &s, "https://api.example.com/v2/movies").unwrap_err();
        assert_eq!(e, SignError::NotSetUp {
            connection: "API".into(),
            why: "Connection \"API\" isn't set up: it needs User name, Password".into()
        });
        write(&mut st, &s, "API", "User name", "ada").unwrap();
        write(&mut st, &s, "API", "Password", "pw:1").unwrap();
        // Basic only, while the optional Key is empty.
        let c = sign(&mut st, &s, "https://API.example.com:443/v2/movies?page=1").unwrap().unwrap();
        assert_eq!(c.headers, [("Authorization".to_string(), "Basic YWRhOnB3OjE=".to_string())]);
        assert!(c.query.is_empty());
        write(&mut st, &s, "API", "Key", "k&1").unwrap();
        let c = sign(&mut st, &s, "https://api.example.com/v2").unwrap().unwrap();
        assert_eq!(c.headers[1], ("X-API-Key".to_string(), "k&1".to_string()));
        assert_eq!(c.query, [("key".to_string(), "k&1".to_string())]);
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
            assert_eq!(sign(&mut st, &s, other), Ok(None), "{other}");
        }
        // Plain http carries credentials only on this computer and the
        // local network.
        write(&mut st, &s, "API", "Address", "http://videos.example.com").unwrap();
        assert!(matches!(sign(&mut st, &s, "http://videos.example.com/graphql"), Err(SignError::Refused(w)) if w.contains("plain http")));
        for local in ["http://localhost:8080", "http://127.0.0.1", "http://trashcan.local:9090", "http://192.168.7.44", "http://[::1]:80"] {
            write(&mut st, &s, "API", "Address", local).unwrap();
            assert!(sign(&mut st, &s, &format!("{local}/graphql")).unwrap().is_some(), "{local}");
        }
    }

    #[test]
    fn addresses_from_a_connection() {
        let (mut st, s) = (Store::in_memory(), stack());
        assert!(url(&mut st, &s, "VideoDB", "/graphql").unwrap_err().contains("isn't set up: it needs User name, Password"));
        write(&mut st, &s, "VideoDB", "User name", "ada").unwrap();
        write(&mut st, &s, "VideoDB", "Password", "pw").unwrap();
        assert_eq!(url(&mut st, &s, "VideoDB", "/graphql").unwrap(), "http://trashcan.local:9090/graphql");
        assert_eq!(url(&mut st, &s, "VideoDB", "videos/1").unwrap(), "http://trashcan.local:9090/videos/1");
        assert_eq!(url(&mut st, &s, "VideoDB", "?page=2").unwrap(), "http://trashcan.local:9090?page=2");
        assert_eq!(url(&mut st, &s, "VideoDB", "https://x.org/a").unwrap(), "https://x.org/a");
        write(&mut st, &s, "VideoDB", "Address", "videodb.example.com/").unwrap();
        assert_eq!(url(&mut st, &s, "VideoDB", "/graphql").unwrap(), "https://videodb.example.com/graphql");
        assert_eq!(read(&st, &s, "VideoDB", "address").unwrap(), "https://videodb.example.com/");
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
