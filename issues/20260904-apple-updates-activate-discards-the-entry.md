# Apple Updates::activate discards the entry assets dir that Linux returns

**Status:** Open
**Systems:** Apple host, Linux host, Delivery
**Severity:** P3
**Author:** Muse Code for Charlie Cheever
**Date:** 2026-09-04
**Related:** host/apple/src/update.rs:261; host/linux/src/update.rs:173-178; QUEUE.md:97-98

Same Client::activate contract, divergent host apply path: Apple does 'let Some((plan, _assets)) = client.activate()' and emits plan only (Swift must re-read select for assets), while Linux returns (plan, assets_dir). Touches the already-owed Linux entry fonts/deck-page overrides (QUEUE:97-98, images only today). Fix: return the assets dir on Apple too and drive overrides from it on both hosts.
