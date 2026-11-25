# Talking to the web: a guide by example

HyperCard stacks lived on one computer. A RandomCard stack can talk to
services on the web: fetch a joke, look up a color, list a server's
movies, sign in to a company's system. This guide is for HyperTalk
developers who are new to all of that. Each section starts from
something you want to do and gives the whole recipe: what to set up, the
script, and what to do when it doesn't work.

The words are explained as they come up. [HYPERTALK.md](HYPERTALK.md) has
the full reference ("The web: REST and JSON"), and
[WEB_SECURITY.md](WEB_SECURITY.md) explains why things are built the way
they are.

**Contents**

- Part 1, the web without signing in
  1. [Get something from the web](#1-get-something-from-the-web)
  2. [Read the JSON that comes back](#2-read-the-json-that-comes-back)
  3. [Send something to a server](#3-send-something-to-a-server)
  4. [Keep the stack responsive while it waits](#4-keep-the-stack-responsive-while-it-waits)
  5. [Show pictures from the web](#5-show-pictures-from-the-web)
  6. [Ask a GraphQL server](#6-ask-a-graphql-server)
- Part 2, connections
  7. [Name a service once: your first connection](#7-name-a-service-once-your-first-connection)
  8. [Share a stack: who gives each setting](#8-share-a-stack-who-gives-each-setting)
  9. [Give each user a settings card](#9-give-each-user-a-settings-card)
- Part 3, signing in
  10. [A service that gives you a key](#10-a-service-that-gives-you-a-key)
  11. [A user name and password (Basic)](#11-a-user-name-and-password-basic)
  12. [OAuth: signing in with your stack's own sign-in card](#12-oauth-signing-in-with-your-stacks-own-sign-in-card)
  13. [OAuth: "Sign in with…" in the browser](#13-oauth-sign-in-with-in-the-browser)
  14. [Company sign-in and single sign-on](#14-company-sign-in-and-single-sign-on)
  15. [A service that signs in its own way](#15-a-service-that-signs-in-its-own-way)
- Part 4, the rest
  16. [Several servers: development, test and the real one](#16-several-servers-development-test-and-the-real-one)
  17. [A server on your own network, over http](#17-a-server-on-your-own-network-over-http)
  18. [When things go wrong](#18-when-things-go-wrong)
  19. [What not to do](#19-what-not-to-do)

---

## Part 1: the web without signing in

### 1. Get something from the web

A web service (a web API) is a server that answers addresses with data
instead of pages. You ask by fetching an address, as a browser does:

```hypertalk
on mouseUp
  put url "https://api.chucknorris.io/jokes/random" into reply
  if the result is not empty then
    answer "That didn't work:" && the result
    exit mouseUp
  end if
  put reply into field "Joke"
end mouseUp
```

- `put url … into x` fetches the address (an HTTP GET) and waits for the
  answer. `get url …` puts it into `it`.
- **Always check `the result`.** It's empty when it worked. Otherwise
  it's the server's status, such as `404 Not Found`, or what went wrong,
  such as no network. The server's reply is still in `reply` for you to
  read; servers often explain the problem there.
- A slow server holds the script until `the httpTimeout` (in seconds)
  runs out. Command-period (Ctrl-period) stops the wait.

The answer usually isn't text to show; it's JSON.

### 2. Read the JSON that comes back

JSON is how most services send data. It looks like this:

```json
{ "value": "Chuck Norris counted to infinity. Twice.",
  "categories": ["math"],
  "author": { "name": "Ada", "city": "London" } }
```

Curly braces hold **keys** and their values; square brackets hold a list
of **elements**. HyperTalk reads them with `key` and `element`:

```hypertalk
put key "value" of json reply into field "Joke"
put element 1 of key "categories" of json reply into theCategory
put key "city" of key "author" of json reply into theCity
put the number of elements of json key "categories" of json reply into n
put the keys of json reply into field "Keys"    -- one a line
put the pretty of json reply into field "Raw"   -- indented, for reading
```

Read a path from the inside out: `key "city" of key "author"` is the
author's city. Elements count from 1, and `last` works too. A key that
isn't there gives empty, so a script can test for it.

A list of results is read with a loop:

```hypertalk
put key "results" of json reply into movies
repeat with i = 1 to the number of elements of json movies
  put key "title" of element i of json movies & return after field "Titles"
end repeat
```

### 3. Send something to a server

Fetching is GET. To make or change something, a script sends a **body**,
usually JSON, with another method:

```hypertalk
on mouseUp
  put "{}" into body
  set key "name" of json body to field "Name"
  set key "email" of json body to field "Email"
  post body to url "https://httpbin.org/post"
  if the result is not empty then answer the result
  put the pretty of json it into field "Reply"
end mouseUp
```

- `post B to url U` makes something, `put B to url U` replaces it, `patch
  B to url U` changes part of it, and `delete url U` removes it. A
  service's documentation says which one each address wants. For a method
  with no command of its own, use `request "OPTIONS" to url U`.
- `set key … of json body` builds JSON in a variable, so you don't type
  quotes and commas by hand.
- **Values that look like numbers become numbers.** `set key "year" of
  json body to 1984` sends the number 1984. A title that happens to be
  "1984" needs to stay text: give it as a JSON string in quotes. The
  `jsonString` function in section 6 does that.
- A body goes as `application/json` when it's JSON. To send something
  else, say so: `set the httpHeaders to "Content-Type: text/csv"`.
- Some services want a web form instead of JSON: `formEncode` builds one.

**Headers** are extra lines that go with a request, such as what kind of
answer you want. Set them before the request; they go with every request
until changed:

```hypertalk
set the httpHeaders to "Accept: application/json" & return & "Accept-Language: en"
```

After a request, `the responseStatus`, `the responseHeaders` and `the
responseTime` (in milliseconds) say how it went.

### 4. Keep the stack responsive while it waits

`put url` waits, and the user can't do anything until the answer comes.
For something slow, or several requests at once, add `with message`: the
script goes on, and the answer comes later as a message to the same
object:

```hypertalk
on mouseUp
  put "Loading…" into field "Status"
  load url "https://api.chucknorris.io/jokes/random" with message "gotJoke"
end mouseUp

on gotJoke theURL, reply
  if the result is not empty then
    put "That didn't work:" && the result into field "Status"
    exit gotJoke
  end if
  put empty into field "Status"
  put key "value" of json reply into field "Joke"
end gotJoke
```

Every request takes `with message`: `post body to url U with message
"saved"` too. The handler gets the address and the reply, and `the result`
says how it went, as before.

### 5. Show pictures from the web

An image part shows a picture from an address. It loads in the
background, so the script never waits:

```hypertalk
put key "poster" of json movie into image "Poster"
set the fit of image "Poster" to fill
```

When the picture comes, the image is sent `imageLoaded`; when it can't
come, `imageFailed source, why`. Put a handler in the image's script to
show a placeholder or an error.

### 6. Ask a GraphQL server

Some services speak GraphQL: one address, and the question is in the
body. To HyperTalk it's a JSON post like any other:

```hypertalk
function graphQL query, vars
  put "{}" into body
  set key "query" of json body to jsonString(query)
  if vars is empty then put "{}" into vars
  set key "variables" of json body to vars
  post body to url "https://example.com/graphql"
  if the result is not empty then return "Error:" && the result
  if key "errors" of json it is not empty then
    return "Error:" && key "message" of element 1 of key "errors" of json it
  end if
  return key "data" of json it
end graphQL

-- Text as a JSON string, in quotes, so that "1984" stays text.
function jsonString t
  put empty into s
  repeat with i = 1 to the number of chars of t
    put char i of t into c
    put charToNum(c) into code
    if code is 34 then put "\" & quote after s
    else if code is 92 then put "\\" after s
    else if code is 10 or code is 13 then put "\n" after s
    else if code is 9 then put "\t" after s
    else put c after s
  end repeat
  return quote & s & quote
end jsonString
```

A GraphQL server answers 200 even when the question was wrong, so check
its `errors` as well as `the result`. (A built-in `graphql` command is on
the roadmap.)

---

## Part 2: connections

### 7. Name a service once: your first connection

Writing the full address in every script gets old, and it's wrong the day
the service moves. A **connection** names a service for the whole stack.
It's also where signing in lives (Part 3), so even an open service is
worth one.

1. Open **Stack Info** and click **Connections…**.
2. Click **New Connection**. Name it, say `Jokes`, and give a description
   ("The joke service").
3. Leave its **Type** as None (no sign-in) and give its **URL**, the
   base address every request starts with:
   `https://api.chucknorris.io`.

Now scripts name the connection and give only the rest of the path:

```hypertalk
put url "/jokes/random" with connection "Jokes" into reply
post body to url "/jokes" with connection "Jokes"
load url "/jokes/random" with connection "Jokes" with message "gotJoke"
```

A path is added to the connection's URL. A full address (`https://…`) is
used as it is.

### 8. Share a stack: who gives each setting

You'll give your stack to other people, and they may not use the same
server, or may have their own accounts. In the Connections window, every
setting has **Given** set to one of:

- **Fixed:** your value, the same for everyone. Use it for a public
  service's address, or the type when only one way of signing in makes
  sense.
- **Default:** your value, which each user may change. Use it for a
  server address most people share but some won't.
- **Open:** each user gives it. Use it for anything personal, like the
  address of their own server.

Secrets (passwords, keys, client secrets) are always each user's own.
They're never saved in the stack, so a stack is safe to share: each copy
holds only its own user's settings, and each user's secrets are in their
own computer's secure store (the Keychain on the Mac, Credential Manager
on Windows, the Secret Service on Linux).

The Type can be left open too. Then one stack serves a user whose server
needs no sign-in and another whose server needs a password.

You can add **settings of your own** to a connection, such as a tenant
name or which of a user's systems to show; they're kept and read just like
the built-in ones.

Every connection also has **Headers**: lines of `Name: value` sent with
every request to it, such as `Accept: application/json` or a version a
service asks for. A header value can include `{{$guid}}` for a fresh id
on each request, and `{{$timestamp}}` for the time.

### 9. Give each user a settings card

RandomCard doesn't show a setup form of its own: your stack decides what
its users see. A request on a connection that isn't set up isn't sent;
`the result` says what's missing, and your stack shows its settings card:

```hypertalk
on mouseUp
  put url "/videos" with connection "Movies" into reply
  if the result contains "isn't set up" then
    answer "Please set up the server first."
    go card "Settings"
    exit mouseUp
  end if
  ...
end mouseUp
```

The settings card reads and saves settings with `setting … of connection`:

```hypertalk
-- On the Settings card
on openCard
  put setting "URL" of connection "Movies" into field "Server"
  put setting "User name" of connection "Movies" into field "User name"
end openCard

-- The Save button
on mouseUp
  set setting "URL" of connection "Movies" to field "Server"
  if the result is not empty then answer the result -- why it was refused
  set setting "User name" of connection "Movies" to field "User name"
  set setting "Password" of connection "Movies" to field "Password"
  put empty into field "Password"
  answer "Saved."
end mouseUp
```

- Make the password field a **secure text** field (Field Info ▸ Secure
  Text): it shows dots, can't be copied, and Find skips it. Empty it after
  saving, so it isn't saved in the stack.
- A setting that can't be set (a Fixed one, or not one of a choice's
  values) leaves the script running and `the result` says why. Show it.
- `set setting "URL" of connection "Movies" to empty` goes back to your
  default.
- Give the card a **Test** button that makes one harmless request and
  shows the result in a dialog, so users find out at once whether it
  works.

---

## Part 3: signing in

Most services want to know who's asking. How they ask differs, and the
connection's **Type** says which way. Whatever the type, the rule is the
same: **your scripts never handle the credential on each request.** The
connection adds it to every request under its URL, and to nothing else,
so a secret can't leak to another server by mistake.

### 10. A service that gives you a key

Many services hand out an **API key**: a long string you get from the
service's website, which identifies your account.

1. Set the connection's Type to **Key**.
2. The service's documentation says how the key is sent. Set **Send as**:
   - **Bearer:** as `Authorization: Bearer <key>`, the standard way.
   - **Header:** in a header of the service's own; give its name in **Key
     name**, such as `X-API-Key`.
   - **Query:** in the address, as `?key=…`; give the parameter's name in
     **Key name**.
3. Leave **Key** to each user: on the settings card, a secure text field
   and `set setting "Key" of connection "Weather" to field "Key"`.

Requests need nothing more: `put url "/forecast?city=Paris" with
connection "Weather" into reply`.

### 11. A user name and password (Basic)

Some servers, often your own or your company's, want a user name and
password on every request (HTTP Basic authentication).

1. Set the connection's Type to **Basic**.
2. On the settings card, set **User name** and **Password** (section 9).

That's all: each request carries them. Basic sends the password with
every request, so a Basic connection only signs in over https, apart from
your own computer and local network (section 17).

### 12. OAuth: signing in with your stack's own sign-in card

Bigger systems don't take a password on every request. You sign in once
at an **identity provider** (Keycloak, Okta, Microsoft Entra ID, Auth0, a
service's own) and get a **token**, a pass that expires after a few
minutes. Requests carry the token, and when it runs out a **refresh
token** gets a new one without asking the user again. This is OAuth.

RandomCard does all of the token work. Your stack only signs in.

**What to ask the provider's administrator for** (or set up yourself):

- The provider's address, its **issuer**, such as
  `https://login.example.com/realms/movies`.
- A **client**: the provider's record of your app. Ask for a *public*
  client (an app on people's computers can't keep a secret) with "direct
  access grants" (the Password grant) allowed. Its **Client ID** is what
  you need.

**The connection:**

| Setting | Value |
|---|---|
| Type | OAuth |
| URL | the service's own address, such as `https://movies.example.com` |
| Grant | Password |
| Provider URL | the issuer |
| Client ID | your client's id |
| Scopes | `openid offline_access` (ask the administrator; `offline_access` keeps the user signed in across days) |

Make those Fixed or Default, as suits your users (section 8).

**The sign-in card:** fields for the user name and password (secure
text), and a Sign In button:

```hypertalk
on mouseUp
  sign in connection "Movies" as field "User name" with password field "Password"
  put empty into field "Password"
  if the result is not empty then
    answer "Couldn't sign in:" && the result  -- such as "Invalid user credentials"
    exit mouseUp
  end if
  go card "Movies"
end mouseUp
```

And a Sign Out button: `sign out connection "Movies"`.

RandomCard sends the password to the provider once and keeps only the
tokens, in the secure store, so the user stays signed in after quitting.
Where your stack keeps the password, if anywhere, is up to you. Not
keeping it is safest: the refresh token keeps the user signed in, and
when that runs out too, the stack asks again.

**Knowing whether the user is signed in:**

```hypertalk
if not setting "Signed in" of connection "Movies" then go card "Sign In"
```

`setting "Token expires"` says when the current token expires, counted as
`the seconds` counts; you never need to act on it, since RandomCard
refreshes the token itself.

The Password grant is the simplest kind of OAuth, but the provider never
gets to show its own sign-in page, so it can't ask for a second factor.
Many providers turn it off. Section 13 is the modern way.

### 13. OAuth: "Sign in with…" in the browser

With the **Authorization code** grant, the user signs in on the
provider's own page, in their usual web browser: with a password, a
passkey, a code from their phone, whatever the provider asks. Your stack
never sees any of it. This is how "Sign in with Google" works in desktop
apps.

**What to ask the administrator for:** a *public* client with the
"standard flow" (authorization code) allowed, **PKCE** required with
method S256, and the redirect URI `http://127.0.0.1/*`. (After signing
in, the provider sends the browser back to RandomCard, which listens
once on this computer, on a port of its own. PKCE makes sure a code that
leaks on the way is no use to anyone else.)

**The connection:** as in section 12, with **Grant** set to
`Authorization code`.

**The sign-in card:** no user name or password fields. Just a button:

```hypertalk
on mouseUp
  put "Signing in, in your browser…" into field "Status"
  sign in connection "Movies"
  if the result is not empty then
    put empty into field "Status"
    answer "Couldn't sign in:" && the result
    exit mouseUp
  end if
  put "Signed in." into field "Status"
end mouseUp
```

The browser opens at the provider. The script waits, as it does for
`answer`, until the user has signed in and the browser comes back
("RandomCard is signed in… You can close this tab"). Command-period stops the wait,
and after 5 minutes `the result` says the sign-in took too long.

If the browser shows an error from the provider and never comes back,
the client isn't set up to allow the `http://127.0.0.1` redirect; the
user stops the wait with Command-period, and the administrator fixes the
client.

**Signing in only when needed.** Rather than a separate step, a stack can
sign in the first time a request needs it:

```hypertalk
-- In the stack script
function signedIn conn
  if setting "Signed in" of connection conn then return empty
  sign in connection conn
  return the result
end signedIn

-- Before a request
get signedIn("Movies")
if it is not empty then
  answer "Couldn't sign in:" && it
  exit mouseUp
end if
put url "/api/me" with connection "Movies" into me
```

### 14. Company sign-in and single sign-on

**Single sign-on (SSO)** means signing in once and being signed in to
every app that uses the same provider. It's the provider's job, not
your stack's, and section 13 already gets it: the browser remembers that
the user is signed in at the provider. A user who signed in to another
app at the same provider this morning clicks Sign In in your stack, the
browser opens, and it comes straight back without asking.

**"Sign in with your company account"** works the same way. The company's
system (Microsoft Entra ID, Okta, Google Workspace, an older SAML system)
sits behind the provider, and the provider's sign-in page shows a button
for it. That's set up entirely at the provider; your stack and its
connection don't change, and they never see SAML or anything else
behind the provider.

What's not built yet (see ROADMAP.md, item 8): `sign out connection`
forgets RandomCard's tokens but leaves the browser signed in at the
provider, so the next sign-in doesn't ask; options to send the provider
hints such as which company sign-in to use; and reading the user's name
from the sign-in.

### 15. A service that signs in its own way

The types cover the common ways. For anything else, sign requests
yourself:

- **Your own header.** Set the connection's Type to None and build the
  header in the script. Anything you put in `the httpHeaders` or a
  connection's Headers is sent as it is, `Authorization` included:

  ```hypertalk
  put setting "User name" of connection "Legacy" into user
  set the httpHeaders to "Authorization: Basic" && base64Encode(user & ":" & field "Password")
  ```

- **Reading secrets in a script.** A secret setting can be read only when
  the connection's **Scripts read secrets** setting is Yes (it's No unless
  you change it). Turn it on only for a stack that has to compute a
  signature from its secret.
- **Tokens you got some other way.** On an OAuth connection, a script can
  set `setting "Access token"` and `setting "Refresh token"` to tokens it
  got itself, and RandomCard uses and refreshes them from then on.

---

## Part 4: the rest

### 16. Several servers: development, test and the real one

A developer usually has several copies of a service: one on their own
computer, a test server, the real one. Make each a connection of its own,
and keep the name of the one in use in a field:

```hypertalk
function activeConnection
  return field "Active connection" of card "Settings"
end activeConnection

put url "/movies" with connection activeConnection() into reply
```

A script can make and remove connections too, for a settings card that
lists servers with New, Edit and Delete buttons:

```hypertalk
put the connections into field "Servers"     -- one a line
create connection "Test server"
set setting "URL" of connection "Test server" to "https://test.example.com"
delete connection "Test server"              -- forgets its secrets too
```

### 17. A server on your own network, over http

Plain `http://` is unencrypted: anyone on the network between you and
the server can read what's sent. So a connection that signs in (Key,
Basic, OAuth) only does it over http to this computer (`localhost`,
`127.0.0.1`) and your local network (`.local` names, addresses like
`192.168.…` and `10.…`). Elsewhere it needs https, and the request isn't
sent; `the result` says why.

For a server that only speaks http and isn't on your network, set the
connection's **Sign in over http** to **Anywhere**. Only do that when you
know the network in between can be trusted.

A URL with no scheme gets https, or http for this computer.

### 18. When things go wrong

Every request sets `the result`. Show it, in a dialog rather than a small
field, since it's often long:

```hypertalk
on showProblem msg
  answer msg
end showProblem
```

| `the result` says | It means | What the stack does |
|---|---|---|
| empty | It worked | Nothing |
| `Connection "X" isn't set up: it needs Password` | A setting is missing | Go to the settings card |
| `Connection "X" isn't signed in` | No token, or the refresh token ran out | Sign in again (section 12 or 13) |
| `401 Unauthorized` | The server refused the credentials | Ask for the password or key again |
| `403 Forbidden` | Signed in, but not allowed | Tell the user; their account needs the right |
| `404 Not Found` | Wrong address | Check the URL setting and the path |
| `… won't sign in over plain http …` | An http server outside your network | Use https (section 17) |
| a 3xx such as `302 Found` | A signed-in request was redirected | Requests that carry credentials don't follow redirects; check the URL |
| no network, timeout | The server didn't answer | Try again; raise `the httpTimeout` for a slow one |

An OAuth request refused with 401 is retried once with a fresh token
before your script hears about it, so a 401 there really means the user
isn't allowed in any more.

### 19. What not to do

- **Don't put secrets in the stack.** Not in a field, a script, or a
  custom property: a stack is a file people pass around. Use the
  connection's secret settings, which live in the secure store.
- **Don't put a password field's text anywhere it's saved.** Make it
  secure text, and empty it once it's been used.
- **Don't build the `Authorization` header yourself** when a type does
  it; the type keeps the secret out of your scripts and away from other
  servers.
- **Don't turn on Scripts read secrets or Sign in over http: Anywhere**
  unless the stack needs it.
- **Don't show users raw errors in a tiny field.** Use a dialog.
- **Don't ask users for a password in a chain of `ask` dialogs.** Give
  them one settings card with all the fields.

---

**Example stacks** in `examples/`: ICNDB (a small REST client, sections
1–4), PostCard (a small Postman, for trying any request), and Swatches
(colors from a web service, with sliders).
