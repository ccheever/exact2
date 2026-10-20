# No App Clip target

**Status:** Closed
**Resolution:** built by 41f9dbfd8: host.ios.appClip embeds a signed clip bundle
**Systems:** host/apple build (bundle.mjs, build.mjs), app.json schema, LLP 1126 D12
**Severity:** P2
**Author:** Claude (Fable 5.1), from the Brooks shop-app lane's report
**Date:** 2026-10-19

## Symptom

LLP 1126 D12 says "An App Clip is an app", and nothing builds one: the lane spent ten minutes packaging its own.

## What exists

`assembleIOS` (`host/apple/bundle.mjs:128-203`) already builds a second `.app` for `--host` with `infoPlist` overrides (`build.mjs:419-466`) and per-bundle entitlements (`build.mjs:333-359`, with associated domains from `host.ios.associatedDomains`).

## Fix

`host.ios.appClip` in `app.json` (`id`, default `<app id>.Clip`, and the invocation domains): the iOS build embeds a clip bundle at `AppClips/<Name>.app` with `NSAppClip` in its Info.plist, `com.apple.developer.parent-application-identifiers` and `appclips:` associated domains in its entitlements, signed before the parent.

## Built

2026-10-19, Claude (Fable 5.1), branch `worktree-agent-ab30c797f56e6c1d4`.

`host.ios.appClip` in `app.json` (`scripts/app.schema.json`, `docs/recipes/app-json.md`): `{}`, or `id` (default
`<app id>.Clip`) and `name` (default the app's). With it, `bun host/apple/build.mjs --ios` (simulator and device)
embeds `<App>.app/AppClips/<Name>.app`: the app's executable, Frameworks, Assets.car and static trees a second time
(`assembleIOS`'s `sibling`, which the `--host` sample bundle now uses too), an Info.plist from `infoPlist(…, { clip:
true })` with the clip's id and name, `NSAppClip` (both requests `false`) and no URL scheme or dev link, and
entitlements from `entitlements({ …app, id, clipParent: app.id })`: the parent's, plus
`com.apple.developer.parent-application-identifiers` = [`<prefix>.<parent id>`], the parent's App Group and Keychain
group when it has entries, no `aps-environment`, and `appclips:<host>` for each `applinks:` domain in place of it;
the parent's entitlements list the `appclips:` entries beside its `applinks:`. The clip is signed before the parent.
A device build takes the clip's profile from `embeddedProfile` (`host/apple/devices.mjs`): for the clip's App ID,
of the app's profile's team, prefix and method, allowing the parent-identifier entitlement; a simulator build
needs none. A simulator build installs the nested bundle itself after the app, since `simctl install` of the app
alone does not register the clip (iOS 26.5).

Verified on an iPhone 17 Pro simulator (iOS 26.5, Xcode 26.6) with an app `exact new` made and `"appClip": {}`:
the layout, both plists, the written entitlements, `simctl listapps` listing `<id>.Clip`, and `simctl launch` of
the clip reaching the app's first screen. Not verified here: the device path (no phone and no provisioning profile
on this Mac), so `embeddedProfile`'s choice is covered by its Bun test only. The Mac's 26.5 SDK does not compile the
sound arm (QUEUE.md, 2026-10-19), which was patched locally, uncommitted, for the builds.
