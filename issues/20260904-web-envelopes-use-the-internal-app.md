# Web envelopes use the internal app slug as their name

**Status:** Open
**Systems:** Web host, App manifest, Delivery
**Severity:** P3
**Author:** Codex (GPT-5) for Charlie Cheever
**Date:** 2026-09-04
**Related:** LLP 1023 D2; LLP 1030 D2; host/web/build.mjs; host/web/dev.mjs

`resolveApp` deliberately exposes two different values: `app.name` is the
internal directory/crate slug, while `app.displayName` is
`manifest.app.name` (`scripts/app.mjs:29-45`). Both web envelope producers put
the former into `exact.json` (`host/web/build.mjs:88-99` and
`host/web/dev.mjs:322-329`).

For the checked-in Caltrain manifest this publishes `app.name: "caltrain"`
although both the cross-platform app name and W3C name are `"Caltrain"`.
`manifest.json`, the page title, Apple host metadata, and the signed stream
head use the display name, so one release exposes different names depending
on whether a consumer reads the web root envelope or a native stream head.
An external app whose slug and display name differ more substantially makes
the error user-visible.

Done when static and live web envelopes use the manifest's cross-platform
display name (`app.displayName`) and a fixture asserts that the slug, W3C
manifest name, and `app.name` may differ without the envelope leaking the
slug. The signed stream and web-root envelope must agree on app id and name.
