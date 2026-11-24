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
    ACCESS_TOKEN, CLIENT_ID, CLIENT_SECRET, Connection, GRANT, PASSWORD_GRANT, PROVIDER_URL, REFRESH_TOKEN, SCOPES, TOKEN_URL,
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

/// Where the connection's tokens come from: its Token URL, or what its
/// provider's discovery document says, read once a session.
fn token_url(store: &mut Store, conn: &Connection) -> Result<String, String> {
    let token = get(conn, TOKEN_URL);
    if !token.is_empty() {
        return Ok(with_scheme(&token));
    }
    let provider = with_scheme(&get(conn, PROVIDER_URL)).trim_end_matches('/').to_string();
    if provider.is_empty() {
        return Err(format!("Connection \"{}\" needs a {PROVIDER_URL} or a {TOKEN_URL}", conn.name));
    }
    if let Some(t) = store.token_urls.get(&provider) {
        return Ok(t.clone());
    }
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
    let Some(t) = doc.get("token_endpoint").and_then(|t| t.as_str()).filter(|_| reply.ok()) else {
        return Err(format!(
            "The provider of connection \"{}\" doesn't say where its tokens come from ({}): give it a {TOKEN_URL}",
            conn.name,
            reply.status_text()
        ));
    };
    store.token_urls.insert(provider, t.to_string());
    Ok(t.to_string())
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

/// `sign in connection "VideoDB" as user with password pw`: tokens from
/// the provider, or the provider's reason why not.
pub fn sign_in(store: &mut Store, stack: &str, conn: &Connection, user: Option<&str>, password: Option<&str>) -> Result<(), String> {
    match kind(conn) {
        Some(Type::OAuth) => {}
        Some(t) => return Err(format!("Connection \"{}\" is {}: only OAuth connections sign in", conn.name, t.name())),
        None => return Err(format!("Connection \"{}\" has no type yet", conn.name)),
    }
    if get(conn, CLIENT_ID).is_empty() {
        return Err(format!("Connection \"{}\" needs a {CLIENT_ID}", conn.name));
    }
    let grant = get(conn, GRANT);
    if grant.eq_ignore_ascii_case(PASSWORD_GRANT) {
        let (Some(user), Some(password)) = (user, password) else {
            return Err(format!(
                "Connection \"{}\" signs in with a user name and password: sign in connection \"{}\" as user with password pw",
                conn.name, conn.name
            ));
        };
        let mut form = vec![("grant_type", "password".to_string()), ("username", user.to_string()), ("password", password.to_string())];
        let scopes = get(conn, SCOPES);
        if !scopes.is_empty() {
            form.push(("scope", scopes));
        }
        token_request(store, stack, conn, form).map_err(Fail::why)
    } else {
        Err(format!("Connection \"{}\" can't sign in with the grant \"{grant}\" yet", conn.name))
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
            "http://login.local/realms/test/.well-known/openid-configuration" => {
                reply(200, r#"{"token_endpoint": "http://login.local/realms/test/token"}"#)
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

        assert_eq!(sign_in(&mut st, "Movies", &conn, Some("viewer"), Some("wrong")).unwrap_err(), "Invalid user credentials");
        sign_in(&mut st, "Movies", &conn, Some("viewer"), Some("secret")).unwrap();
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

        sign_in(&mut st, "Movies", &conn, Some("viewer"), Some("secret")).unwrap();
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
