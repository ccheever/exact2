# Restore input coverage in the JS differential driver

**Status:** Closed
**Resolution:** Fixed by cff90b364; the JS driver supplies tag-specific element constructor checks. Corpus verification recorded in the checkpoint audit.
**Systems:** semantics differential tests, web input
**Author:** Codex (GPT-6), for Charlie Cheever
**Date:** 2026-10-06
**Severity:** P2
**Related:** semantics/README.md; LLP 1071; LLP 1092

Running `target/debug/difftest corpus --js-only` with Bun 1.4.2 currently reports 293 cases: 249 agree, five diverge, three outside the JS target, and 36 driver errors. Every driver error is `ReferenceError: HTMLTextAreaElement is not defined`, before the intended input assertion.

`semantics/difftest/js/drive.mjs:119–130` creates a VM context over the lightweight render DOM and supplies Event/CustomEvent but no HTMLInputElement/HTMLTextAreaElement constructors. The real runtime's `textField` helper (`host/web/navigation.js:975`) uses those constructors for instanceof checks. Other typed-control behavior also depends on browser element classes.

Affected cases include input-change, component props, typed controls, input attributes, primitive resources, inputs in rows, and most text-editing fixtures. This is a verification defect: the same ordinary input path worked in the review's actual Chrome Caltrain drive. It does not establish that the browser product throws this error.

Supply faithful constructor/prototype behavior in the existing test DOM, or use an appropriate real browser execution path. Preserve distinct textarea/input/select behavior and their control-type rules rather than aliasing every element to one class.

Acceptance: the full JS corpus has zero missing-DOM-constructor driver errors, and the 36 cases execute their intended assertions. Test both text fields and typed controls so an instanceof workaround cannot silently misclassify them. Keep driver failures distinct from semantic divergences.
