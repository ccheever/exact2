# Contract: `pan=` types none of its action's parameters, so a child's pan action cannot call its parent component's action (`cannot infer the type of parameter @capture:…`)

**Status:** Closed
**Resolution:** fixed by #478
**Systems:** Contract compiler
**Severity:** P3
**Author:** Daehyeon Mun (via the T3 Code clone)
**Date:** 2026-10-11
**Related:** https://github.com/ccheever/exact2/pull/468 (T3 Code clone task pr-list-scroll-and-sidebar-sweep, PS-2); the clone's `examples/t3-code/EXACT2-GAPS.md` X13 (the row-action sweep; this gap has no X id of its own); LLP 1089 (action composition: lifted actions and their captures); LLP 1057 §10.6 (`pan`, `panrelease`); issues/closed/20260901-child-action-parameter-capture.md

## Summary

A child component's action can call its `action` props (`docs/contract-for-agents.md`, "Composition and lifetime").
When that action is bound to `pan=`, though, the call fails to build if the prop's value carries curried
arguments. A parent component's action carries them: the parent's own props are captured into it. A root action
with bound arguments (`tell=note("pre")`) carries them too. `contract build` refuses the app with an error that
names a parameter the author never wrote and cannot type:

```text
app.contract:21:5 [type-cannot-infer] cannot infer the type of parameter `@capture:2:a1:0`; write `@capture:2:a1:0: <type>` or call the action from a handler
```

The same child action bound to `press=`, `input=`, `hover=`, `scroll=` or `panrelease=` builds. A root action passed
down as it is also builds on `pan=`, because it carries nothing to capture.

The cause is general: `pan=` types none of its action's parameters. A root action `action drag(x, y)` on `pan=drag`
is refused with `cannot infer the type of parameter x; … or call the action from a handler`, though a handler calls
it. With authored parameters the author can write the types. The inliner's hidden captures have no spelling, so
there the author has no way out.

## Why this arose

In the T3 Code clone (PR #468, PS-2), the sidebar's row-action sweep had to close the window's hover card as the
sweep takes over. `ThreadSection`'s `sweep` action is bound to `pan=sweep(t.id, "settle")` on a button two
components down. It was to call `hoverCard(…)`, whose value is `T3Window`'s `sidebarHover`, a child component's
action that captures `T3Window`'s props. `contract build` failed with `[type-cannot-infer] cannot infer the type of
parameter @capture:108:a11:0`. The clone works around it by calling the root's `sidebarHover` instead. The root
hands it down unchanged (as `unhover`), so it carries no captures. On that call the window's half of `sidebarHover`
(its count of hover events) does not run.

## Reproduction

Main `7e86209e20a32d5660ed182925d6b46f68ea20b8`. The app is made with `bun scripts/exact.mjs new <dir>`, and its
`app.ts` exports an empty `sources`. The command is `bun exact.mjs contract build app.contract`.

```text
// A child's action on `pan=` calls an action prop whose value is its parent component's action.
component Capapp
  view
    main testId="root" padding=16
      Panel(title="Panel")

component Panel
  props
    title: string
  state last = ""
  action note(label: string)
    last = label
  view
    column
      text `${title}: last ${last}` testId="last"
      Leaf(label="a", tell=note)

component Leaf
  props
    label: string
    tell: action
  action drag(dx: number, dy: number)
    tell(label)
  view
    box pan=drag width=100 height=40 testId="go"
```

| Variant (one change to the file above) | Result on main `7e86209e2` |
|---|---|
| As written | exit 1: `app.contract:21:5 [type-cannot-infer] cannot infer the type of parameter @capture:2:a1:0; write @capture:2:a1:0: <type> or call the action from a handler`. Line 21 is Leaf's `tell: action` |
| The clone's shape: `Leaf` hands `drag("k")` to a `Grip` component, whose `box pan=move` binds it | exit 1, the same refusal |
| `button "Go" press=drag(1, 2)` in place of the `box` | builds |
| `box panrelease=drag` | builds |
| `scroll scroll=drag` | builds |
| `input=`, `hover=` with a matching action | build |
| The root owns `note`, and `Panel` hands it to `Leaf` unchanged (`tell=tell`), on `pan=` | builds |
| The root owns `note(pre: string, label: string)` and passes `tell=note("pre")`, on `pan=` | exit 1, the same refusal (`@capture:2:a1:0`) |
| No components: the root's `action drag(x, y)` on `pan=drag` | exit 1: `cannot infer the type of parameter x … or call the action from a handler`, and the same for `y` |
| The root's `action drag(k, x: number, y: number)` on `pan=drag("k")` | exit 1: `cannot infer the type of parameter k` |

## Where it comes from

1. The inliner lifts a child's actions into the root. A child's non-action props become hidden parameters
   `@capture:{n}:{i}`, typed with the prop's type (`contract/syntax/src/inline.rs:526-554`). A child action's value
   at the use site is therefore a call that carries those captures (`:604-613`). When a grandchild's action calls an
   action prop whose value is such a call, each curried argument becomes another hidden parameter,
   `@capture:{n}:a{pi}:{j}`, with `ty: None` (`:555-603`, LLP 1089 D7). Its type has to come from where the lifted
   action is bound.
2. The type checker gives an untyped parameter its type from the handlers that bind its action:
   `refine_params_from_view` (`contract/types/src/component.rs:447-684`) infers each bound argument and each event
   payload. Its list of handler names (`:488-541`) leaves out `pan`, and its payload table (`:577-600`) has
   `scroll`, `panrelease` and `resize` as two numbers but no `pan`. So nothing types a `pan=` action's parameters,
   and `:348-354` reports each one as `type-cannot-infer`.
3. Analysis knows `pan`. It is in `HANDLERS` (`contract/analyze/src/lib.rs:330`), its arity is two numbers
   (`:449`), and its payload is `dx`, `dy` (`contract/analyze/src/payload.rs:38`). Four more handlers are in
   `HANDLERS` and missing from the type checker's list: `heightrelease`, `reorderdrop`, `transformgeometry` and
   `transformrelease`. That was read from the source and not run.

A local check, not kept: adding `"pan"` to that list, and to the two-number payload arm, makes every refused variant
above build.

## Expected

- A handler types its action's parameters, the hidden captures included. A child's action can call an action prop
  whatever handler binds it, as it can on `press=`. Every handler that analysis admits should be in the type
  checker's list.
- A refusal never names an inliner parameter (`@capture:…`) that the author cannot write. If a capture cannot be
  typed, the message names the prop and the use (`tell=note` at line 16).

## Acceptance criteria

- The repro builds, and so do the `pan=drag("k")` and root-curried variants.
- `action drag(x, y)` on `pan=drag` gets `number` for both parameters, as on `panrelease=`.
- A test in the compiler's corpus or `contract/cli/tests` covers a child's `pan=` action that calls a parent
  component's action through a prop. One more test covers `heightrelease`, `reorderdrop`, `transformgeometry` and
  `transformrelease`.
