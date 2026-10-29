# C2 Apple (909be5f71b..a51df0293f), round 3, Astra (gpt-6-astra, max)

**RIGHT.** No findings in `909be5f71b..HEAD`.

- Malformed icon forms produce no path or SF-symbol name; prop removal clears a previously rendered glyph.
- Every Apple path renderer receives the rule. Rule-only set/clear updates redraw; nonzero wire bytes and default keys remain unchanged.
- The Apple reader exception is removed legitimately: `icon_name` uses `Icon::of_row`. In-memory guard probes caught both the old comparison and an adjacent raw read.
- Even-odd plans receive a different compatibility ID, which the update store enforces.
- Pixel assertions distinguish an inked band, a clear hole and nonzero filling; the toolbar’s blank-image loophole is closed.

`git diff --check` passed. Rust/XCTest suites were not run: this read-only checkout has no build artifacts. No files changed.
