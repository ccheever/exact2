# A nested-list seek moves a sibling list and refuses the valid target

**Status:** Closed
**Severity:** P2
**Systems:** runner/src/instance/collection/into_view.rs
**Author:** Codex
**Date:** 2026-11-02
**Commit:** 5b94c216a6fee8fca42026f5cb2e9a80063da03a

`scrollIntoView("inner-b", key, row=outerKey)` chooses the first outer collection containing `outerKey`, without checking which outer collection contains `inner-b`. Two sibling collections commonly share row keys. A valid request for the second one's inner list then scrolls the first collection and is refused, leaving the intended target unchanged.

At `runner/src/instance/collection/into_view.rs:618-629`, resolution uses only `c.nested && c.resolve(row).is_ok()` and immediately begins the outer seek. The requested inner id is checked only afterward, at lines 640-653. That refusal does not undo the wrong outer seek.

Reproduced on this Mac against the commit's existing debug artifacts. Compile and boot this valid Contract:

```text
component App
  view
    column height=600
      list id="outer-a" virtualized=true height=200
        for x in [0, 1, 2] key=x
          column height=100
            list id="inner-a" virtualized=true height=80
              for y in [0, 1, 2, 3] key=y
                text y.toString()
      list id="outer-b" virtualized=true height=200
        for x in [0, 1, 2] key=x
          column height=100
            list id="inner-b" virtualized=true height=80
              for y in [0, 1, 2, 3] key=y
                text y.toString()
```

Call the public `Runner::scroll_into_view` with `list="inner-b"`, `key=Number(3)`, `row=Some(Number(2))`, `block=Start`, `inline=Nearest`, `view=None`, `smooth=false`, `offset=None`. It selects `outer-a` and reports `refused: the row holds no list with id inner-b`, although `outer-b` contains that list in row 2.

The executed probe `/tmp/exact-runner-sibling-inner-seek-probe.rs` uses the same fixture with outer keys 0 through 20, and requests row 20, which is initially unmounted. It asserts the refusal and prints the changed collections: `outer-a` gains a 640 px correction and replaces its mounted rows 0–15 with row 20; `outer-b` remains unchanged. The fixture passed `contract::compile` before running through the public runner API. No host app, network or runtime process was started; this establishes runner selection and mutation, not a rendered-host observation.

Resolve the requested inner id and outer row together before changing either collection. Add a regression with sibling nested collections sharing outer keys, including an initially unmounted target row; a refused request should leave unrelated collections unchanged.

Deduplication: searched this checkout's issues and both local `exact2/issues` and `exact/issues`, including closed tickets. The closed `20261009-scroll-padding-settlement-restore.md` concerns alignment/restoration and host animation, not selecting a sibling outer collection.

## Resolution

Nested seeks resolve the requested inner list and key before selecting or moving an outer list. Unmounted rows are inspected in a disposable tree.

## Verification

`nested_seek_selects_the_outer_list_holding_the_requested_id` passed with two outer lists sharing row keys and an unmounted target row. The unrelated list stays unchanged; a missing inner id changes neither list.
