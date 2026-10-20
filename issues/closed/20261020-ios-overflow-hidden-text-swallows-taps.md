# On iOS a text with overflow-x=hidden takes a person's touch from the button around it

**Status:** Closed
**Resolution:** A paragraph's scroller with nothing to scroll hands the touch to its node (ScrollView.hitTest), so the press bubbles to the button
**Systems:** Apple host (iOS), hit testing, text
**Severity:** P2
**Author:** Claude (Opus 5.5), from the Linear Port lane's friction log
**Date:** 2026-10-20

## What happens

A list row `button press=open(id)` holds a title text with `white-space="nowrap"
overflow-x="hidden" text-overflow="ellipsis"` (the ellipsis needs the overflow). On iOS that text
becomes a `UIScrollView`, and a real touch on the title lands on it: the button's press never fires.
The agent's default tap activates the button directly, so `agent ios` drives and tests pass while a
person's tap does nothing. Only `--touch platform` showed it (`landed … "view":"ScrollView"`).

Workaround: `pointer-events="none"` on the text. Cost: about 25 minutes.

## Reproduction

```
list appearance="none"
  button press=hit testId="row"
    text "<long text>" white-space="nowrap" overflow-x="hidden" text-overflow="ellipsis"
```

`agent ios --touch platform "tap row" "state"`: `hit` did not run.

## Expected

As on the web: `overflow-x: hidden` clips but is not a scroll container a person can grab, so a tap
on the text activates the button around it. A truncating label should never take a touch.

## Provenance

Found by an unattended Claude Opus 5.5 lane porting Linear's iPhone app to Exact (2026-10-19),
on the iOS 27.0 simulator (iPhone 17 Pro Max), exact2 at `9b2765ab79`; not yet re-run on a later main.
The lane's notes: `~/lanes/linear/FRICTION.md` on Charlie's Mac.
