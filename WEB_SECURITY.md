# Web security: connections, secrets and signing in

The plan for how stacks talk to web APIs that need credentials, and how
each user's credentials are kept. It grew out of item 8 of the web section
of [ROADMAP.md](ROADMAP.md), which now points here.

Web APIs secure themselves in many different ways. This is a big topic
and we'll keep finding more of it; the plan starts with what we know and
what's most common, and grows type by type.

## The idea

RandomCard is an authoring tool. We don't build what stack authors
build: we build their tools. A stack that talks to web services is an
application, like a web app that talks to other servers, and the tools
should feel like a web framework's: the app declares the services it
uses, a configuration supplies the values, and the framework handles the
security.

The tools are open and flexible, for authors who will have their own
ideas about how to use them, and shaped so the easy way is the good
practice: a declaration covers the common cases with no code, scripts can
reach in where an author needs more, and the safety rules hold whichever
way they go. That is the HyperCard way.

### Connections

A web app keeps its services' configuration as a hierarchy, by app, by
service, by element:

```yaml
tourStack:
  moon:
    type: key
    url: https://moon.com
    key: …
  space:
    type: oauth
    url: https://space.org
    authUrl: https://oauth.space.com/authorize
    tokenUrl: https://oauth.space.com/token
    clientId: …
    clientSecret: …
```

RandomCard's connections are the same hierarchy:

- **A stack names the connections it needs:** `moon`, `space`,
  `VideoDB`, as many as it uses.
- **Each connection has a type,** a helper built into RandomCard (None,
  Key, Basic, OAuth, and more to come). The type decides the connection's
  elements, and knows how to use them on every request, so an author
  writes no security code.
- **The elements of a type are always the same; what varies is who
  gives each value.** Some are constant for every user on the planet:
  Facebook's URL, endpoints and app id. Some differ by user: VideoDB's
  URL, user and password. The type is an element too: Facebook's is
  always OAuth, but one user's VideoDB server is open (None), another's
  uses Basic and a third's OAuth, and the same stack serves all three
  with no change to its scripts.
- **The author controls the config,** because it's all about context: a
  value constant for a stack's users may not be for the people building
  it, as a Facebook engineer points the same stack at `dev01` instead of
  `graph`. For each element the author decides:
  - **fixed:** the author's value, which no one changes;
  - **a default:** the author's value, which a user may change;
  - **open:** each user gives it.

  A stack can fix everything but the user's own sign-in, or leave
  everything open but the connection's name. Secrets are never the
  author's to give. The author can also declare **named contexts**, as
  Spring has profiles (`production`, `dev`), each with its own values,
  one in use at a time.
- **Each user's values are in their copy of the stack,** the way an
  address book is shared empty and each friend fills in their own.
  Secrets (passwords, keys, client secrets, tokens) are the exception:
  they go in the system's secure store (the macOS Keychain, Windows
  Credential Manager, the Secret Service on Linux), named plainly by
  stack, connection and setting (`VideoDB / videodb-basic / Password`),
  never in the stack. Sharing a filled copy shares its URLs and user
  names, never its secrets; emptying a copy before sharing it is the
  author's to provide, as it is for an address book.
- **Scripts use a connection by name:** `put url "/phases" with
  connection "enphase" into reply`. The type adds what the request needs:
  a header, a query key, Basic, a bearer token from a sign-in it keeps
  and refreshes. Scripts never see secrets or tokens.

### The URL

A connection's URL is its base: the scheme (so nothing assumes http or
https), the host, the port and any path the app lives under, never used
alone. Each request adds the rest: its path, path variables and query.
With a URL of `https://host.com/videodb`, `put url "/movies/12?full=1"
with connection "VideoDB"` sends `https://host.com/videodb/movies/12?full=1`.
In a connection, URL always means this, so the element is just URL.
Services can sit on ports of their own (`http://media.local:9090`,
`:9191`) or on paths of one server (`http://media.local:8080/videodb`,
`/solar`); stacks don't care which.

### Who sets things up

How the values get there is the author's design, not ours: RandomCard has
no setup form or menu of its own. A stack builds a settings card from the
script tools, reads a file a company hands out, or anything else its
author likes. Which of a stack's connections is in use, when it has
several, is the author's design too. One person might write the
stack, an IT department fill in the company's server, and each user add
their own password: the tools allow that without RandomCard knowing
anything about it.

**Who owns the app credentials.** Some services give each customer their
own key, or let each sign in: the clean case. Others register the
author's app and give the author a client id and secret, which customers
then sign in through. Then either each user registers their own app and
enters its values (secure, more work for them), or the author's values
are handed to every user, and the service's limits may count every
user's calls against the author's app. Where a service allows public
clients, PKCE needs no secret at all; otherwise the usual answer is a
small server in between that holds the secret. The tools support all of
these; which one a stack uses is the author's call.

## The tools

