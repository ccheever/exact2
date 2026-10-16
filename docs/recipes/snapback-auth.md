# Recipe: guests, personas and password sessions

Bare `use identity` enables guests and password accounts. A guest calls `guest()`
before any read; its bearer session is kept in the same secret store as a password
session. `use identity password` disables guests. Personas work only in `dev`.

When adapting a persona app, start with `state persona = ""`, signed out.
`reload` resets this development selection; update inherited reload assertions to
expect signed out. Password and guest sessions restore from `secret.keep` instead.
Hide retained rows until the current principal's resource has answered.

### A complete mixed identity app

Install the pinned Snapback4 in the backend directory, write `snapback/schema.q`
below, then run `bun <exact2>/scripts/exact.mjs new app --snapback --online`.
Replace the explorer with these files. `snapback --port` selects the backend at bake.
This small public guestbook uses a bounded query; a partial answer is refused.

`snapback/schema.q`:

```quarry
use identity
use personas alice, bob

table notes:
  author: principal
  body: text 1..200 ?
  at: time
  by byTime: at, id
  public 'guestbook notes are public'
  insert <- .author = viewer
  update <- deny
  delete <- deny
  online only

query recent():
  return notes last 50 by byTime

mutation post(body: text 1..200 ?):
  row = insert notes { author: viewer, body, at: now }
  return { id: row.id }
```

`app.contract`:

```contract
shape Clock
  epochAtZero: number
shape Account
  principal: string
shape Note
  id: string
  body: string
shape Board
  notes: Note[]
shape Ack
  ok: bool

component App
  state persona = ""
  state email = ""
  state password = ""
  state draft = ""
  resource time = exactTime() as shape Clock
  resource account = restoreAccount(time.epochAtZero) as shape Account else empty()
  resource board = notes(persona, account.principal, time.epochAtZero) as shape Board else empty()
  mutation signed as shape Ack refreshes account, board
  mutation posted as shape Ack refreshes board
  derive active = persona != "" or account.principal != ""
  action choosePersona
    persona = "alice"
  action leave
    persona = ""
    send signed = logout(time.epochAtZero + performanceNow())
  action editEmail(value: string)
    email = value
  action editPassword(value: string)
    password = value
  action editDraft(value: string)
    draft = value
  action guest
    send signed = guest(time.epochAtZero + performanceNow())
  action signup
    send signed = signup(email, password, time.epochAtZero + performanceNow())
  action post
    send posted = post(persona, draft.trim() == "" ? none : some(draft.trim()), time.epochAtZero + performanceNow())
  view
    column gap=8
      text (active ? "Signed in" : "Signed out") testId="session"
      text (match failure(signed) { case some(f) => "Couldn't sign in. Try again.", case none => "" }) testId="problem"
      if not active
        button "Use Alice (dev only)" press=choosePersona testId="alice"
        button "Continue as guest" press=guest disabled=pending(signed) testId="guest"
        input value=email input=editEmail type="email" aria-label="Email" testId="email"
        input value=password input=editPassword type="password" aria-label="Password" testId="password"
        button "Sign up" press=signup disabled=pending(signed) testId="signup"
      else
        button "Log out" press=leave disabled=pending(signed) testId="logout"
        input value=draft input=editDraft aria-label="Optional note" testId="draft"
        button "Post" press=post disabled=pending(posted) testId="post"
        if not pending(account) and not failed(account) and not pending(board) and not failed(board)
          for note, i in board.notes key=note.id
            text note.body testId=`note-${i}`
```

`app.ts`:

