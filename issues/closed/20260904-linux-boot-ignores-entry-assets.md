# Linux next-launch boots the selected plan against the baked assets

**Status:** Closed
**Resolution:** Linux now pins the selected generation's verified plan and complete asset roster through boot; images and declared fonts resolve from immutable verified bytes, absent names are tombstones, and a corrupt boot asset durably falls back to entry zero before first pixel.
**Systems:** Linux host, Delivery
**Severity:** P1
**Author:** Grok 4.6 for Charlie Cheever
**Date:** 2026-09-04
**Related:** LLP 1026 D11, LLP 1030.000 §4 stage 4, `host/linux/src/app.rs`

`Store::select` returns `assets_dir`. `Client::activate` returns `(plan, assets)`. `Client::selected_plan` — the boot API — returns `(entry, plan bytes)` only and drops the directory.

Apple works around it: `exact_update_select` → `Updates.selection().assets` → `useAssets` at `ExactApp` init (`host/apple/Sources/ExactKit/Session.swift`). Linux `select_update` takes only `selected_plan` (`host/linux/src/app.rs`) and `Presenter::boot` uses the baked `config.assets`. `set_updates` never calls `use_overrides`; that runs only on `deliveryActivate` (`host/linux/src/presenter.rs`).

Next-launch on Linux therefore applies a new plan against the binary's assets. An asset-only update is a no-op until someone activates; a plan that names a new asset 404s against the bundle. The store staged the files. Fonts and deck pages from an entry are already owed in QUEUE; images were assumed to work because they do on Apple and on Linux activate.

Make the boot helper return the assets directory next to the plan (same shape as `activate`), and have Linux apply it before first pixel the way Apple already does. `selection_json` is the ABI that already carries both paths.
