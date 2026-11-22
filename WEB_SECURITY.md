# Web security: connections, secrets and signing in

The plan for how stacks talk to services that need credentials, and how
each user's credentials are kept. It grew out of item 8 of the web section
of [ROADMAP.md](ROADMAP.md), which now points here.

## The idea

A stack is an application that people share, and each person brings
their own settings, as a web app's customers each bring their own
database. The author writes the
stack against a service, using RandomCard's tools for whatever security
that service uses; anyone else with an account there uses the stack by
giving it their own credentials. So RandomCard's job isn't to decide
what stacks may do: it's to give authors good tools, the way a web
framework gives you an OAuth client, and keep each user's secrets out
of the stack.

**Connections.** A stack declares each service it talks to, in two
halves:
- **The author's half, in the stack:** the service's address, how its
  security works (which scheme, its endpoints and scopes), and where
  the credentials go on each request.
- **Each user's half, never in the stack:** their credentials or their
  sign-in, and settings such as which of their systems to show. Given
  in a setup dialog RandomCard makes from the declaration (or the
  stack's own settings card), or a settings file, so an IT department
  can hand out a stack with its settings ready. Ordinary settings are
  kept in RandomCard's settings, secrets (passwords, keys, client
  secrets, tokens) in the system's secure store: the macOS Keychain,
  Windows Credential Manager, the Secret Service on Linux. Sharing a
  stack shares only its code.