```ts
import type {Answer, Result, Sources} from './app.contract.d.ts';
import {Snapback, refused, pick} from './snapback4/snapback.ts';
import {api} from './backend/api.ts';
export const appId = 'com.example.app';
const origin = 'http://127.0.0.1:30100';
export const grants = `net.fetch ${origin}\nsecret.keep snapback-session`;
const note = pick<Result<'notes'>['notes'][number]>();
const sources: Sources = {
  restoreAccount: async ([now], store) => {
    const db = new Snapback({origin, auth: {store, key: 'snapback-session', now}});
    return {principal: (await db.restore(now))?.principal ?? ''};
  },
  notes: async ([persona, principal, now], store) => {
    if (!persona && !principal) return {notes: []};
    const db = persona
      ? new Snapback({origin, headers: () => ({'x-snapback-persona': persona})})
      : new Snapback({origin, auth: {store, key: 'snapback-session', now}});
    if (!persona && db.session?.principal !== principal) throw refused({code: 'E_AUTH', message: 'Sign in again.'});
    const rows = await db.readAll(api.recent, {});
    if (!rows.complete) throw refused({code: 'E_PARTIAL', message: 'The full guestbook is unavailable.'});
    return {notes: rows.map(row => note({...row, body: row.body ?? '(no note)'}, 'id', 'body'))};
  },
  guest: async ([now], store) => {
    const db = new Snapback({origin, auth: {store, key: 'snapback-session', now}});
    const config = await db.authConfig();
    if (!config.guests) throw refused({code: 'E_AUTH', message: 'Guests are unavailable.'});
    const signed = await db.guest();
    if (!signed.ok) throw refused(signed.why);
    return {ok: true};
  },
  signup: async ([email, password, now], store) => {
    const db = new Snapback({origin, auth: {store, key: 'snapback-session', now}});
    const signed = await db.signup(email, password);
    if (!signed.ok) throw refused(signed.why);
    return {ok: true};
  },
  logout: async ([now], store) => {
    await new Snapback({origin, auth: {store, key: 'snapback-session', now}}).logout();
    return {ok: true};
  },
  post: async ([persona, body, now], store) => {
    const db = persona
      ? new Snapback({origin, headers: () => ({'x-snapback-persona': persona})})
      : new Snapback({origin, auth: {store, key: 'snapback-session', now}});
    const sent = await db.send(api.post, {body});
    if (sent.state === 'failed') throw refused(sent.why);
    return {ok: true};
  },
};
export const answer: Answer = (source, args, store, storage, native) => sources[source](args, store, storage, native);
```

`app.test.contract`:

```contract
fresh
epoch now

test "guests authenticate before reading and can omit an optional note"
  expect text "session" == "Signed out"
  expect tree missing "note-0"
  tap "guest"
  clock data
  expect text "session" == "Signed in"
  tap "post"
  clock data
  expect text "note-0" == "(no note)"
  reload
  clock data
  expect text "session" == "Signed in"
  expect text "note-0" == "(no note)"
  tap "logout"
  clock data
  expect text "session" == "Signed out"
  expect tree missing "note-0"

test "persona reload signs out while a password session restores"
  expect text "session" == "Signed out"
  tap "alice"
  clock data
  type "draft" "persona hello"
  tap "post"
  clock data
  expect text "note-0" == "persona hello"
  reload
  clock data
  expect text "session" == "Signed out"
  expect tree missing "note-0"
  type "email" "alice@example.com"
  type "password" "correct horse battery staple"
  tap "signup"
  clock data
  expect text "session" == "Signed in"
  type "draft" "account hello"
  tap "post"
  clock data
  expect text "note-0" == "account hello"
  reload
  clock data
  expect text "session" == "Signed in"
  expect text "note-0" == "account hello"
  tap "logout"
  clock data
  reload
  clock data
  expect text "session" == "Signed out"
  expect tree missing "note-0"
```

Run `bun exact.mjs contract types app.contract -o app.contract.d.ts`,
`bun exact.mjs backend`, then `bun exact.mjs test web` in another terminal.
The README retest command executes this recipe along with the guestbooks.
The existing [password example](../../snapback4/README.md#a-complete-online-password-app)
adds login and periodic session refresh. Remove persona controls for production;
`serve` refuses persona headers. Use a distinct secret key for each backend origin.
