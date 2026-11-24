//! OAuth 2.0 (RFC 6749) for connections of type OAuth: getting tokens from
//! the provider, keeping them in the secure store, sending the access token
//! as `Authorization: Bearer` (RFC 6750), and refreshing it before it
//! expires. The stack never has to watch a token's clock.
//!
//! Grants, as stacks need them:
//!
//! - **Password** (RFC 6749 §4.3): `sign in connection "VideoDB" as user
//!   with password pw`. The user name and password are the stack's: it
//!   hands them over, and RandomCard keeps only the tokens.
//!
//! The provider's token URL is the connection's Token URL, or found from its
//! Provider URL's `/.well-known/openid-configuration` (OpenID Connect
//! Discovery 1.0; RFC 8414 for plain OAuth servers).

use std::time::{Duration, SystemTime, UNIX_EPOCH};

use crate::model::connections::{
    ACCESS_TOKEN, AUTHORIZATION_URL, CLIENT_ID, CODE_GRANT, CLIENT_SECRET, Connection, GRANT, PASSWORD_GRANT, PROVIDER_URL, REFRESH_TOKEN, SCOPES, TOKEN_URL,
    Type,
};
use crate::web::http;

use super::connections::{SignError, Store, kind, plain_http_refused, value, with_scheme};

/// How long before it expires an access token is replaced: time for a
/// request to arrive, plus the two computers' clocks disagreeing.
const MARGIN: u64 = 30;
/// How long to wait for the provider.
const TIMEOUT: Duration = Duration::from_secs(30);

fn now() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs())
}

/// An ordinary element's value, by name.
fn get(conn: &Connection, name: &str) -> String {
    conn.find(name).map(|f| value(conn, &f)).unwrap_or_default().trim().to_string()
}

/// Why a token request failed.
enum Fail {
    /// The provider said no (`invalid_grant`, a wrong password): the
    /// tokens it was asked about are no good.
    Rejected(String),
    /// No answer, or one that made no sense: try again later.
    Failed(String),
}

impl Fail {
    fn why(self) -> String {
        match self {
            Fail::Rejected(w) | Fail::Failed(w) => w,
        }
    }
}

/// One of the provider's addresses: the connection's own setting when it
/// has one (Token URL, Authorization URL), else what the provider's
/// discovery document says, read once a session.
fn endpoint(store: &mut Store, conn: &Connection, setting: &str, key: &str) -> Result<String, String> {
    let given = get(conn, setting);
    if !given.is_empty() {
        return Ok(with_scheme(&given));
    }
    let provider = with_scheme(&get(conn, PROVIDER_URL)).trim_end_matches('/').to_string();
    if provider.is_empty() {
        return Err(format!("Connection \"{}\" needs a {PROVIDER_URL} or a {setting}", conn.name));
    }
    if !store.discovered.contains_key(&provider) {
        let reply = (store.send)(&http::Request {
            method: "GET".into(),
            url: format!("{provider}/.well-known/openid-configuration"),
            headers: vec![("Accept".into(), "application/json".into())],
            body: None,
            timeout: TIMEOUT,
            credentials: None,
        })
        .map_err(|e| format!("Can't reach the provider of connection \"{}\": {e}", conn.name))?;
        let doc: serde_json::Value = serde_json::from_str(&reply.body).unwrap_or_default();
        if !reply.ok() || !doc.is_object() {
            return Err(format!(
                "The provider of connection \"{}\" has no discovery document ({}): give it a {setting}",
                conn.name,
                reply.status_text()
            ));
        }
        store.discovered.insert(provider.clone(), doc);
    }
    store.discovered[&provider].get(key).and_then(|t| t.as_str()).map(str::to_string).ok_or_else(|| {
        format!("The provider of connection \"{}\" doesn't say its {key}: give it a {setting}", conn.name)
    })
}

/// Where the connection's tokens come from.
fn token_url(store: &mut Store, conn: &Connection) -> Result<String, String> {
    endpoint(store, conn, TOKEN_URL, "token_endpoint")
}

