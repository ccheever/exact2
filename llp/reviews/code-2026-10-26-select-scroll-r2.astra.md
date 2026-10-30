# Code review: Apple pass #2: an outline brings the selection's own copy into view, round 2 of 2, 2026-10-26 (astra)

- **Family:** OpenAI. GPT-6 Astra via `codex exec`, reasoning effort max, `-s read-only`, a throwaway worktree at `de2130e9a8`.
- **Method:** one brief (sha256 `509697093ddb23934a2e9d15e2d4fc230f6cf360fad267b78ccc7e0fb3a46549`), blind to the other reviewer; 2 rounds (host code), then narrow re-checks until clean. The author (Claude, reload1) is not a reviewer.
- **Transcription:** the final message, unedited but for the worktree path prefix.

**SHOULD** — [InspectMac.swift:244](host/apple/Sources/ExactKit/Mac/InspectMac.swift:244), [InspectIOS.swift:103](host/apple/Sources/ExactKit/IOS/InspectIOS.swift:103): Both Apple predicates still count CSS `visibility: hidden` nodes as shown. [CssVisibility.swift:1](host/apple/Sources/ExactKit/CssVisibility.swift:1) deliberately preserves native `isHidden` and alpha. Consequently, outlining a CSS-hidden selection still scrolls to its blank box; without a selection, a hidden first copy can win. Check the node’s computed CSS visibility and cover this case natively—the new Mac test only sets `isHidden`.

The other round-1 fixes look sound. Chrome supports `visibilityProperty` from [version 121](https://groups.google.com/a/chromium.org/g/blink-dev/c/ruNqbP3wDSY); the absent-API fallback avoids exceptions but skips visibility filtering.

Both web decision blocks passed isolated execution checks. Full UI tests were not run under the read-only constraint.
