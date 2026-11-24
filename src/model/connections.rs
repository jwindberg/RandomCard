//! The services a stack talks to, as its author declares them.
//!
//! A stack is an application people share, and each person who uses it
//! brings their own values: the URL of their server, their user name, their
//! key. Like an address book shared empty and filled in by each friend, a
//! connection has the author's half (a name, what it's for, and for each of
//! its elements whether the author's value is fixed, a default, or left
//! open) and each user's values, both saved in the stack. Secrets are the
//! exception: they go in the system's secure store, never in the stack
//! (see `runtime::connections`).
//!
//! Each connection has a type, a helper built into RandomCard (None, Key,
//! Basic, OAuth), which decides its elements and how a request signs in.
//! The type is an element too, so it can be fixed by the author or left to
//! each user: one person's server is open, another's uses Basic.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

/// The element every connection has for its type.
pub const TYPE: &str = "Type";
/// The element every connection has for its base: the scheme, host, port
/// and any path, to which each request adds its own path and query.
pub const URL: &str = "URL";

/// The element every connection has for headers sent on each request under
/// its URL: lines of `Name: value`, as `the httpHeaders` has them, such as a
/// service's `X-Client-App`. A value may hold dynamic values, filled in
/// afresh for each request (see `runtime::connections::dynamic`).
pub const HEADERS: &str = "Headers";

/// The element every connection has for whether its type signs in over
/// plain http beyond this computer and the local network: by default not,
/// but a server that only speaks http can be reached.
pub const OVER_HTTP: &str = "Sign in over http";
pub const LOCAL_ONLY: &str = "Local network only";
pub const ANYWHERE: &str = "Anywhere";
/// The element every connection has for whether scripts can read its
/// secrets: by default not, but a stack that signs requests its own way
/// needs them.
pub const READ_SECRETS: &str = "Scripts read secrets";

/// A connection's type: how its requests sign in.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Type {
    /// Just the URL: a server of one's own, with no security.
    #[default]
    None,
    /// A key, sent as `Authorization: Bearer` (RFC 6750), a header the
    /// service names, or a query parameter.
    Key,
    /// A user name and a password (RFC 7617).
    Basic,
    /// OAuth 2.0 (RFC 6749): a token from the provider, sent as
    /// `Authorization: Bearer` and refreshed as it expires.
    OAuth,
}

/// One element a type has.
#[derive(Debug)]
pub struct Def {
    pub name: &'static str,
    /// Shown beside its box, unless the author gives a hint of their own.
    pub hint: &'static str,
    /// Kept in the system's secure store, never the author's to give, and
    /// never given to scripts.
    pub secret: bool,
    /// The connection is set up without it.
    pub optional: bool,
    /// The values it takes, when it's a choice; the first is its value
    /// until someone gives one.
    pub choices: &'static [&'static str],
}

const fn def(name: &'static str, hint: &'static str) -> Def {
    Def { name, hint, secret: false, optional: false, choices: &[] }
}

/// How a Key is sent.
pub const SEND_AS: &str = "Send as";
pub const BEARER: &str = "Bearer";
pub const HEADER: &str = "Header";
pub const QUERY: &str = "Query";
/// The header's or query parameter's name, for a Key not sent as Bearer.
pub const KEY_NAME: &str = "Key name";

/// How an OAuth connection gets its tokens: for now, by a user name and
/// password the stack hands over (RFC 6749 §4.3).
pub const GRANT: &str = "Grant";
pub const PASSWORD_GRANT: &str = "Password";
/// The OAuth provider (the issuer, in OpenID Connect): its token URL is
/// found from its `/.well-known/openid-configuration`.
pub const PROVIDER_URL: &str = "Provider URL";
pub const TOKEN_URL: &str = "Token URL";
pub const CLIENT_ID: &str = "Client ID";
pub const CLIENT_SECRET: &str = "Client secret";
pub const SCOPES: &str = "Scopes";
/// What RandomCard keeps for an OAuth connection once it's signed in: its
/// tokens, secrets in the secure store, and when the access token expires
/// and whether it's signed in, which any script can read.
pub const ACCESS_TOKEN: &str = "Access token";
pub const REFRESH_TOKEN: &str = "Refresh token";
pub const TOKEN_EXPIRES: &str = "Token expires";
pub const SIGNED_IN: &str = "Signed in";