/// When a JWT access token (RFC 7519) expires, from its `exp` claim.
pub(super) fn jwt_expiry(token: &str) -> Option<u64> {
    use base64::Engine;
    let claims = token.split('.').nth(1)?;
    let bytes = base64::engine::general_purpose::URL_SAFE_NO_PAD.decode(claims.trim_end_matches('=')).ok()?;
    let v: serde_json::Value = serde_json::from_slice(&bytes).ok()?;
    v.get("exp")?.as_u64()
}

/// A number from JSON that may come as text.
fn seconds(v: Option<&serde_json::Value>) -> Option<u64> {
    let v = v?;
    v.as_u64().or_else(|| v.as_str().and_then(|s| s.trim().parse().ok()))
}

/// Ask the provider for tokens, and keep what it gives. The client signs
/// in with Basic when it has a secret (RFC 6749 §2.3.1), else gives only
/// its id (a public client).
fn token_request(store: &mut Store, stack: &str, conn: &Connection, mut form: Vec<(&str, String)>) -> Result<(), Fail> {
    let url = token_url(store, conn).map_err(Fail::Failed)?;
    if let Ok(u) = url::Url::parse(&url)
        && let Some(why) = plain_http_refused(conn, &u)
    {
        return Err(Fail::Failed(why));
    }
    let id = get(conn, CLIENT_ID);
    let secret = match conn.find(CLIENT_SECRET) {
        Some(f) => store.secret(stack, conn, &f).map_err(Fail::Failed)?.unwrap_or_default(),
        None => String::new(),
    };
    let mut headers = vec![
        ("Content-Type".to_string(), "application/x-www-form-urlencoded".to_string()),
        ("Accept".to_string(), "application/json".to_string()),
    ];
    if secret.is_empty() {
        form.push(("client_id", id));
    } else {
        let pair = format!("{}:{}", http::form_encode(&id), http::form_encode(&secret));
        headers.push(("Authorization".into(), format!("Basic {}", http::base64_encode(&pair))));
    }
    let body = form.iter().map(|(k, v)| format!("{}={}", http::form_encode(k), http::form_encode(v))).collect::<Vec<_>>().join("&");
    let reply = (store.send)(&http::Request { method: "POST".into(), url, headers, body: Some(body), timeout: TIMEOUT, credentials: None })
        .map_err(|e| Fail::Failed(format!("Can't reach the provider of connection \"{}\": {e}", conn.name)))?;
    let v: serde_json::Value = serde_json::from_str(&reply.body).unwrap_or_default();
    let text = |k: &str| v.get(k).and_then(|x| x.as_str()).map(str::to_string);
    if !reply.ok() {
        // The provider's own words (RFC 6749 §5.2), else its status.
        let why = text("error_description").or_else(|| text("error")).unwrap_or_else(|| reply.status_text());
        return Err(if text("error").is_some() { Fail::Rejected(why) } else { Fail::Failed(why) });
    }
    let Some(access) = text("access_token") else {
        return Err(Fail::Failed(format!("The provider of connection \"{}\" sent no access token", conn.name)));
    };
    let expires = seconds(v.get("expires_in")).map(|s| now() + s).or_else(|| jwt_expiry(&access));
    store.keep_token(stack, conn, ACCESS_TOKEN, &access).map_err(Fail::Failed)?;
    // A provider that rotates refresh tokens sends a new one; one that
    // doesn't sends none, and the old one stays.
    if let Some(refresh) = text("refresh_token") {
        store.keep_token(stack, conn, REFRESH_TOKEN, &refresh).map_err(Fail::Failed)?;
    }
    store.set_expiry(stack, conn, expires);
    Ok(())
}

/// Forget a connection's tokens: `sign out connection "VideoDB"`.
pub fn forget(store: &mut Store, stack: &str, conn: &Connection) -> Result<(), String> {
    store.set_expiry(stack, conn, None);
    let a = store.keep_token(stack, conn, ACCESS_TOKEN, "");
    let r = store.keep_token(stack, conn, REFRESH_TOKEN, "");
    a.and(r)
}

/// What `sign in` does next: it's done (the Password grant), or the user
/// is signing in at the provider (the Authorization code grant), for the
/// script to wait on.
pub enum Started {
    Done(Result<(), String>),
    Browser(Pending),
}

