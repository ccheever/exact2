# Narrow confirmation: package icon tokens (30e8c01a1c), Astra

**RIGHT**

- **RE-1:** [Ownership registration](/tmp/rv-pkgicons-astra/contract/cli/src/sources.rs:586) derives from the icon identity before filtering missing or escaping SVGs. Owners supply manifest dependencies, watch roots and asset entries independently of SVG existence. Creating a missing SVG changes the asset digest and follows the watcher’s `changed` announcement path. Added regressions cover initial watch registration and creation notification.
- **RE-2:** [Colour resolution](/tmp/rv-pkgicons-astra/contract/lower/src/icon_svg.rs:243) discards normalized `inherit` and `currentcolor` overrides, retaining the parent’s colour. Both requested forms have regression cases.
- **RE-3:** The multi-swap race remains explicitly documented in [confined.rs](/tmp/rv-pkgicons-astra/contract/lower/src/icons/confined.rs:35) and [QUEUE](/tmp/rv-pkgicons-astra/QUEUE.md:128), including how outside bytes can pass the check. It is acknowledged as a known limit, not claimed fixed.

No ordinary-use blocking hole found within these three checks. Static review only; no builds, tests or file changes.
