# Two photo saves share one temp path

**Status:** Closed
**Systems:** Photo editor save (`apps/photo-editor/app.ts`, `savePhoto`)
**Severity:** P2
**Author:** Grok, from a review of this checkout
**Date:** 2026-11-02

**Problem.** Two overlapping saves both write `app:/tmp/edits/<same count>`. The later transform replaces the file the earlier save is about to add to Photos, so one tap can store the other edit's pixels, or the same file can be added twice.

**Cause.** `savePhoto` lists `EDITS`, then after the awaits writes `` `${EDITS}/${edits.length + 1}` `` (`apps/photo-editor/app.ts`). Nothing serializes the answer. Two calls that both observe the same directory length pass that one path to `transformImage` and then to `addToPhotoLibrary`. Fieldnotes wraps this kind of overlap because an answer can be in flight across an await; this save does not.

**Reproduction.** Save twice before the first `transformImage` finishes (a double tap, or Save on two edits while the first encode is still running). Both calls use the same output path.

**Fix.** Allocate the output name under a lock that covers the directory read through `addToPhotoLibrary`, or use a unique name so two saves cannot target one file.

**Acceptance.** Two overlapping saves write two files and add each edit's own pixels. A single save still adds one asset. A failed transform still returns the existing error and does not consume a library add.

**Resolution.** Give each photo edit a UUID destination before asynchronous encoding, removing the shared directory-count allocation.

**Verification after the fix.** The Photo Editor Bun regression held two overlapping encodes, then verified distinct paths with the corresponding pixels. A failed encode did not add a photo.