/// `sign in connection "VideoDB" as user with password pw`, or with no user
/// name for the Authorization code grant: tokens from the provider, or the
/// provider's reason why not.
pub fn sign_in(store: &mut Store, stack: &str, conn: &Connection, user: Option<&str>, password: Option<&str>) -> Started {
    match kind(conn) {
        Some(Type::OAuth) => {}
        Some(t) => return Started::Done(Err(format!("Connection \"{}\" is {}: only OAuth connections sign in", conn.name, t.name()))),
        None => return Started::Done(Err(format!("Connection \"{}\" has no type yet", conn.name))),
    }
    if get(conn, CLIENT_ID).is_empty() {
        return Started::Done(Err(format!("Connection \"{}\" needs a {CLIENT_ID}", conn.name)));
    }
    let grant = get(conn, GRANT);
    let scopes = get(conn, SCOPES);
    if grant.eq_ignore_ascii_case(PASSWORD_GRANT) {
        let (Some(user), Some(password)) = (user, password) else {
            return Started::Done(Err(format!(
                "Connection \"{}\" signs in with a user name and password: sign in connection \"{}\" as user with password pw",
                conn.name, conn.name
            )));
        };
        let mut form = vec![("grant_type", "password".to_string()), ("username", user.to_string()), ("password", password.to_string())];
        if !scopes.is_empty() {
            form.push(("scope", scopes));
        }
        Started::Done(token_request(store, stack, conn, form).map_err(Fail::why))
    } else if grant.eq_ignore_ascii_case(CODE_GRANT) {
        match start_browser(store, conn, &scopes) {
            Ok(p) => Started::Browser(p),
            Err(why) => Started::Done(Err(why)),
        }
    } else {
        Started::Done(Err(format!("Connection \"{}\" can't sign in with the grant \"{grant}\"", conn.name)))
    }
}

/// How long a sign-in in the browser may take.
pub const BROWSER_WAIT: Duration = Duration::from_secs(5 * 60);

/// A sign-in in the user's browser, waiting for the provider to send the
/// browser back to RandomCard with a code (RFC 6749 §4.1, for native apps
/// as RFC 8252 says): to a one-time listener on this computer's loopback
/// address, on a port of its own.
pub struct Pending {
    listener: std::net::TcpListener,
    redirect: String,
    state: String,
    verifier: String,
    connection: String,
}

/// Random bytes as base64url with no padding: a PKCE verifier, a state.
fn random_text(bytes: usize) -> String {
    use base64::Engine;
    let b: Vec<u8> = (0..bytes).map(|_| rand::random::<u8>()).collect();
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(b)
}

/// PKCE's S256 challenge for a verifier (RFC 7636 §4.2).
pub(super) fn challenge(verifier: &str) -> String {
    use base64::Engine;
    use sha2::Digest;
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(sha2::Sha256::digest(verifier.as_bytes()))
}

/// Listen for the browser's return, and open the browser at the provider.
fn start_browser(store: &mut Store, conn: &Connection, scopes: &str) -> Result<Pending, String> {
    let authorize = endpoint(store, conn, AUTHORIZATION_URL, "authorization_endpoint")?;
    let mut url = url::Url::parse(&authorize).map_err(|e| format!("Connection \"{}\": {AUTHORIZATION_URL} {e}", conn.name))?;
    let listener = std::net::TcpListener::bind(("127.0.0.1", 0)).map_err(|e| format!("Can't wait for the sign-in: {e}"))?;
    listener.set_nonblocking(true).map_err(|e| format!("Can't wait for the sign-in: {e}"))?;
    let port = listener.local_addr().map_err(|e| e.to_string())?.port();
    let redirect = format!("http://127.0.0.1:{port}/callback");
    // 32 random bytes: a 43-character verifier, as RFC 7636 §4.1 suggests.
    let (verifier, state) = (random_text(32), random_text(16));
    {
        let mut q = url.query_pairs_mut();
        q.append_pair("response_type", "code")
            .append_pair("client_id", &get(conn, CLIENT_ID))
            .append_pair("redirect_uri", &redirect)
            .append_pair("state", &state)
            .append_pair("code_challenge", &challenge(&verifier))
            .append_pair("code_challenge_method", "S256");
        if !scopes.is_empty() {
            q.append_pair("scope", scopes);
        }
    }
    (store.open_browser)(url.as_str())?;
    Ok(Pending { listener, redirect, state, verifier, connection: conn.name.clone() })
}

