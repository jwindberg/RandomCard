//! The services a stack talks to, as its author declares them.
//!
//! A stack is an application people share, and each person who uses it
//! brings their own settings: the address of their server, their user name,
//! their key. So a connection has two halves. This is the author's half,
//! saved in the stack: a name, what it's for, and the settings a user gives.
//! The user's half, the values, is never in the stack: see
//! `runtime::connections`.

use serde::{Deserialize, Serialize};

/// The setting every connection has: where the service is.
pub const ADDRESS: &str = "Address";

/// A service the stack talks to.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Connection {
    /// How scripts name it: `with connection "VideoDB"`.
    pub name: String,
    /// What it is, shown to the user who sets it up.
    pub description: String,
    /// What a user gives, in the order they're asked: Address first.
    pub settings: Vec<Setting>,
    /// How each request to the Address proves who's asking, made from the
    /// settings: Basic, a header, a query parameter, or more than one.
    pub credentials: Vec<Credential>,
}

/// Where a credential goes on each request.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Place {
    /// HTTP Basic (RFC 7617): the value is `user:password`, sent as
    /// `Authorization: Basic` and its base64.
    #[default]
    Basic,
    /// A header: `Authorization` with `Bearer {Token}` (RFC 6750), or one
    /// the service names, such as `X-API-Key`.
    Header,
    /// A query parameter, such as `key`, added to the address.
    Query,
}

/// One proof sent with each request. Its value is text with settings in
/// braces, `{User name}:{Password}` or `Bearer {API Key}`, filled in from
/// the user's values when the request is sent; scripts never see it.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Credential {
    pub place: Place,
    /// The header's or query parameter's name; none for Basic.
    pub name: String,
    pub value: String,
}

/// One thing a user gives for a connection.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Setting {
    pub name: String,
    /// A hint shown beside its box ("Your account's API key").
    pub hint: String,
    /// The value until the user gives one, such as a usual address. Never
    /// for a secret: a default would be in the stack for anyone to read.
    pub default: String,
    /// Kept in the system's secure store, never shown again once saved,
    /// and never given to scripts.
    pub secret: bool,
    /// The connection is set up without it; a credential that uses it is
    /// left off while it's empty (a key some people have, for more calls).
    pub optional: bool,
}

impl Connection {
    /// A new connection, with its Address.
    pub fn new(name: &str) -> Connection {
        Connection {
            name: name.to_string(),
            description: String::new(),
            settings: vec![Setting { name: ADDRESS.into(), ..Setting::default() }],
            credentials: Vec::new(),
        }
    }

    /// HTTP Basic from a user name and a secret password, the usual way.
    pub fn add_basic(&mut self) {
        for (name, secret) in [("User name", false), ("Password", true)] {
            if self.setting(name).is_none() {
                self.settings.push(Setting { name: name.into(), secret, ..Setting::default() });
            }
        }
        self.credentials.push(Credential { place: Place::Basic, name: String::new(), value: "{User name}:{Password}".into() });
    }

    /// A setting by name, ignoring case.
    pub fn setting(&self, name: &str) -> Option<&Setting> {
        self.settings.iter().find(|s| s.name.eq_ignore_ascii_case(name.trim()))
    }
}

/// The settings a credential's value names, in braces, in order.
pub fn named_settings(value: &str) -> Vec<&str> {
    let mut names = Vec::new();
    let mut rest = value;
    while let Some(open) = rest.find('{') {
        let Some(close) = rest[open..].find('}') else { break };
        names.push(rest[open + 1..open + close].trim());
        rest = &rest[open + close + 1..];
    }
    names
}

/// A new stack's lasting identity: 128 random bits, in hex. Users' settings
/// for a stack's connections are kept under it, so they follow the stack
/// when its file is moved or renamed. A copy of a stack keeps it: it's the
/// same application.
pub fn new_uid() -> String {
    format!("{:032x}", rand::random::<u128>())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_connection_has_an_address() {
        let c = Connection::new("VideoDB");
        assert_eq!(c.settings.len(), 1);
        assert!(c.setting("address").is_some() && c.setting(" ADDRESS ").is_some());
        assert!(c.setting("Password").is_none());
    }

    #[test]
    fn basic_and_its_settings() {
        let mut c = Connection::new("VideoDB");
        c.add_basic();
        assert_eq!(c.settings.iter().map(|s| (s.name.as_str(), s.secret)).collect::<Vec<_>>(), [
            ("Address", false),
            ("User name", false),
            ("Password", true)
        ]);
        assert_eq!(named_settings(&c.credentials[0].value), ["User name", "Password"]);
        assert_eq!(named_settings("Bearer { Token }"), ["Token"]);
        assert!(named_settings("no braces {").is_empty());
    }

    #[test]
    fn stack_ids_differ() {
        let (a, b) = (new_uid(), new_uid());
        assert_eq!(a.len(), 32);
        assert_ne!(a, b);
    }
}
