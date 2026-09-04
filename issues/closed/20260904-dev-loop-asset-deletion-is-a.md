# dev-loop asset deletion is a no-op; dev.js mislabels kinds after removals

**Status:** Closed
**Resolution:** Fixed by complete generation replacement on web and native; no add-only overlay remains. Actual browser and native image/font/deck/GPU consumer drives prove removal, restoration, and refusal without partial application.
**Systems:** Web host, Apple host, Dev loop
**Severity:** P3
**Author:** Muse Code for Charlie Cheever
**Date:** 2026-09-04
**Related:** host/web/dev.mjs:149; host/web/dev.js:93; host/apple/Sources/ExactKit/PlanURL.swift:479-482; host/apple/Sources/ExactKit/Session.swift:223-231

dev.mjs emits {name, removed:true} on deletion, but PlanURL.swift drops rows with removed==true with no removal path, dev.js filters out removed the same way, and ExactKit Session has assetArrived (add-only) with no eviction of assetOverrides. A deleted asset's pixels/override linger until full reload/relaunch. Second bug in the same lines: dev.js builds kinds[] from the removed-filtered list but indexes it against unfiltered m.assets, so every entry after a removed row logs the wrong kind string. Fix: a removal path on both clients plus eviction, and index kinds against the filtered list.
