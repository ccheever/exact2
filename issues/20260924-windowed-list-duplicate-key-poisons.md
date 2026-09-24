# A duplicate key in a live windowed list poisons the session

**Status:** Open
**Systems:** Runner, Lists
**Severity:** P1
**Author:** Grok 4.7 for Charlie Cheever
**Date:** 2026-09-24
**Related:** LLP 1010

A windowed list (`item-height` or `estimated-item-height`) that boots with unique keys dies on the next commit whose items share a key. `ListWindow::replace` returns `DuplicateKey` while scanning keys, before it writes `items` or `heights` (`runner/src/instance/window.rs`). `update_tree` then calls `poison` for every instance error (`runner/src/runner/lists.rs`). `poison` drops in-flight requests and every later commit is `Poisoned`. The poison arm of `conclude` restores the store only.

Plain `each` and virtualized collections disambiguate repeats and journal them (`runner/src/instance.rs`, `disambiguate`). `-0` and `0` also collide here, because `key_text` folds them. Boot already reports a typed error. A later refresh, slot write, or resource answer does not: two rows with the same `item.id` end the session.

Treat a repeated key the way the other list engines do, or refuse that commit without poisoning. Done when a live windowed list keeps answering after a duplicate-key update, and the first boot of a duplicate-key list is still a typed error.
