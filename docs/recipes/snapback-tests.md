# Recipe: testing against a Snapback backend

A Snapback app needs its backend running before `test web`: the runner checks
loopback origins in `app.ts`’s `net.fetch` grants and refuses a Snapback run if
none listens. Start `snapback4 dev` on that origin’s port, seeded if needed;
`EXACT_PORT` sets the page’s port and never moves the backend.

Each test opens empty app storage; backend rows survive tests and runs unless
`fresh` at the file’s top (or leading a test) empties `snapback4 dev` before each
test. This uses `POST /dev/reset` (Snapback4 **0.4.29**, `dev` only).
`fresh "seed.json"` then seeds it with `[{"table": …, "row": {…}}]`, from a path
relative to the test file. A server lacking reset fails the test.
Each seed row includes a stable 26-character Crockford base32 `id` (digits
and capital letters except I, L, O and U); the scaffold supplies these ids.
Tables with `insert <- deny` (or no `insert` rule) are commonly role rosters provisioned outside the
app: bare `fresh` removes them too, so privileged operations will refuse.
Use `fresh "seed.json"` and seed those roster rows for each test. A `--snapback`
scaffold writes sample roster rows (or copies the backend's `seed.json`);
review their persona, role and field values before using them.
The runner warns when the drive epoch differs from the backend clock by more
than a day, including an explicitly chosen epoch. `epoch now` follows real time;
an intentional clock test must arrange both clocks.

Server ids are unique, not ordered. Sort by time; tests can reach rows by text
or position (`for t, i in tasks key=t.id`, ``testId=`task-${i}` ``).

Wiring the backend: [snapback](snapback.md).

A test that deliberately refuses a request asserts it after settling data:

```contract-test
test "the full slot refuses a write"
  tap "send"
  clock data
  expect refusal "posted"
  expect text "error" == "SLOT_FULL"
```

`posted` is the Contract mutation slot. Each assertion consumes one
observed refusal from that slot; an unexpected or extra refusal remains a wiring
problem. Assert the failure's rendered code or message too. A refused mutation
keeps its prior successful reply and runs no `then`, so matching the slot alone
does not show the latest failure.