1. **Connection types,** each a helper with its own elements, built from
   standard parts rather than a recipe per service. Every type has a URL.
   - **None:** just the URL. A server of one's own, with no security.
   - **Key:** a key, and where it goes: `Authorization: Bearer` (RFC
     6750), a header the service names, or a query parameter.
   - **Basic:** a user and a password (RFC 7617).
   - **OAuth:** the authorization and token endpoints, a client id and
     secret (or none, for a public client with PKCE, RFC 7636), the
     scopes, and how the sign-in comes back: a loopback address
     (`http://127.0.0.1:port`) or a code the user pastes. Tokens are kept
     and refreshed (RFC 6749). Sign-in happens in the system's own
     browser, never a web view inside RandomCard (RFC 8252).
   - **Later,** as we meet them: a login endpoint that returns a JWT (see
     below); the device flow (RFC 8628, for the cyberdeck); OpenID
     Connect, its provider found from `/.well-known/openid-configuration`;
     client credentials (an app acting as itself); client certificates
     (mutual TLS, RFC 8705); signed JWT assertions (RFC 7523).
   - **Extras on any type:** elements of the author's own beyond the
     type's (a tenant id, which of a user's systems to show), and headers
     sent on every request under the URL (`Accept`, a service's version
     header, `X-Tenant`), each a fixed value or an element's. They carry
     no security: that's the type's job.
   - **Vendors' variations,** where a service bends a standard, are
     elements of the type, not new types: Enphase wants a bearer token
     and a `?key=` on every call; Facebook's device sign-in is RFC 8628
     with its own endpoints and `appid|clienttoken` as the credential.
2. **The stored connections,** for each stack and each user, always
   there, so a stack works without any setup screens of its own.
3. **Script access** to them: which connections a stack declares and
   their types; read and set any element that isn't secret and the
   author didn't fix; set a secret but never read it; whether a
   connection is set up; which context is in use; start a sign-in and
   forget one. Enough for an author to build any setup they like. The
   syntax is to design on its own merits.
4. **Interceptors,** as messages, the way Spring's interceptors work: one
   sent before each request on a connection, so a handler can add or
   change headers and the query, and one after its reply, before the
   script that asked sees it. A handler can be in the card, background or
   stack, for every connection or one by name. It runs before the type
   adds the security, so it never sees secrets or tokens. A service's own
   request signing uses built-ins that sign with a secret by name (HMAC,
   SHA-256, base64), without handing it to the script. The message names
   are to design.
5. **No setup form of RandomCard's own.** A request on a connection that
   isn't set up isn't sent, and `the result` says what it needs; the
   stack decides what to show. Connections are a networking library for
   scripts, not parts for cards.
6. **Safety rules,** which aren't the author's to choose:
   - Credentials go only to requests under their connection's URL (same
     scheme, host and port, and whole path segments at or below its
     path), and tokens only to the hosts they were issued for.
   - Plain http only on this computer and the local network; https
     everywhere else, as the Mac app's ATS rule already says.
   - No redirects followed with credentials, and no URL with a key in it
     shown in an error.
   - Secrets and tokens belong to the URL they were given for: changing
     a connection's URL, or its context, never sends them to the new one.
   - Later, a safety net for stacks from strangers: a request to a host
     no connection declares asks first.

### JWT

JSON Web Tokens (RFC 7519) turn up in several places, and only some
need a type of their own:

- **A JWT handed to the user,** from a service's dashboard: the Key type,
  as a bearer token. Nothing new.
- **OAuth tokens that are JWTs:** the OAuth type, which reads the `exp`
  claim to refresh before it expires.
- **A login endpoint that returns one,** common in Spring apps: POST a
  user and password to a login path, get a JWT and maybe a refresh
  token, send it as a bearer token, refresh at a refresh path. Neither
  OAuth nor Basic: likely a type of its own (Login: the login and
  refresh paths, a user and password, and where the token is in the
  reply).
- **Signed JWT assertions** (RFC 7523): the client signs a JWT with its
  own private key, kept in the secure store, to get a token, as Google's
  service accounts do. A type of its own, later.
- **Knowing when a token expires,** for every type that gets one
  (OAuth, Login): from the token reply's `expires_in`, or a JWT's `exp`
  claim when the token is one. A token that expires within a margin
  (time for the request to arrive, plus clock skew between the two
  computers) isn't used: a fresh one is fetched first, by its refresh
  token, or by signing in again. Requests made while one is being
  fetched wait for it instead of each fetching their own. A 401 anyway
  (a token revoked early) gets one fresh token and one retry, never a
  loop. The user never sees a token expire, and never copies one from
  one request into another, as Postman makes them do.
- **Claims:** a script might read a token's claims, such as who signed
  in, without seeing the token. Claims aren't secrets; to decide.

## The Tour and the examples

