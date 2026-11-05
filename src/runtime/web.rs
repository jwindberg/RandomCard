//! HyperTalk's side of the web (the library is `crate::web`):
//!
//! - `put url "https://…" into x` (and `get url …`) fetches a page. `the
//!   result` is empty when it worked; otherwise it's the HTTP status
//!   ("404 Not Found") or what went wrong, and the reply is still there to
//!   read. `the httpHeaders` (one `Name: value` a line) go with every
//!   request, `the httpTimeout` is in seconds, and `the responseHeaders`
//!   are the last reply's.
//! - `key "value" of json x` reads a key of a JSON object, and nests:
//!   `key "name" of key "owner" of json x`.

use std::sync::atomic::Ordering;
use std::sync::mpsc;
use std::time::Duration;

use crate::web::{http, json};

use super::interp::{Ctl, Interp, R, err};

impl Interp {
    /// The web's functions, or None if `name` isn't one.
    pub(super) fn web_function(&mut self, name: &str, args: &[String]) -> Option<R<String>> {
        match name {
            "__url" => Some(self.fetch("GET", args.first().cloned().unwrap_or_default(), None)),
            // `json x`, then its keys from the outside in.
            "__json" => {
                let (source, keys) = args.split_first()?;
                Some(json::get(source, keys).or_else(err))
            }
            _ => None,
        }
    }

    /// Send a request with the script's headers and timeout. It waits on
    /// another thread, so Command-period still stops the script. The
    /// reply's body is the value; `the result` says how it went.
    pub(super) fn fetch(&mut self, method: &str, url: String, body: Option<String>) -> R<String> {
        let req = http::Request {
            method: method.into(),
            url: url.trim().to_string(),
            headers: http::parse_headers(&self.http_headers),
            body,
            timeout: Duration::from_secs_f64(self.http_timeout.max(0.1)),
        };
        let (tx, rx) = mpsc::channel();
        std::thread::spawn(move || {
            let _ = tx.send(http::send(&req));
        });
        let reply = loop {
            match rx.recv_timeout(Duration::from_millis(30)) {
                Ok(r) => break r,
                Err(mpsc::RecvTimeoutError::Timeout) => {
                    if self.abort.load(Ordering::SeqCst) {
                        return Err(Ctl::ExitToHyperCard);
                    }
                }
                Err(mpsc::RecvTimeoutError::Disconnected) => break Err("The request stopped".into()),
            }
        };
        match reply {
            Ok(r) => {
                self.result = if r.ok() { String::new() } else { r.status_text() };
                self.response_headers = r.headers;
                Ok(r.body)
            }
            Err(e) => {
                self.result = e;
                self.response_headers.clear();
                Ok(String::new())
            }
        }
    }
}