/// Open a page in the system's own browser, never one inside RandomCard
/// (RFC 8252 §8.12): the user's passwords and sign-ins are there.
pub fn open_browser(url: &str) -> Result<(), String> {
    let mut cmd = if cfg!(target_os = "macos") {
        std::process::Command::new("open")
    } else if cfg!(windows) {
        // `start`'s first quoted argument is a window title.
        let mut c = std::process::Command::new("cmd");
        c.args(["/C", "start", ""]);
        c
    } else if cfg!(target_os = "haiku") {
        std::process::Command::new("open")
    } else {
        std::process::Command::new("xdg-open")
    };
    cmd.arg(url).spawn().map(|_| ()).map_err(|e| format!("Can't open the browser: {e}"))
}

/// The page the browser shows when it comes back.
fn page(title: &str, text: &str) -> String {
    let esc = |s: &str| s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;");
    format!(
        "<!DOCTYPE html><html><head><meta charset=\"utf-8\"><title>{t}</title></head>\
         <body style=\"font-family: system-ui, sans-serif; margin: 4em auto; max-width: 32em\">\
         <h1>{t}</h1><p>{x}</p></body></html>",
        t = esc(title),
        x = esc(text)
    )
}

impl Pending {
    /// Whether the browser has come back: the code, or why the sign-in
    /// failed; None while it's still out.
    pub fn poll(&mut self) -> Option<Result<String, String>> {
        use std::io::{Read, Write};
        let (mut stream, _) = self.listener.accept().ok()?;
        let _ = stream.set_nonblocking(false);
        let _ = stream.set_read_timeout(Some(Duration::from_secs(5)));
        let mut buf = vec![0u8; 8192];
        let mut got = 0;
        // The request line and headers; the body (there's none) is ignored.
        while got < buf.len() {
            match stream.read(&mut buf[got..]) {
                Ok(0) | Err(_) => break,
                Ok(n) => {
                    got += n;
                    if buf[..got].windows(4).any(|w| w == b"\r\n\r\n") {
                        break;
                    }
                }
            }
        }
        let head = String::from_utf8_lossy(&buf[..got]).into_owned();
        let target = head.lines().next().and_then(|l| l.split_whitespace().nth(1)).unwrap_or("");
        let reply = |stream: &mut std::net::TcpStream, status: &str, body: &str| {
            let _ = write!(
                stream,
                "HTTP/1.1 {status}\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            );
        };
        // Anything else the browser asks for (a favicon) isn't the sign-in.
        let Ok(url) = url::Url::parse(&format!("http://127.0.0.1{target}")) else { return None };
        if url.path() != "/callback" {
            reply(&mut stream, "404 Not Found", "");
            return None;
        }
        let q: std::collections::BTreeMap<String, String> = url.query_pairs().into_owned().collect();
        let name = &self.connection;
        if q.get("state") != Some(&self.state) {
            reply(&mut stream, "400 Bad Request", &page("Not signed in", "This sign-in isn't the one RandomCard started. Try again from RandomCard."));
            return Some(Err(format!("The sign-in to connection \"{name}\" came back from a sign-in RandomCard didn't start")));
        }
        if let Some(error) = q.get("error") {
            let why = q.get("error_description").cloned().unwrap_or_else(|| error.clone());
            reply(&mut stream, "200 OK", &page("Not signed in", &format!("{why}. You can close this tab and go back to RandomCard.")));
            return Some(Err(why));
        }
        let Some(code) = q.get("code") else {
            reply(&mut stream, "400 Bad Request", &page("Not signed in", "The provider sent no code."));
            return Some(Err(format!("The provider of connection \"{name}\" sent no code")));
        };
        reply(
            &mut stream,
            "200 OK",
            &page("Signed in", &format!("RandomCard is signed in to {name}. You can close this tab and go back to RandomCard.")),
        );
        Some(Ok(code.clone()))
    }

    /// The code, swapped for tokens with the PKCE verifier (RFC 6749 §4.1.3).
    pub fn finish(self, store: &mut Store, stack: &str, conn: &Connection, code: &str) -> Result<(), String> {
        let form = vec![
            ("grant_type", "authorization_code".to_string()),
            ("code", code.to_string()),
            ("redirect_uri", self.redirect),
            ("code_verifier", self.verifier),
        ];
        token_request(store, stack, conn, form).map_err(Fail::why)
    }
}

/// A fresh access token from the refresh token (RFC 6749 §6). One the
/// provider refuses is forgotten, with the access token: the user signs in
/// again.
fn refresh(store: &mut Store, stack: &str, conn: &Connection) -> Result<String, SignError> {
    let not_signed_in = || SignError::NotSetUp {
        connection: conn.name.clone(),
        why: format!("Connection \"{}\" isn't signed in", conn.name),
    };
    let Some(refresh) = store.token(stack, conn, REFRESH_TOKEN) else {
        let _ = forget(store, stack, conn);
        return Err(not_signed_in());
    };
    match token_request(store, stack, conn, vec![("grant_type", "refresh_token".into()), ("refresh_token", refresh)]) {
        Ok(()) => store.token(stack, conn, ACCESS_TOKEN).ok_or_else(not_signed_in),
        Err(Fail::Rejected(why)) => {
            let _ = forget(store, stack, conn);
            Err(SignError::NotSetUp { connection: conn.name.clone(), why: format!("Connection \"{}\" isn't signed in: {why}", conn.name) })
        }
        Err(Fail::Failed(why)) => Err(SignError::Refused(why)),
    }
}

/// The access token for a request, refreshed first when it's about to
/// expire.
pub fn bearer(store: &mut Store, stack: &str, conn: &Connection) -> Result<String, SignError> {
    if let Some(access) = store.token(stack, conn, ACCESS_TOKEN)
        && store.expiry(stack, conn, &access).is_none_or(|at| at > now() + MARGIN)
    {
        return Ok(access);
    }
    refresh(store, stack, conn)
}

/// After a 401 for `refused`: the token another request has already
/// fetched, or a fresh one; None when there's none to be had.
pub fn renew(store: &mut Store, stack: &str, conn: &Connection, refused: &str) -> Option<String> {
    if let Some(access) = store.token(stack, conn, ACCESS_TOKEN)
        && access != refused
        && store.expiry(stack, conn, &access).is_none_or(|at| at > now() + MARGIN)
    {
        return Some(access);
    }
    refresh(store, stack, conn).ok()
}

/// Whether a connection has tokens: an access token, or a refresh token to
/// get one with.
pub fn signed_in(store: &mut Store, stack: &str, conn: &Connection) -> bool {
    store.token(stack, conn, ACCESS_TOKEN).is_some() || store.token(stack, conn, REFRESH_TOKEN).is_some()
}

/// When the access token expires, in seconds as `the seconds` counts them;
/// empty when there's none, or no way to tell.
pub fn expires(store: &mut Store, stack: &str, conn: &Connection) -> String {
    store
        .token(stack, conn, ACCESS_TOKEN)
        .and_then(|a| store.expiry(stack, conn, &a))
        .map(|at| (super::dates::to_seconds(super::dates::now()) + at as i64 - now() as i64).to_string())
        .unwrap_or_default()
}

#[cfg(test)]
pub(super) mod tests {
    use super::*;
    use crate::model::Stack;
    use crate::model::connections::{Given, TYPE, URL};
    use std::sync::Mutex;