**The Tour explains; it doesn't configure.** Its two Web pages (REST
and JSON, then Connections) show what an author declares and what a
script says, in words and short examples. They never read, set or use a
stored connection, and never call a service. Working examples are stacks
of their own:

- **A server of one's own:** one stack for every user's own server, its
  type open: None for an open server, Basic or OAuth for a secured one.
- **Enphase:** a stack for Enphase solar customers on the Enlighten v4
  API: OAuth with a pasted code and the client authenticating with
  Basic, refresh before expiry, and every call carrying a bearer token
  and `?key=`. Today that's about 150 lines of a Java server; it should
  be a declaration.
- **Facebook:** its type, URL, endpoints and app id fixed by the author,
  so each user only signs in: Facebook Login for Devices (the client
  token is meant to be embedded) and OpenID Connect with PKCE; showing
  who signed in, with their picture, and a Page's recent posts for
  someone who manages one. Meta allows only an app's own admins,
  developers and testers in development mode; anyone else needs Meta's
  App Review.

**Done when** all three work against the real services for someone other
than their author, given only their own values, with no secret of the
author's in the stack file.

## The order

1. Connections and the secure store. *Built, as settings an author
   invents.*
2. Keys and Basic. *Built, as credential templates an author assembles.*
3. **Typed connections:** rebuild 1 and 2 around types; see below.
4. OAuth with refresh, by loopback and by pasted code; then Enphase.
5. The device flow and OpenID Connect; then Facebook.
6. Login with a JWT, client credentials, client certificates, JWT
   assertions and request signing, as stacks need them.
7. The safety net for stacks from strangers.

## Step 3: typed connections

Each part is built, tried and looked at before the next.

- **3a. The model.** A connection is a name, a description, and its
  elements, each with the author's value (if any) and whether it's
  fixed, a default or open; the type is one of them. The types None, Key
  and Basic, each signing its requests; OAuth's elements declared, its
  sign-in left for step 4. Extras: elements of the author's own and
  fixed headers. The element is URL, not Address. The credential
  templates go. *Built 2026-11-23, with Stack Info ▸ Connections…
  showing Fixed, Default or Open for each element, for 3d to look over.*
- **3b. The store.** *Decided 2026-11-23:* users' values are saved in
  the stack, secrets in the secure store under the stack's, connection's
  and setting's names; no `connections.json`, no stack identity, no File
  ▸ Connections… and no setup form. *Built the same day.* Still to do:
  the rule that secrets belong to their URL. Named contexts are on hold:
  a stack with several servers declares a connection for each, and
  chooses the one in use itself.
- **3c. HyperTalk.** `setting "URL" of connection "VideoDB"` and `set
  setting …` keep working. *Built 2026-11-23, for the example stack's
  own editor:* `the connections`, `create connection "name"` and `delete
  connection "name"` (which forgets its secrets too). Still to do:
  whether a connection is set up, a setting's hint, choices and who
  gives it; the interceptor messages. Syntax shown for a decision before
  it's built.
- **3d. The window.** Stack Info ▸ Connections…: name, type, and each
  element's value and whether it's fixed, a default or open.
- **3e. The stacks.** The example for a server of one's own: its type and
  URL open, so each user picks None or Basic. The Tour: no connection of its own; Web page 2 explains,
  with no buttons that touch a stored connection. HYPERTALK.md to match.

## What's built (steps 1 and 2)

Built 2026-11-22, and kept where the new design agrees:

- **The secure store:** secrets through the `keyring-core` crate in each
  system's secure store.
  Where none is running, RandomCard says so and keeps a secret only until
  it quits. The Linux Secret Service is tried; the Keychain and
  Credential Manager are built but not yet run on their systems.
- **HyperTalk:** `setting "Address" of connection "VideoDB"` reads a value,
  `set setting … of connection … to x` saves one (a secret can be set,
  never read), and `with connection "VideoDB"` on `url`, every request
  command and `load url` joins a path to the connection's URL.
- **Signing:** credentials go on any request under the connection's URL,
  named or not, including image parts and movies. A request that names
  its connection is signed by that one, so several can share a server. Movies sign in on Linux
  (GStreamer's HTTP source gets the headers) and on a Mac (an
  AVAssetResourceLoaderDelegate fetches each range with RandomCard's HTTP
  client); Windows says it can't yet. A live (HLS) stream that signs in
  doesn't play on a Mac yet.
- **The safety rules** above, except that secrets don't yet belong to
  their URL.

What step 3 replaces: an author inventing a connection's settings and
assembling its credentials from templates (`Bearer {Token}`, `{User
name}:{Password}`) in Stack Info ▸ Connections…; and the name Address.

## Testing

Each type is tried against a back end of our own, configured once for
each way a service can be secured (open, Basic, a key, a JWT login,
OAuth), with the same example stack run against every one and only the
user's values changing. Tokens there can be set to expire in seconds, so
refreshing before expiry, one refresh at a time and the retry on a 401
are all tried on purpose rather than waited for.
