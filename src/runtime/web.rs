//! HyperTalk's side of the web (the library is `crate::web`):
//!
//! - `put url "https://…" into x` (and `get url …`) fetches a page. `the
//!   result` is empty when it worked; otherwise it's the HTTP status
//!   ("404 Not Found") or what went wrong, and the reply is still there to
//!   read. `the httpHeaders` (one `Name: value` a line) go with every
//!   request, `the httpTimeout` is in seconds, and `the responseHeaders`
//!   are the last reply's.
//! - `key "value" of json x` reads a key of a JSON object, `element 2 of
//!   json x` an element of an array (from 1, or `last`), and they nest:
//!   `key "value" of element 1 of key "result" of json x`. `the keys of
//!   json x` and `the elements of json x` are one a line, and `the number
//!   of elements of json x` counts them.

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
            // `json x`, then its steps from the outside in, each a kind
            // ("k" for a key, "e" for an element) and a value.
            "__json" | "__jsonkeys" | "__jsonelements" | "__jsoncount" | "__jsonpretty" => {
                let (source, rest) = args.split_first()?;
                let steps: Vec<json::Step> = rest
                    .chunks(2)
                    .map(|p| match p {
                        [k, v] if k == "e" => json::Step::Element(v.clone()),
                        [_, v] => json::Step::Key(v.clone()),
                        _ => json::Step::Key(String::new()),
                    })
                    .collect();
                // A loop reads the same reply again and again: parse it once.
                if self.json_cache.as_ref().is_none_or(|(text, _)| text != source) {
                    match json::parse(source) {
                        Ok(v) => self.json_cache = Some((source.clone(), v)),
                        // `the pretty of json x` is for showing: text that
                        // isn't JSON shows as it is.
                        Err(_) if name == "__jsonpretty" && rest.is_empty() => return Some(Ok(source.clone())),
                        Err(e) => return Some(err(e)),
                    }
                }
                let value = json::walk(&self.json_cache.as_ref()?.1, &steps);
                Some(Ok(match name {
                    "__jsonkeys" => json::keys(&value),
                    "__jsonelements" => json::elements(&value),
                    "__jsoncount" => json::count(&value).to_string(),
                    "__jsonpretty" => json::pretty(&value),
                    _ => json::text(&value),
                }))
            }
            _ => None,
        }
    }

    /// A request with the script's headers and timeout.
    fn web_request(&self, method: &str, url: String, body: Option<String>) -> http::Request {
        http::Request {
            method: method.into(),
            url: url.trim().to_string(),
            headers: http::parse_headers(&self.http_headers),
            body,
            timeout: Duration::from_secs_f64(self.http_timeout.max(0.1)),
        }
    }

    /// Note a reply in `the result` (empty when it worked, else the status
    /// or what went wrong), `the responseStatus`, `the responseHeaders` and
    /// `the responseTime`; its body is the value.
    pub(super) fn take_reply(&mut self, reply: Result<http::Response, String>, millis: f64) -> String {
        self.response_time = millis;
        match reply {
            Ok(r) => {
                self.result = if r.ok() { String::new() } else { r.status_text() };
                self.response_status = r.status_text();
                self.response_headers = r.headers;
                r.body
            }
            Err(e) => {
                self.result = e;
                self.response_status.clear();
                self.response_headers.clear();
                String::new()
            }
        }
    }

    /// Send a request and wait for it. It waits on another thread, so
    /// Command-period still stops the script.
    pub(super) fn fetch(&mut self, method: &str, url: String, body: Option<String>) -> R<String> {
        let req = self.web_request(method, url, body);
        let (tx, rx) = mpsc::channel();
        let started = std::time::Instant::now();
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
        Ok(self.take_reply(reply, started.elapsed().as_secs_f64() * 1000.0))
    }

    /// Send a request without waiting: the script goes on, and when the
    /// reply comes, `message` goes to the object whose script asked, with
    /// the URL and the reply (`on gotJoke theURL, reply`).
    pub(super) fn fetch_later(&mut self, method: &str, url: String, body: Option<String>, message: String) -> R<()> {
        let req = self.web_request(method, url, body);
        let target = self.frame().me;
        let Some(events) = self.event_tx.clone() else {
            // No event loop (a script run on its own): reply straight away.
            let started = std::time::Instant::now();
            let reply = http::send(&req);
            let body = self.take_reply(reply, started.elapsed().as_secs_f64() * 1000.0);
            return self.send_message(target, &message, &[req.url, body]).map(|_| ());
        };
        std::thread::spawn(move || {
            let started = std::time::Instant::now();
            let reply = http::send(&req);
            let millis = started.elapsed().as_secs_f64() * 1000.0;
            let _ = events.send(super::world::Event::WebReply { target, message, url: req.url, reply, millis });
        });
        Ok(())
    }
}
