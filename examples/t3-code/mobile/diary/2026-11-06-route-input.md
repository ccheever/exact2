# 2026-11-06 — active route input

The retained tab stacks keep inactive route boxes mounted so switching tabs does
not discard their navigation state. Their route wrappers pass input through, but
the first content row also needs an explicit active or inactive pointer policy.
Without that state, iOS could treat a retained route wrapper as the covering hit
target and Settings, Usage and pushed Back controls were unreachable.

The route content row now restores `pointer-events: auto` only for the focused
route. The inactive row remains inert while its stack is retained. Contract
coverage and the mobile navigation tests pass; a fresh signed-device drive is
still required when the device service is available.

## Checkpoints

- Contract build: passed.
- Route geometry, root navigation and scroll tests: passed.
