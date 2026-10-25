# A padded textarea shows no placeholder on iOS

**Status:** Closed
**Resolution:** Fixed: TextArea.draw drew the placeholder with draw(in:) in a content box shorter than a line (UIKit's inset plus the authored padding), which drops the line; it now draws from the box's first line, clipped by the view
**Systems:** Apple host (iOS), native fields (textarea)
**Severity:** P2
**Author:** Claude (Opus 5.5), from the Linear Port lane's friction log
**Date:** 2026-10-20

## What happens

The Comment field on the port's issue screen,

```
textarea rows=1 placeholder="Comment" font-size=17 padding-top=13
```

draws no placeholder on iOS. Its text view is 20.5 pt tall in the tree. `TextAreaIOS` draws the
placeholder with `NSString.draw(in: bounds.inset(by: textContainerInset))`; when the padding
leaves a rect shorter than one line, that draws nothing. This is the lane's likely cause, not
confirmed.

The lane laid a `text "Comment"` with `pointer-events="none"` over the field while the draft is
empty.

## Expected

As on the web: the placeholder shows in the content box at the first line, whatever the padding,
and is clipped, not dropped, when the box is short.

## Reproduction

The `textarea` above on iOS, empty and unfocused: no placeholder. Without `padding-top` it shows.

## Provenance

Found by an unattended Claude Opus 5.5 lane porting Linear's iPhone app to Exact (2026-10-20),
on the iOS 27.0 simulator (iPhone 17 Pro Max), exact2 at `9b2765ab79`; not yet reproduced on a later main.
The lane's notes: `~/lanes/linear/FRICTION.md` (entries 21 and 22) on Charlie's Mac.
