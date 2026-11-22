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
}

impl Connection {
    /// A new connection, with its Address.
    pub fn new(name: &str) -> Connection {
        Connection {
            name: name.to_string(),
            description: String::new(),
            settings: vec![Setting { name: ADDRESS.into(), ..Setting::default() }],
        }
    }

    /// A setting by name, ignoring case.
    pub fn setting(&self, name: &str) -> Option<&Setting> {
        self.settings.iter().find(|s| s.name.eq_ignore_ascii_case(name.trim()))
    }
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
    fn stack_ids_differ() {
        let (a, b) = (new_uid(), new_uid());
        assert_eq!(a.len(), 32);
        assert_ne!(a, b);
    }
}