const TYPE_DEF: Def = Def { choices: &["None", "Key", "Basic", "OAuth"], ..def(TYPE, "How it signs in") };
const URL_DEF: Def = def(URL, "Such as https://api.example.com/v2");
const HEADERS_DEF: Def = Def { optional: true, ..def(HEADERS, "One a line, Name: value") };
const OVER_HTTP_DEF: Def =
    Def { optional: true, choices: &[LOCAL_ONLY, ANYWHERE], ..def(OVER_HTTP, "Where the type may sign in over plain http") };
const READ_SECRETS_DEF: Def =
    Def { optional: true, choices: &["No", "Yes"], ..def(READ_SECRETS, "Whether scripts can read its secrets") };
/// The elements every connection has, whatever its type, after its type's.
const COMMON: [&Def; 3] = [&HEADERS_DEF, &OVER_HTTP_DEF, &READ_SECRETS_DEF];
const KEY: &[Def] = &[
    Def { secret: true, ..def("Key", "The key the service gave you") },
    Def { choices: &[BEARER, HEADER, QUERY], ..def(SEND_AS, "Bearer, a header, or a query parameter") },
    Def { optional: true, ..def(KEY_NAME, "The header's or parameter's name, such as X-API-Key or key") },
];
const BASIC: &[Def] = &[def("User name", ""), Def { secret: true, ..def("Password", "") }];
const OAUTH: &[Def] = &[
    Def { choices: &[PASSWORD_GRANT], ..def(GRANT, "How it gets a token") },
    Def { optional: true, ..def(PROVIDER_URL, "The provider (issuer), whose token URL is found from it") },
    Def { optional: true, ..def(TOKEN_URL, "Where tokens come from, when the provider doesn't say") },
    def(CLIENT_ID, "The app's id, from the provider"),
    Def { secret: true, optional: true, ..def(CLIENT_SECRET, "None for a public client") },
    Def { optional: true, ..def(SCOPES, "What the app asks to do, separated by spaces") },
];

impl Type {
    pub const ALL: [Type; 4] = [Type::None, Type::Key, Type::Basic, Type::OAuth];

    pub fn name(self) -> &'static str {
        match self {
            Type::None => "None",
            Type::Key => "Key",
            Type::Basic => "Basic",
            Type::OAuth => "OAuth",
        }
    }

    /// A type by name, ignoring case.
    pub fn parse(s: &str) -> Option<Type> {
        Type::ALL.into_iter().find(|t| t.name().eq_ignore_ascii_case(s.trim()))
    }

    /// The type's own elements, beyond Type and URL.
    pub fn elements(self) -> &'static [Def] {
        match self {
            Type::None => &[],
            Type::Key => KEY,
            Type::Basic => BASIC,
            Type::OAuth => OAUTH,
        }
    }
}

/// Who gives an element's value.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Given {
    /// The author's value, which no one changes.
    Fixed,
    /// The author's value, which a user may change.
    Default,
    /// Each user gives it.
    #[default]
    Open,
}

impl Given {
    pub const ALL: [Given; 3] = [Given::Fixed, Given::Default, Given::Open];

    pub fn name(self) -> &'static str {
        match self {
            Given::Fixed => "Fixed",
            Given::Default => "Default",
            Given::Open => "Open",
        }
    }
}

/// The author's word on one element: a type's, or one of the author's own
/// (an extra, such as a tenant id). An element of a type that isn't listed
/// is open, with the type's hint.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Element {
    pub name: String,
    /// The author's value, when it's fixed or a default. Never for a secret:
    /// it would be in the stack for anyone to read.
    pub value: String,
    pub given: Given,
    /// A hint of the author's own, shown beside its box.
    pub hint: String,
    /// For an extra: kept in the secure store, never given to scripts.
    pub secret: bool,
    /// For an extra: the connection is set up without it.
    pub optional: bool,
}

