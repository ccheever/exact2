# enterkeyhint on a textarea does not label the iOS return key

**Status:** Closed
**Resolution:** UITextView.returnKeyType follows enterkeyhint through the input's mapping (NodeView.returnKey); unsaid it stays the plain key
**Systems:** Apple host (iOS), native fields (textarea)
**Severity:** P3
**Author:** Claude (Opus 5.5), from the Linear Port lane's friction log
**Date:** 2026-10-20

## What happens

The card lists `enterkeyhint` for `textarea`, but on iOS only single-line fields set
`returnKeyType` (`NativeFieldsIOS.swift`, the `props["enterKeyHint"]` switch on the
`UITextField`); the `UITextView` behind a `textarea` keeps the Return key. Linear's composer has a
one-line-high, growing title whose key reads Next, and the port could not match it.

Workaround (behaviour, not the key's label): a `beforeinput` handler cancels `insertLineBreak` and
focuses the description. Cost: about 10 minutes.

## Reproduction

`textarea enterkeyhint="next" rows=1`; focus it on iOS: the return key reads "return".

## Expected

`UITextView.returnKeyType` follows `enterkeyhint` as `UITextField`'s does (the HTML attribute
applies to `textarea` too: https://html.spec.whatwg.org/multipage/interaction.html#input-modalities:-the-enterkeyhint-attribute).

## Provenance

Found by an unattended Claude Opus 5.5 lane porting Linear's iPhone app to Exact (2026-10-19),
on the iOS 27.0 simulator (iPhone 17 Pro Max), exact2 at `9b2765ab79`; not yet re-run on a later main.
The lane's notes: `~/lanes/linear/FRICTION.md` on Charlie's Mac.
