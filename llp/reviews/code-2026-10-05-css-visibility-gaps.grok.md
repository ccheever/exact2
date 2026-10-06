# Round 1
I'll review the newest commit on HEAD without changing files, starting with the review instructions and the commit itself.The request is a read-only review of the newest commit, not a branch-wide review. I'll inspect that commit and the CSS visibility path around it.The commit is small and focused. I'll read the visibility, focus, and inline-text paths it touches, including callers, before judging the new behavior.The new commit is a real follow-on to per-element CSS visibility, and several of the paths it touches are correct. A visible painting run is still a hit, selection still starts, and a visible descendant that holds focus is not resigned. Four of the new behaviors are wrong.

[P1] A visible run nested under a hidden inline ancestor is not a link or a press — host/apple/Sources/ExactKit/InlineText.swift:162

`inlineTarget` aborts the whole walk at the first hidden run whose range contains the offset, and `inlineLink` / `inlineActivationTarget` stop at `!current.hidden`. A painting `visibility: visible` run is still returned, so `inlineRunShows` stays true, a miss still falls through to text selection, and hover on that run itself still resolves. The handler usually lives on an ancestor. A visible link whose text is wrapped in a hidden span, or a visible span inside a hidden link, never reaches that ancestor's `href` or `press`. The shaped-run fallback cannot recover it: `paragraphSpec` copies only the leaf's own `href`.

[P1] tvOS does not move the remote off a hidden pressable — host/apple/Sources/ExactKit/CssVisibility.swift:59

`updateFocusIfNeeded` only moves focus when the focused view can no longer be focused. `canBecomeFocused` in `RemoteTVOS.swift:17` is still true for a hidden node that has a `press` handler, because that clause does not consult `cssVisibilityHidden`. A hidden button keeps the remote focus, `remoteSelect` still presses it, and every later style apply calls `updateFocusIfNeeded` again while `isFocused` stays true. A visible descendant is left alone, which is right. A plain tabindex box with no `press` handler does move.

[P1] Hiding a dirty field applies its `change` inside the style batch — host/apple/Sources/ExactKit/CssVisibility.swift:42

`giveUpFocus` runs from `applyStyle`'s defer, while `Presenter.applying` is still true. `resignFirstResponder` / `makeFirstResponder(nil)` synchronously ends editing. `commitEdit` then calls `onChange`, and `Session` applies that batch before the rest of the visibility batch. `blur` is queued through `send` and is safe. `change` is not. A field or text area the user has typed in, with a `change` handler, reenters `apply` mid-loop. It does not resign some other view: the Mac and iOS checks are this node, its field, or its text area, so a visible descendant that holds focus keeps it. After a successful resign, later style applies no-op on iOS and macOS.

[P2] A hidden Mac box still blurs its backdrop — host/apple/Sources/ExactKit/Mac/NodeViewMac.swift:506

`updateMaterial` clears `backgroundMaterial` when hidden, then its defer always calls `applyBackdrop`. That filter is installed whenever `backdrop_blur` is set and there is no material prop (`Backdrop.swift:197`). iOS goes through `materialRequest`, which now returns nil for both, so the effect view is removed there only.

[P2] Hiding a node that has both a material and a glass group shows the group — host/apple/Sources/ExactKit/Backdrop.swift:130

`glassGroupConflict` treats `materialRequest != nil` as "this node has its own material, so no group" (`GlassGroup.swift:109`). The new nil means a hidden node no longer conflicts, and `syncGlassGroup` builds the group and reparents its children. Showing it again removes the group. The group is not one of the views `applyCssVisibility` hides.

[P2] `aria-labelledby` of a hidden plain-text paragraph goes empty — host/apple/Sources/ExactKit/Accessibility.swift:18

A visible paragraph with a `text` prop still speaks that prop: `visibleParagraphText` returns `paragraphText` when there are no inline runs. A button or label named by a visible text child is unchanged. A hidden plain-text paragraph now returns `""`. The child walk is the right accname result. `labelledBy` and `accessibleDescription` use the same string, so a hidden paragraph referenced by `aria-labelledby` or `aria-describedby` no longer contributes its text. The node's own live announcement was already skipped by `accessibilityVisible`.

The Mac test covers a focused plain box, a material view, and a fully hidden run. It does not cover a visible run under a hidden link, a field or text area, tvOS, backdrop blur, a glass group, or a labelled-by reference.

# Round 2
I'll review the newest commit on HEAD as a read-only pass, starting with the review instructions and the diff itself.This is a read-only review of the HEAD commit only. I'll pull that diff and the surrounding Apple host Swift before judging the round-1 fixes.The diff is small and focused. Next I’ll read the focus, inline-target, and layout paths around those edits to see whether the deferred resign and the hidden-run checks actually hold.The deferred resign is the risky part. I'll check whether hiding the field or text view already ends editing synchronously, which would re-enter the batch before that deferral runs.Two defects in `0b90bb016`. The deferred resign fixes a plain box, and the inline-target walk now keeps a visible run inside a hidden link. Both of the cases below still fail.

**1. Mac editors still end editing inside the style batch.** `host/apple/Sources/ExactKit/CssVisibility.swift:31`

`giveUpFocus` (`CssVisibility.swift:54`) resigns on the next turn, but `applyCssVisibility` hides the field and the text area first:

```31:32:host/apple/Sources/ExactKit/CssVisibility.swift
        field?.isHidden = hidden
        textArea?.isHidden = hidden
```

On AppKit, `isHidden = true` while that control is editing resigns the field editor synchronously and posts `controlTextDidEndEditing` / `textDidEndEditing` before `giveUpFocus` is even scheduled. Those callbacks call `commitEdit` (`Mac/FieldEditingMac.swift:26`, `Mac/TextAreaMac.swift:229`), which calls `onChange` directly (`Mac/PresenterMac.swift:790`), not `send`. `Session.swift:697` then `apply`s the change batch inside the style batch. The deferred `makeFirstResponder(nil)` runs later and finds the editor already gone. A plain focused box is fine — the new test covers that path, and UIKit does not resign on `isHidden`, so the iOS/tvOS deferral does what it claims.

**2. A click on a visible run inside a hidden link or press handler is dropped.** `host/apple/Sources/ExactKit/InlineText.swift:96`

`inlineTarget` correctly refuses only a hidden innermost run and still walks up to the hidden container. Delivery then refuses that container:

```94:96:host/apple/Sources/ExactKit/InlineText.swift
    func inlineEnabled(_ id: UInt32) -> Bool {
        guard let host = textHost(id), !host.inert, !host.disabled,
              let value = inlineText(id), value.props["disabled"] != "true", !value.hidden else { return false }
```

`activateInline` (`InlineText.swift:207`) requires `inlineEnabled`. On iOS, `touchesBegan` records `inlineActivationTarget`, which is the hidden parent (`IOS/PointerIOS.swift:154`), and `touchesEnded` calls `activateInline` (`PointerIOS.swift:176`) and returns. The `inlineLink` fallback never runs, so neither the hidden link nor a hidden `press` handler fires. On Mac, a hidden `press` handler dies the same way (`Mac/NodeViewMac.swift:1456`). A hidden href still navigates on Mac only because selection uses `inlineLink` and `follow`, which is all the new test checks (`VisibilityMacTests.swift:119`). `hoverInline` (`TextInteraction.swift:11`) uses the same gate, so a hover handler on that hidden container is dropped too. An agent tap by id on the hidden run itself is supposed to fail; the bubble path is not.