/// A header sent on every request under the connection's URL (`Accept`, a
/// service's version header): a fixed value, or an element's. Headers carry
/// no security: that's the type's job.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Header {
    pub name: String,
    pub value: String,
    /// The element whose value it sends, in place of `value`.
    pub element: String,
}

/// A service the stack talks to.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Connection {
    /// How scripts name it: `with connection "VideoDB"`.
    pub name: String,
    /// What it is, shown to the user who sets it up.
    pub description: String,
    /// The author's word on its elements: Type and URL, the types', and
    /// extras of the author's own.
    pub elements: Vec<Element>,
    pub headers: Vec<Header>,
    /// The values this copy's user has given, by element name in lower
    /// case: never a secret's.
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub values: BTreeMap<String, String>,
}

/// An element as a connection has it: the type's definition (or the
/// extra's) with the author's word on it.
#[derive(Clone, Debug, PartialEq)]
pub struct Field {
    pub name: String,
    pub hint: String,
    pub secret: bool,
    pub optional: bool,
    pub choices: &'static [&'static str],
    pub given: Given,
    /// The author's value; empty for a secret, and for one left open.
    pub value: String,
    /// One of the author's own, not a type's.
    pub extra: bool,
}

impl Field {
    /// The value until someone gives one: the author's, or a choice's first.
    pub fn fallback(&self) -> &str {
        if !self.value.is_empty() { &self.value } else { self.choices.first().copied().unwrap_or("") }
    }
}

/// Whether a name is one of any type's elements, Type and URL included: an
/// extra can't take it.
pub fn reserved(name: &str) -> bool {
    let n = name.trim();
    [TYPE, URL, HEADERS, OVER_HTTP, READ_SECRETS, ACCESS_TOKEN, REFRESH_TOKEN, TOKEN_EXPIRES, SIGNED_IN].iter().chain(Type::ALL.iter().flat_map(|t| t.elements().iter().map(|d| &d.name))).any(|r| r.eq_ignore_ascii_case(n))
}

impl Connection {
    /// A new connection: its type a default of None, its URL open.
    pub fn new(name: &str) -> Connection {
        Connection {
            name: name.to_string(),
            elements: vec![Element { name: TYPE.into(), value: "None".into(), given: Given::Default, ..Element::default() }],
            ..Connection::default()
        }
    }

    /// The author's word on an element, by name, ignoring case.
    pub fn element(&self, name: &str) -> Option<&Element> {
        self.elements.iter().find(|e| e.name.eq_ignore_ascii_case(name.trim()))
    }

    /// The author's word on an element, added open if there was none.
    pub fn element_mut(&mut self, name: &str) -> &mut Element {
        let i = match self.elements.iter().position(|e| e.name.eq_ignore_ascii_case(name.trim())) {
            Some(i) => i,
            None => {
                self.elements.push(Element { name: name.trim().to_string(), ..Element::default() });
                self.elements.len() - 1
            }
        };
        &mut self.elements[i]
    }

    /// Set the author's word on an element: who gives it, and the value.
    pub fn declare(&mut self, name: &str, given: Given, value: &str) {
        let e = self.element_mut(name);
        e.given = given;
        e.value = value.to_string();
    }

    /// The type the author fixed, if they did.
    pub fn fixed_type(&self) -> Option<Type> {
        self.element(TYPE).filter(|e| e.given == Given::Fixed).and_then(|e| Type::parse(&e.value))
    }

    /// The types a user may choose: the fixed one, or all of them.
    pub fn types(&self) -> Vec<Type> {
        match self.fixed_type() {
            Some(t) => vec![t],
            None => Type::ALL.to_vec(),
        }
    }

    fn field(&self, d: &Def) -> Field {
        let e = self.element(d.name);
        let given = if d.secret { Given::Open } else { e.map_or(Given::Open, |e| e.given) };
        Field {
            name: d.name.to_string(),
            hint: e.map(|e| e.hint.clone()).filter(|h| !h.is_empty()).unwrap_or_else(|| d.hint.to_string()),
            secret: d.secret,
            optional: d.optional,
            choices: d.choices,
            given,
            value: if given == Given::Open { String::new() } else { e.map(|e| e.value.clone()).unwrap_or_default() },
            extra: false,
        }
    }

