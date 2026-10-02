# On the web's JS target a dynamically bound readonly is inverted, so Fieldnotes' title and body can't be typed in

**Status:** Closed
**Resolution:** Fixed by 28d617918.
**Systems:** host/web-js, web host, Contract lowering
**Severity:** P1
**Author:** Claude (Opus 5.5) for Charlie Cheever
**Date:** 2026-10-02
**Related:** host/web-js/src/style.rs prop_name, host/web/src/element.rs:536, contract/lower/src/tags.rs:675

Contract lowers `readonly=<expr>` to the kernel prop `editable` holding `not <expr>`
(`contract/lower/src/tags.rs:675`, `InvertedBoolProp`). The Apple hosts read
`editable` directly, and the wasm web host turns it back into the DOM attribute with
the inversion (`host/web/src/element.rs:536`: `readonly` is `editable == false`). The JS
target finds the DOM name by projecting a lone node (`host/web-js/src/style.rs`
`prop_name`). It learns that `editable` is called `readonly`, but not that the value
flips, so a bound `readonly` is written as `editable` itself: `readonly=false` makes the
element read-only, and `readonly=true` leaves it editable. A literal `readonly=true` is
unaffected: it goes through the static attribute path in `host/web-js/src/rows.rs`.

The JS target is the web default, so this breaks Fieldnotes in the browser. Its title and
body inputs carry `readonly=(pending(opened) or not openedReady)`, which is false once
the notebook opens. The DOM then has `readonly`, and nothing typed reaches the note,
whether a person types it or the agent does (`type note-title …` leaves
`draftTitle` at `none`, and Save stays disabled).

Reproduction (an app from `exact new`, web only):

```
component Probe
  state a = ""
  state b = ""
  state locked = false
  action editA(value: string)
    a = value
  action editB(value: string)
    b = value
  view
    column padding=24 gap=12
      input value=a input=editA testId="plain"
      input value=b readonly=locked input=editB testId="readonly-state"
```

With `EXACT_APP_DIR` pointing at the app, `bun host/web/build.mjs probe-web`, then
`bun scripts/agent.mjs web --app probe "type plain one" "type readonly-state two" state`
gives `a: "one"` and `b: ""`. The emitted module contains
``G(u,`readonly`,()=>!r())``, where `r` is `locked`.

Fix: apply the wasm host's inversion where the JS target emits an `editable` binding
(for example, `prop_name` returns the value transform along with the DOM name). Add a
conformance case with a bound `readonly`; the wasm oracle already gets it right.
