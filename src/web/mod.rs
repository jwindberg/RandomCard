//! The web, as a library: an HTTP client for REST services and JSON
//! reading, with nothing of HyperTalk in it. The language's side, `put url
//! …` and `key … of json …`, is in `runtime/web.rs`.

pub mod http;
pub mod json;