    /// The author's own elements, beyond any type's.
    pub fn extras(&self) -> Vec<Field> {
        self.elements
            .iter()
            .filter(|e| !reserved(&e.name))
            .map(|e| {
                let given = if e.secret { Given::Open } else { e.given };
                Field {
                    name: e.name.clone(),
                    hint: e.hint.clone(),
                    secret: e.secret,
                    optional: e.optional,
                    choices: &[],
                    given,
                    value: if given == Given::Open { String::new() } else { e.value.clone() },
                    extra: true,
                }
            })
            .collect()
    }

    /// The elements a connection of type `t` has, in the order they're
    /// asked: Type, URL, the type's, Headers, then the author's own.
    pub fn fields(&self, t: Type) -> Vec<Field> {
        let mut out = vec![self.field(&TYPE_DEF), self.field(&URL_DEF)];
        out.extend(t.elements().iter().map(|d| self.field(d)));
        out.extend(COMMON.iter().map(|d| self.field(d)));
        out.extend(self.extras());
        out
    }

    /// An element by name, of any type the connection may have, or an
    /// extra: what a script may name.
    pub fn find(&self, name: &str) -> Option<Field> {
        let n = name.trim();
        [&TYPE_DEF, &URL_DEF]
            .into_iter()
            .chain(COMMON)
            .chain(self.types().into_iter().flat_map(|t| t.elements().iter()))
            .find(|d| d.name.eq_ignore_ascii_case(n))
            .map(|d| self.field(d))
            .or_else(|| self.extras().into_iter().find(|f| f.name.eq_ignore_ascii_case(n)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_new_connection() {
        let c = Connection::new("VideoDB");
        let f = c.fields(Type::None);
        assert_eq!(f.iter().map(|f| f.name.as_str()).collect::<Vec<_>>(), [TYPE, URL, HEADERS, OVER_HTTP, READ_SECRETS]);
        assert_eq!((f[3].fallback(), f[4].fallback()), (LOCAL_ONLY, "No"));
        assert_eq!((f[0].given, f[0].fallback()), (Given::Default, "None"));
        assert_eq!((f[1].given, f[1].fallback()), (Given::Open, ""));
        assert_eq!(c.types(), Type::ALL);
    }

    #[test]
    fn types_decide_the_elements() {
        let mut c = Connection::new("API");
        let names = |c: &Connection, t| c.fields(t).into_iter().map(|f| f.name).collect::<Vec<_>>();
        assert_eq!(names(&c, Type::Basic), [TYPE, URL, "User name", "Password", HEADERS, OVER_HTTP, READ_SECRETS]);
        assert_eq!(names(&c, Type::Key), [TYPE, URL, "Key", SEND_AS, KEY_NAME, HEADERS, OVER_HTTP, READ_SECRETS]);
        assert_eq!(c.fields(Type::Key)[3].fallback(), BEARER);
        assert_eq!(Type::parse(" oauth "), Some(Type::OAuth));
        // Any type's elements while the type is open; only its own once fixed.
        assert!(c.find("password").is_some() && c.find("client id").is_some());
        c.declare(TYPE, Given::Fixed, "Key");
        assert_eq!(c.types(), [Type::Key]);
        assert!(c.find("Password").is_none() && c.find("key name").is_some());
    }

    #[test]
    fn secrets_are_never_the_authors() {
        let mut c = Connection::new("API");
        c.declare("Password", Given::Fixed, "hunter2");
        let p = c.find("Password").unwrap();
        assert_eq!((p.given, p.value.as_str()), (Given::Open, ""));
        c.elements.push(Element { name: "Tenant".into(), value: "acme".into(), given: Given::Default, ..Element::default() });
        c.elements.push(Element { name: "Pin".into(), value: "1234".into(), given: Given::Fixed, secret: true, ..Element::default() });
        assert_eq!(c.find("tenant").unwrap().fallback(), "acme");
        assert_eq!(c.find("pin").unwrap().value, "");
        assert!(reserved("user NAME") && !reserved("Tenant"));
        assert_eq!(c.fields(Type::None).len(), 7);
    }
}
