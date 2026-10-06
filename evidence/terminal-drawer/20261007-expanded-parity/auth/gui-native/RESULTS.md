# Actual native Account sign-in drive

Current working app baked with native-owned serial auth RPC queue, fixture backend port 16332, isolated agent local port 16336. No real authentication service or credential input.

- Physical OS individual `a`, `b`, `c` keypresses: actual PTY logged exactly `abc`; AX Terminal input remained focused after each. Evidence key-a.json, key-b.json, key-c.json and ../agent-events.ndjson.
- Physical OS Escape: actual PTY logged U+001B; Settings Account terminal remained mounted and focused. Evidence physical-escape.json.
- Native clipboard CmdV: actual PTY input suffix equals the entire 10,000-character ASCII fixture exactly, in order. Evidence paste-proof.json; original pasteboard items/types saved and restored.
- Physical Tab: no PTY tab byte; AX still reported Terminal input. Traversal is not verified.
- Cancel unmounted terminal; Start created a new PTY flow and subsequent abc passed. The restarted flow was canceled before stopping the fixture app.

Agent-driver caveat: `type(..., abc)` native-module path produced only `a`; agent Escape uses platform/global shortcut delivery and dismissed Settings. These are not evidence of physical-key behavior. Orca type-text xyz duplicated the text (xyz+x+y+z), so only separate press-key and native paste are used for exact-input claims.

This drive preceded the final passive auth-event forwarding filter extension and final drawer motion attributes. Their final combined bake and regression suite are tracked separately.

Final follow-up: Tab and ShiftTab now PASS on the final build; see ../gui-tab/RESULTS.md.
