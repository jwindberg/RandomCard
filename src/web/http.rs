//! A small HTTP client for REST services, over ureq: any method, headers,
//! a body and a timeout in; status, headers and body out, whatever the
//! status. TLS is Rustls on Linux and the system's own elsewhere (see
//! Cargo.toml).

use std::time::Duration;

/// What the client sends.
#[derive(Clone, Debug)]
pub struct Request {
    pub method: String,
    pub url: String,
    pub headers: Vec<(String, String)>,
    pub body: Option<String>,
    pub timeout: Duration,
}

/// A reply, whatever its status.
#[derive(Clone, Debug, PartialEq)]
pub struct Response {
    pub status: u16,
    pub reason: String,
    /// `Name: value`, one a line.
    pub headers: String,
    pub body: String,
}

impl Response {
    /// A 2xx status.
    pub fn ok(&self) -> bool {
        (200..300).contains(&self.status)
    }

    /// The status as people read it: "404 Not Found".
    pub fn status_text(&self) -> String {
        format!("{} {}", self.status, self.reason).trim().to_string()
    }
}

/// Text made safe for a URL's path or query: everything but letters,
/// digits and `-_.~` as `%XX` bytes of its UTF-8.
pub fn url_encode(text: &str) -> String {
    let mut out = String::new();
    for b in text.bytes() {
        if b.is_ascii_alphanumeric() || b"-_.~".contains(&b) {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

/// `%XX` bytes, and `+` for a space as forms send it, back to text.
pub fn url_decode(text: &str) -> String {
    let b = text.as_bytes();
    let hex = |i: usize| b.get(i).and_then(|c| (*c as char).to_digit(16));
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        match (b[i], hex(i + 1), hex(i + 2)) {
            (b'%', Some(h), Some(l)) => {
                out.push((h * 16 + l) as u8);
                i += 3;
            }
            (c, _, _) => {
                out.push(if c == b'+' { b' ' } else { c });
                i += 1;
            }
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// Headers from text, one `Name: value` a line; other lines are skipped.
pub fn parse_headers(text: &str) -> Vec<(String, String)> {
    text.lines()
        .filter_map(|l| l.split_once(':'))
        .map(|(k, v)| (k.trim().to_string(), v.trim().to_string()))
        .filter(|(k, _)| !k.is_empty())
        .collect()
}

fn agent(timeout: Duration) -> ureq::Agent {
    let config = ureq::Agent::config_builder().http_status_as_error(false).timeout_global(Some(timeout));
    #[cfg(not(target_os = "linux"))]
    let config = config.tls_config(ureq::tls::TlsConfig::builder().provider(ureq::tls::TlsProvider::NativeTls).build());
    config.build().into()
}

/// Send a request and wait for the reply. Err is a request that got no
/// reply at all: a bad address, no network, a timeout.
pub fn send(req: &Request) -> Result<Response, String> {
    let agent = agent(req.timeout);
    macro_rules! with_headers {
        ($builder:expr) => {{
            let mut b = $builder;
            for (k, v) in &req.headers {
                b = b.header(k.as_str(), v.as_str());
            }
            b
        }};
    }
    let body = req.body.as_deref().unwrap_or("").as_bytes();
    let reply = match req.method.to_ascii_uppercase().as_str() {
        "GET" => with_headers!(agent.get(&req.url)).call(),
        "DELETE" => with_headers!(agent.delete(&req.url)).call(),
        "HEAD" => with_headers!(agent.head(&req.url)).call(),
        "POST" => with_headers!(agent.post(&req.url)).send(body),
        "PUT" => with_headers!(agent.put(&req.url)).send(body),
        "PATCH" => with_headers!(agent.patch(&req.url)).send(body),
        other => return Err(format!("Unknown HTTP method {other}")),
    };
    let mut reply = reply.map_err(|e| e.to_string())?;
    let status = reply.status();
    let headers = reply
        .headers()
        .iter()
        .map(|(k, v)| format!("{}: {}", k.as_str(), v.to_str().unwrap_or("")))
        .collect::<Vec<_>>()
        .join("\n");
    let bytes = reply.body_mut().read_to_vec().map_err(|e| e.to_string())?;
    Ok(Response {
        status: status.as_u16(),
        reason: status.canonical_reason().unwrap_or("").to_string(),
        headers,
        body: String::from_utf8_lossy(&bytes).into_owned(),
    })
}

#[cfg(test)]
pub mod test_server {
    //! A web server for tests, on a free local port: it answers each
    //! request from a function of (method, path, headers, body).

    use std::io::{BufRead, BufReader, Read, Write};
    use std::net::TcpListener;

    pub type Handler = fn(&str, &str, &str, &str) -> (u16, &'static str, String);

    /// Start one; returns its base URL (`http://127.0.0.1:port`).
    pub fn start(handler: Handler) -> String {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        std::thread::spawn(move || {
            for stream in listener.incoming().flatten() {
                let mut reader = BufReader::new(stream.try_clone().unwrap());
                let mut first = String::new();
                if reader.read_line(&mut first).is_err() {
                    continue;
                }
                let mut parts = first.split_whitespace();
                let (method, path) = (parts.next().unwrap_or("").to_string(), parts.next().unwrap_or("").to_string());
                let (mut headers, mut length) = (String::new(), 0usize);
                loop {
                    let mut line = String::new();
                    if reader.read_line(&mut line).unwrap_or(0) == 0 || line.trim().is_empty() {
                        break;
                    }
                    if let Some((k, v)) = line.split_once(':')
                        && k.trim().eq_ignore_ascii_case("content-length")
                    {
                        length = v.trim().parse().unwrap_or(0);
                    }
                    headers.push_str(&line);
                }
                let mut body = vec![0; length];
                let _ = reader.read_exact(&mut body);
                let (status, kind, reply) = handler(&method, &path, &headers, &String::from_utf8_lossy(&body));
                let mut out = stream;
                let _ = write!(
                    out,
                    "HTTP/1.1 {status} X\r\nContent-Type: {kind}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{reply}",
                    reply.len()
                );
            }
        });
        format!("http://127.0.0.1:{port}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn url_text() {
        assert_eq!(url_encode("chuck norris & co/é"), "chuck%20norris%20%26%20co%2F%C3%A9");
        assert_eq!(url_decode("chuck%20norris+%26%20co%2F%C3%A9"), "chuck norris & co/é");
        assert_eq!(url_decode("100%"), "100%");
    }

    #[test]
    fn headers_from_text() {
        assert_eq!(
            parse_headers("Accept: application/json\nAuthorization: Bearer x:y\n\nnot a header"),
            vec![("Accept".into(), "application/json".into()), ("Authorization".into(), "Bearer x:y".into())]
        );
    }

    #[test]
    fn requests_and_replies() {
        let base = test_server::start(|method, path, headers, body| match path {
            "/echo" => (200, "text/plain", format!("{method} {body} {}", headers.to_ascii_lowercase().contains("x-token: abc"))),
            _ => (404, "application/json", r#"{"error":"Not Found"}"#.into()),
        });
        let req = |method: &str, path: &str, body: Option<&str>| Request {
            method: method.into(),
            url: format!("{base}{path}"),
            headers: parse_headers("X-Token: abc"),
            body: body.map(String::from),
            timeout: Duration::from_secs(5),
        };
        let r = send(&req("GET", "/echo", None)).unwrap();
        assert!(r.ok());
        assert_eq!(r.body, "GET  true");
        assert_eq!(send(&req("POST", "/echo", Some("hello"))).unwrap().body, "POST hello true");
        // A 404 is still a reply, with its body.
        let r = send(&req("GET", "/nothing", None)).unwrap();
        assert_eq!((r.ok(), r.status_text().as_str()), (false, "404 Not Found"));
        assert!(r.body.contains("Not Found"));
        assert!(r.headers.to_ascii_lowercase().contains("content-type: application/json"));
        // No server there at all.
        let none = Request { url: "http://127.0.0.1:1/".into(), ..req("GET", "/", None) };
        assert!(send(&none).is_err());
    }
}