    /// What the fake provider was sent, and what it said.
    pub static SENT: Mutex<Vec<String>> = Mutex::new(Vec::new());

    /// A provider: discovery at /realms/test, tokens for viewer/secret,
    /// and a refresh token "r1" that gives a fresh one.
    pub fn provider(req: &http::Request) -> Result<http::Response, String> {
        let reply = |status: u16, body: &str| {
            Ok(http::Response { status, reason: String::new(), headers: String::new(), body: body.to_string() })
        };
        SENT.lock().unwrap().push(format!("{} {} {}", req.method, req.url, req.body.clone().unwrap_or_default()));
        let body = req.body.clone().unwrap_or_default();
        match req.url.as_str() {
            "http://login.local/realms/test/.well-known/openid-configuration" => reply(
                200,
                r#"{"token_endpoint": "http://login.local/realms/test/token", "authorization_endpoint": "http://login.local/realms/test/auth?ui=1"}"#,
            ),
            // The code the fake browser brought back, with the verifier
            // whose challenge it was given.
            "http://login.local/realms/test/token" if body.contains("grant_type=authorization_code") => {
                let form: std::collections::BTreeMap<String, String> =
                    url::form_urlencoded::parse(body.as_bytes()).into_owned().collect();
                let verifier = form.get("code_verifier").cloned().unwrap_or_default();
                let challenged = CHALLENGE.lock().unwrap().clone();
                if form.get("code").map(String::as_str) == Some("c1")
                    && challenge(&verifier) == challenged
                    && form.get("redirect_uri").is_some_and(|r| r.starts_with("http://127.0.0.1:"))
                {
                    reply(200, r#"{"access_token": "b1", "refresh_token": "r1", "expires_in": 300}"#)
                } else {
                    reply(400, r#"{"error": "invalid_grant", "error_description": "Code not valid"}"#)
                }
            }
            "http://login.local/realms/test/token" if body.contains("grant_type=password") => {
                if body.contains("username=viewer") && body.contains("password=secret") {
                    reply(200, r#"{"access_token": "a1", "refresh_token": "r1", "expires_in": 300, "token_type": "Bearer"}"#)
                } else {
                    reply(401, r#"{"error": "invalid_grant", "error_description": "Invalid user credentials"}"#)
                }
            }
            "http://login.local/realms/test/token" if body.contains("refresh_token=r1") => {
                reply(200, r#"{"access_token": "a2", "expires_in": 300}"#)
            }
            "http://login.local/realms/test/token" => reply(400, r#"{"error": "invalid_grant", "error_description": "Token is not active"}"#),
            _ => reply(404, ""),
        }
    }

    /// The challenge the fake browser was sent to the provider with.
    static CHALLENGE: Mutex<String> = Mutex::new(String::new());

    /// A browser: the user signs in at once, and the provider sends it
    /// back to the redirect with a code and the state, or a refusal.
    fn browser(url: &str) -> Result<(), String> {
        let u = url::Url::parse(url).map_err(|e| e.to_string())?;
        let q: std::collections::BTreeMap<String, String> = u.query_pairs().into_owned().collect();
        assert_eq!(u.path(), "/realms/test/auth");
        assert_eq!(q.get("ui").map(String::as_str), Some("1"), "the endpoint's own query stays");
        assert_eq!(q["code_challenge_method"], "S256");
        assert_eq!(q["client_id"], "my-app");
        *CHALLENGE.lock().unwrap() = q["code_challenge"].clone();
        let back = format!("{}?code=c1&state={}", q["redirect_uri"], q["state"]);
        std::thread::spawn(move || {
            use std::io::{Read, Write};
            let u = url::Url::parse(&back).unwrap();
            let mut s = std::net::TcpStream::connect(("127.0.0.1", u.port().unwrap())).unwrap();
            // A favicon first, as browsers ask: not the sign-in.
            write!(s, "GET /favicon.ico HTTP/1.1\r\nHost: x\r\n\r\n").unwrap();
            let mut page = String::new();
            let _ = s.read_to_string(&mut page);
            let mut s = std::net::TcpStream::connect(("127.0.0.1", u.port().unwrap())).unwrap();
            write!(s, "GET /callback?{} HTTP/1.1\r\nHost: x\r\n\r\n", u.query().unwrap()).unwrap();
            let _ = s.read_to_string(&mut page);
            assert!(page.contains("Signed in"), "{page}");
        });
        Ok(())
    }

    #[test]
    fn signs_in_in_the_browser_with_pkce() {
        let mut st = store();
        st.open_browser = browser;
        let mut s = stack();
        s.connections[0].declare(GRANT, Given::Fixed, CODE_GRANT);
        let conn = s.connections[0].clone();
        let Started::Browser(mut pending) = sign_in(&mut st, "Movies", &conn, None, None) else { panic!("no browser") };
        let until = std::time::Instant::now() + Duration::from_secs(5);
        let code = loop {
            if let Some(back) = pending.poll() {
                break back.unwrap();
            }
            assert!(std::time::Instant::now() < until, "the browser never came back");
            std::thread::sleep(Duration::from_millis(10));
        };
        assert_eq!(code, "c1");
        pending.finish(&mut st, "Movies", &conn, &code).unwrap();
        assert_eq!(st.token("Movies", &conn, ACCESS_TOKEN).as_deref(), Some("b1"));
        // RFC 7636's own example.
        assert_eq!(challenge("dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk"), "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM");
    }

    fn stack() -> Stack {
        let mut s = Stack::new("Movies");
        let mut c = Connection::new("API");
        c.declare(TYPE, Given::Fixed, "OAuth");
        c.declare(URL, Given::Default, "http://api.local:8080");
        c.declare(PROVIDER_URL, Given::Default, "http://login.local/realms/test");
        c.declare(CLIENT_ID, Given::Default, "my-app");
        c.declare(SCOPES, Given::Default, "openid offline_access");
        s.connections.push(c);
        s
    }

    fn store() -> Store {
        let mut st = Store::in_memory();
        st.send = provider;
        st
    }

    #[test]
    fn signs_in_signs_requests_and_refreshes() {
        use super::super::connections::{read, sign};
        let (mut st, s) = (store(), stack());
        let conn = s.connection("API").unwrap().clone();
        // Not signed in: nothing is sent, and the result says so.
        let at = "http://api.local:8080/api/me";
        assert!(matches!(sign(&mut st, &s, at, None), Err(SignError::NotSetUp { why, .. }) if why.contains("isn't signed in")));
        assert_eq!(read(&mut st, &s, "API", "Signed in").unwrap(), "false");

        let done = |st: &mut Store, pw: &str| match sign_in(st, "Movies", &conn, Some("viewer"), Some(pw)) {
            Started::Done(d) => d,
            Started::Browser(_) => panic!("a browser for a password"),
        };
        assert_eq!(done(&mut st, "wrong").unwrap_err(), "Invalid user credentials");
        done(&mut st, "secret").unwrap();
        {
            let sent = SENT.lock().unwrap();
            let post = sent.iter().find(|l| l.contains("grant_type=password")).unwrap();
            assert!(post.contains("client_id=my-app") && post.contains("scope=openid+offline_access"), "{post}");
        }
        assert_eq!(read(&mut st, &s, "API", "Signed in").unwrap(), "true");
        let expires: i64 = read(&mut st, &s, "API", "Token expires").unwrap().parse().unwrap();
        let left = expires - super::super::dates::to_seconds(super::super::dates::now());
        assert!((298..=300).contains(&left), "{left}");
        // The token is a secret: readable only when the connection allows.
        assert!(read(&mut st, &s, "API", "Access token").is_err());

        let c = sign(&mut st, &s, at, None).unwrap().unwrap();
        assert!(c.headers.contains(&("Authorization".into(), "Bearer a1".into())));
        assert!(c.renew.is_some());

        // About to expire: refreshed before the request goes.
        st.set_expiry("Movies", &conn, Some(now() + 10));
        let c = sign(&mut st, &s, at, None).unwrap().unwrap();
        assert!(c.headers.contains(&("Authorization".into(), "Bearer a2".into())));
        // The refresh token stays when the provider sends no new one.
        assert_eq!(st.token("Movies", &conn, REFRESH_TOKEN).as_deref(), Some("r1"));

        // A 401: another request's fresh token, if there is one.
        assert_eq!(renew(&mut st, "Movies", &conn, "a1").as_deref(), Some("a2"));

        // A refresh token the provider refuses: signed out.
        st.keep_token("Movies", &conn, REFRESH_TOKEN, "old").unwrap();
        st.set_expiry("Movies", &conn, Some(now()));
        assert!(matches!(sign(&mut st, &s, at, None), Err(SignError::NotSetUp { why, .. }) if why.contains("Token is not active")));
        assert_eq!(read(&mut st, &s, "API", "Signed in").unwrap(), "false");

        done(&mut st, "secret").unwrap();
        forget(&mut st, "Movies", &conn).unwrap();
        assert_eq!(read(&mut st, &s, "API", "Signed in").unwrap(), "false");
    }

    #[test]
    fn jwt_expiry_from_exp() {
        // {"alg":"none"}.{"exp":1700000000}.
        assert_eq!(jwt_expiry("eyJhbGciOiJub25lIn0.eyJleHAiOjE3MDAwMDAwMDB9."), Some(1_700_000_000));
        assert_eq!(jwt_expiry("opaque"), None);
    }
}
