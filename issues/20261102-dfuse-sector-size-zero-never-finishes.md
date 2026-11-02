# A DfuSe sector size of zero never finishes the update

**Status:** Closed
**Systems:** Bench DFU (`apps/bench/dfu.ts`, `apps/bench/app.ts`)
**Severity:** P2
**Author:** Grok, from a review of this checkout
**Date:** 2026-11-02

**Problem.** An update against a DFU alternate whose interface string names a sector size of 0 erases the same address forever. The screen stays on the update until the board errors or the app is killed.

**Cause.** `dfuseLayout` accepts `(\d+)` as the sector size (`apps/bench/app.ts`). Zero is a match. `updating` refuses a transfer size that is not greater than zero, and the erase loop advances by `dfuse.pageSize` with no such check (`apps/bench/dfu.ts`). Adding zero leaves `page` at 0, so the generator keeps yielding an erase of the same address. `continueUpdate` steps that generator once per answer.

**Reproduction.** Start an update of a non-empty image with a DfuSe interface string whose first sector size is zero, for example `@Internal Flash /0x08000000/1*0` or `/0x08000000/01*000Kg`. The erase command repeats at `0x08000000`.

**Fix.** Refuse `pageSize` unless it is a finite integer greater than zero before the erase loop, the same way `transferSize` is refused. Fail the update with `DfuError`.

**Acceptance.** The strings above fail the update before any erase is sent, and the error names the sector size. A layout of `04*016Kg` still erases 16 KiB pages and completes.

**Resolution.** Validate transfer and erase-page sizes as positive safe integers before issuing USB requests.

**Verification after the fix.** The DFU Bun tests passed, including zero, negative, fractional, NaN and infinite sizes with no USB calls, and a valid transfer.
