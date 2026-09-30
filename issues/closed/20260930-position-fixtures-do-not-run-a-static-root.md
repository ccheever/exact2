# The Chrome position fixtures lay every kernel root out positioned, so the static-root path is untested there

**Status:** Closed
**Resolution:** Containing-block fixtures now exercise the production static-root default, frame comparison rejects non-finite coordinates, and existing compiler-positioning plus reparent/hide differential tests cover the other named paths.
**Systems:** kernel tests
**Severity:** P4
**Author:** Claude (Fable 5.1) for Charlie Cheever
**Date:** 2026-09-30
**Related:** kernel/tests/it/browser_position.rs, kernel/tests/it/browser_cases.rs, llp/reviews/code-2026-09-30-taffy-css.{astra,grok}.md (astra 15, grok 9)

`browser_position.rs::failures` gives every kernel root `position: relative` (the browser's root was, so that it is the containing block the kernel's is). Since 2e21c345a the kernel lowers a static root to `relative` itself, so the production path *is* what the fixture runs; but nothing in the fixtures lays a root out with `position: static` authored explicitly, and no fixture reparents an absolute box, hides an ancestor between two layouts (the hidden case has its own test now) or checks a transform-bearing box against the compiler rule end to end. `browser_cases.rs::css_rows` silently drops `translate`/`scale`/`rotate`/`transform`/`filter`/`backdrop-filter`/`isolation` so that the containing-block fixture can state what Chrome does with them; the seven such cases are pinned as `NOT_IN_THE_KERNEL`. The frame comparator does not reject NaN. Small test-quality items, none of which changes a result today.