- **Environments:** a connection can have several sets of settings (a
  service's sandbox while building, production after), chosen without
  editing the stack.
- **In scripts,** requests name the connection, something like
  `put url U with connection "Enphase" into reply`, and RandomCard
  adds the credentials. Scripts never see secrets or tokens, so a stack
  can't send them anywhere else; tokens go only to the hosts they were
  issued for. The syntax is to design on its own merits.

**The security schemes,** built from standard parts rather than a
recipe per service, so a new service is a declaration, not a new
RandomCard:
- **How a user proves who they are:** an API key; HTTP Basic (RFC
  7617); OAuth 2.0 (RFC 6749) with the authorization code flow, with or
  without PKCE (RFC 7636), client credentials (an app acting as itself)
  and refresh tokens; the device flow (RFC 8628); OpenID Connect for
  single sign-on (Okta, Microsoft Entra, Google, Facebook), the
  provider found from its `/.well-known/openid-configuration`; client
  certificates (mutual TLS, RFC 8705); signed JWT assertions (RFC
  7523).
- **Where the proof goes:** a header (`Authorization: Bearer`, RFC
  6750, or one the service names), a query parameter, or Basic, and
  more than one at once (Enphase wants a bearer token and an API key).
- **How a sign-in comes back:** sign-in always happens in the system's
  own browser, never a web view inside RandomCard (RFC 8252); it comes
  back through a loopback address (`http://127.0.0.1:port`), a code
  the user pastes (Enphase's way), or a device code entered on another
  device (the cyberdeck's way).
- **Vendors' variations on a standard,** described in the declaration:
  Facebook's device sign-in is RFC 8628 in spirit, with its own
  endpoints, `appid|clienttoken` as the credential and a poll every 5
  seconds.
- **For schemes nobody standardized** (a service's own request
  signing), a connection can name a handler in the stack that
  RandomCard calls before each request, with HMAC, SHA-256 and base64
  built in. HTTP Message Signatures (RFC 9421) built in, if services
  come to use it.

**Who owns the app credentials.** Some services give each customer
their own key, or let each sign in: the clean case. Others register
the author's app and give the author a client id and secret, which
customers then sign in through. Then either each user registers their
own app and enters its values (secure, more work for them), or the
author's values ship in the stack, and the secret goes to every user,
and the service's limits may count every user's calls against the
author's app. Where a service allows public clients, PKCE needs no
secret at all (RFC 8252); otherwise the usual answer is a small server
in between that holds the secret. RandomCard supports all of these and
says plainly which is which.

**Three example stacks drive it,** each in `examples/` (the Tour never
calls outside services):
- **VideoDB:** a server of one's own, with no security; the connection
  is just its address, moved out of the stack file.
- **Enphase:** a stack for Enphase solar customers on the Enlighten v4
  API, which takes an API key, client id and secret from a registered
  app; the authorization code flow with a pasted code and the client
  authenticating with Basic; refresh before expiry; and every call
  carrying a bearer token and `?key=`. Today that's about 150 lines of
  a Java server; it should be a declaration.
- **Facebook:** sign-in with Facebook Login for Devices (nothing secret
  in the stack: the client token is meant to be embedded) and OpenID
  Connect with PKCE; showing who signed in, with their picture in an
  image part, and a Page's recent posts for someone who manages one.
  Meta allows only an app's own admins, developers and testers in
  development mode; anyone else needs Meta's App Review.

**Done when** all three stacks work against the real services: VideoDB
against its own server, Enphase showing a customer's solar production,
and Facebook showing who signed in. Each works for someone other than
its author, given only their own settings, with no secret of the
author's in the stack file. They're the proof the tools are general.

**The order:**
1. Connections and their settings, with the secure store; VideoDB's
   address moves out of its stack.
2. API keys and Basic, in headers and query parameters.
3. OAuth: the authorization code flow with refresh, by loopback and by
   pasted code; then Enphase.
4. The device flow and OpenID Connect; then Facebook.
5. The signing handler, client credentials, client certificates and
   JWT assertions.
6. A safety net for stacks from strangers: a request to a host that no
   connection declares and the user never gave the stack asks first.

Meanwhile the Mac app allows plain http only on the local network
(`NSAllowsLocalNetworking`, so `http://trashcan.local` streams), says
why in its Local Network prompt, and needs https everywhere else; the
same rule should hold on Linux and Windows, so a stack behaves alike
everywhere.

## Step 1: connections and their settings

Decided and built 2026-11-22. The Linux Secret Service is tried; the
Keychain and Credential Manager are built but not yet run on their systems.

- **Each stack has a lasting identity** (`uid` in the stack file: 128
  random bits). A new stack gets one when it's made; a stack from before
  gets one when it's opened, kept once it's saved. Users' values are kept
  under it, so they follow the stack when its file moves or is renamed. A
  copy keeps it: it's the same application.
- **The author's half** is saved in the stack: each connection's name, its
  description, and its settings, each with a hint, a default (never for a
  secret) and whether it's secret. Every connection has an Address.
- **The user's half** is kept on their computer: ordinary values in
  `connections.json` in RandomCard's settings folder, under the stack's
  identity and the connection's name; secrets in the system's secure store
  (the `keyring-core` crate with each system's own store: the Keychain,
  Windows Credential Manager, the Secret Service over D-Bus in Rust). Where
  a system has no secure store running, RandomCard says so and keeps the
  secret only until it quits; it never writes secrets to a plain file.
- **In HyperTalk:**
  - `setting "Address" of connection "VideoDB"` reads a user's value (or
    the setting's default), and `set setting … of connection … to x`
    saves one, to the user's computer, never the stack, so a stack's own
    settings card can do the setting up. A secret can be set but never
    read: reading one is an error.
  - `url "/graphql" with connection "VideoDB"`, and `with connection` on
    `post`, `put … to url`, `patch`, `query`, `delete url`, `request` and
    `load url`: a path is taken from the connection's Address. In step 2
    the same words attach the credentials, so scripts written now keep
    working when a service adds security.
  - A connection that isn't set up opens RandomCard's setup dialog, made
    from the declaration, once a run: cancelled, the request is like one
    with no reply, and `the result` says what's missing. File >
    Connections… opens the same form any time. An Address typed without a
    scheme gets https, or http for this computer (localhost, 127.0.0.1,
    [::1]), as browsers do.
  - A settings file (JSON: settings by connection) fills values in, from
    Open Settings File… in either window; secrets in it go to the secure
    store, and the file is only read.
- **The author declares connections** in Stack Info's Connections… window.
- **VideoDB is the proof:** its server address moves out of the stack file
  into a connection; its scripts use `with connection "VideoDB"`, and its
  Settings card reads and sets the Address. The Tour's Connections card
  shows it with a server on the user's own computer.
- **Movies and images** keep their own forms with no `with connection`: a
  movie's address is made from `setting "Address" of connection …`. In
  step 2, credentials are added to any request whose address starts with
  a connection's Address, which covers them too.

## Step 2: API keys and Basic

Built 2026-11-22; movies still to do.

- **A connection's credentials,** declared under "How it signs in" in its
  Connections… window: each is Basic, a header or a query parameter, with
  a value naming settings in braces (`{User name}:{Password}`, `Bearer
  {Token}`, `{API Key}`). Add Basic adds a User name and a secret Password.
- **Which requests carry them:** any from the stack whose address is under
  the connection's Address (scheme, host, port, path prefix; the closest
  Address wins), named or not: `url`, every request command, and image
  parts. Movies are next, in each system's player.
- **Kept safe:** filled in only as the request is sent, so scripts never see
  them; no redirects followed with them; plain http only on this computer
  and the local network (as the Mac app's ATS rule already says);
  errors never show a URL with a key in it.
- **Optional settings:** not needed to be set up; a credential using an
  empty one is left off. VideoDB's User name and Password are optional, so
  it works with an open server and with the `basic-auth` branch.
- **Tried against** the `basic-auth` branch of videodb-be on
  localhost:8080: 401 without them, the movie list with them.

## Driving it with VideoDB

VideoDB's server (videodb-be, Spring Boot) and front end (videodb-fe,
React) gain security on branches of their own, one for each scheme,
built and run here: HTTP Basic for step 2, then an OAuth 2.0 resource
server with an authorization server (Spring Authorization Server or
Keycloak) for step 3, the React app and RandomCard each signing in as a
public client with PKCE. The stack follows each branch with a change to
its connection's declaration only, never its scripts.
