# Review round 1, LLP 1121 stage 9 H: an empty symbol: name is no symbol, Astra (gpt-6-astra, max)

Reviewed: d55ef0369e.

- **BLOCKING — grouped rows differ.** [kernel/src/grouped.rs:212](/tmp/rv-rl-empty/kernel/src/grouped.rs:212) returns `None` for `symbol:` but `Some("")` for `symbol:sf/`. Consequently, a leading empty image followed by text makes only the former a custom row ([line 414](/tmp/rv-rl-empty/kernel/src/grouped.rs:414)). iOS then loses standard cell content/accessory presentation. Admit the empty role here too and test both spellings.

- **SHOULD-FIX — inspectors still report an invalid role.** `symbol:` reports `reason: "role"` while `symbol:sf/` reports `"empty"` in [web/glue.js:1054](/tmp/rv-rl-empty/host/web/glue.js:1054), [web-js/agent.js:178](/tmp/rv-rl-empty/host/web-js/agent.js:178), [AgentIOS.swift:495](/tmp/rv-rl-empty/host/apple/Sources/ExactKit/IOS/AgentIOS.swift:495), [AgentMac.swift:370](/tmp/rv-rl-empty/host/apple/Sources/ExactKit/Mac/AgentMac.swift:370), and [linux/presenter.rs:1082](/tmp/rv-rl-empty/host/linux/src/presenter.rs:1082).

- **SHOULD-FIX — existing terminal exception to “every host draws nothing.”** [terminal/src/paint.rs:736](/tmp/rv-rl-empty/host/terminal/src/paint.rs:736) paints `[image]` or alternate text when decoding fails. Both empty spellings reach this fallback and can display it with an authored nonzero box. This is shared existing behavior, not a new difference.

What checks out:

- `check`, template folding, ternary validation, aliases/literals, miswiring warnings, icons, tags, runner `icon_source`, and kernel name/path resolution handle the empty name consistently. Accepting a literal `image "symbol:"` silently can conceal an omission, but that is the ruling’s intentional consequence; nonempty misspellings remain rejected.
- Apple and both web renderers suppress the refusal log correctly. Linux/Android/Windows painters find no path and paint no glyph. Graphical hosts retain an em-sized box for **both** spellings. Native-button face validation still rejects both empty literals equally ([controls.rs:1000](/tmp/rv-rl-empty/contract/lower/src/controls.rs:1000)).
- [vocab.rs:1096](/tmp/rv-rl-empty/contract/cli/tests/it/vocab.rs:1096) correctly exercises compilation, baking, booting, and exactly one mounted image, `symbol:close`. The corpus change is correct. However, [line 1125](/tmp/rv-rl-empty/contract/cli/tests/it/vocab.rs:1125)’s host claim exceeds its assertions: mounted empty images, logs, transitions from populated symbols, and grouped-row parity remain untested.
- [QUEUE.md:17](/tmp/rv-rl-empty/QUEUE.md:17) accurately separates this narrow exception from deferred dead-arm validation policy.

Read-only JS probes confirmed blank paths, identical placeholders, and no logs for both empty spellings; they reproduced the inspector mismatch. Rust/native tests were not run: no existing build artifacts, and rebuilding would write files.

After round 1: the grouped row, the agent reasons and the test comment are fixed in round 2's sha, 963e889b00, with tests. The terminal fallback was left alone: it is the same for both spellings.
