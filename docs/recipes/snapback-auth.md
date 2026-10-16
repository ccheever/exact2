# Recipe: guests, personas and password sessions

Use the [complete mixed identity app](../../snapback4/README.md#a-complete-mixed-identity-app)
for the schema, Contract, TypeScript and browser tests. The README retest command
runs it twice against the selected Snapback version.

Bare `use identity` permits guests and password accounts. Call `guest()` before
reading; keep its session with `auth: {store, key, now}` and grant `secret.keep`
for that key. `authConfig()` is public `GET /auth/config`; it reports `guests`,
`password` and `development`. `use identity password` removes guests.

During development the schema may also declare `use personas alice, bob`.
Start with `state persona = ""`, signed out, and offer an explicit persona choice.
A persona uses development headers. Password and guest sessions use stored bearer
credentials; await logout before switching accounts. Hide retained rows until the
current principal's resource answers. `serve` refuses persona headers, so remove
those controls for production. Use a distinct secret key for each backend origin.

`reload` resets a transient persona choice. An inherited test that expected
Alice after reload must now expect signed out, then choose Alice explicitly.
A password or guest session restores from the secret store instead. These
steps are part of the complete example's tested transition:

```contract-test
test "persona reload signs out; a password session restores"
  expect text "session" == "Signed out"
  tap "alice"
  clock data
  expect text "session" == "Signed in"
  reload
  clock data
  expect text "session" == "Signed out"
  expect tree missing "note-0"
  type "email" "alice@example.com"
  type "password" "correct horse battery staple"
  tap "signup"
  clock data
  expect text "session" == "Signed in"
  reload
  clock data
  expect text "session" == "Signed in"
  tap "logout"
  clock data
  reload
  clock data
  expect text "session" == "Signed out"
  expect tree missing "note-0"
```

Run that test against `dev` with `fresh` and `epoch now` at the file's top.
The [password example](../../snapback4/README.md#a-complete-online-password-app)
adds login and periodic session refresh with Contract time.
