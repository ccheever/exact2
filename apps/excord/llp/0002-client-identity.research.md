# Excord LLP 0002: what Discord's official clients send, 2026-10-22

**Type:** Research
**Status:** Draft, 2026-10-22
**Systems:** `apps/excord/discord/client.ts` (the per-host identities), `apps/excord/discord/gateway.ts` (encoding and compression)
**Author:** Claude (Opus 5.5) for Charlie Cheever
**Date:** 2026-10-22
**Related:** Excord LLP 0001 §2 (each host identifies as its official client); `client.ts`'s `IDENTITIES` table, whose values came from a model's memory and are about two years old

## Summary

The identity values in `client.ts` are stale by roughly a hundred app versions
and lack fields the current clients send. This records the current values, read
from public sources and from the desktop app installed on Charlie's M1, without
signing in or touching his account. iOS's request headers still need a capture.

## Findings

**Versions, 2026-10-22**

| Client | Version | Source |
|---|---|---|
| iOS app | 350.0, released 2026-10-19, minimum iOS 16.0 | App Store lookup, `itunes.apple.com/lookup?bundleId=com.hammerandchisel.discord` |
| macOS desktop | 0.0.419, stable, released 2026-10-22; Electron 42.11.15, Chrome 148.0.7778.280; `sentryDist` `stable-osx-universal` | the app on `apartment-m1-max` after its update (`Info.plist`, `build_info.json`, the Electron framework's strings); `discord.com/api/updates/stable?platform=osx` |
| Windows desktop | host 1.0.9265 | `updates.discord.com/distributions/app/manifests/latest?channel=stable&platform=win&arch=x64` (the older `api/updates/stable?platform=win` is retired: it answers 0.0.311 of 2021) |
| Web client | build number 647615 | `GLOBAL_ENV` in `discord.com/app` |

`client.ts` has iOS 253.0, macOS 0.0.339, Windows 1.0.9181 and web build 363699.

**`X-Super-Properties`** (the web bundle `web.2ae8383b003d87ae.js`, the function that
builds them). Fields: `os`, `browser`, `device`, `device_model_name` (empty on the
web), `system_locale`, `has_client_mods`, `browser_user_agent`, `browser_version`,
`os_version`, `referrer`, `referring_domain`, the `utm_*` fields when present,
`referrer_current`, `referring_domain_current` (and the other `_current` copies),
`release_channel`, `client_build_number`, `native_build_number` (the desktop app's
only), `client_event_source` (`null` outside the overlay), `client_launch_id`,
`launch_signature`, `client_heartbeat_session_id`, `client_app_state`, and
`install_source` when the desktop app reports one. The last four are new since
`client.ts`'s table: a session's `client_launch_id` and `launch_signature` are set
once at launch; `client_heartbeat_session_id` and `client_app_state` (`focused`,
`unfocused` and so on) change during it.

**The gateway's encoding and compression** (the same bundle):

| Client | Encoding | Compression |
|---|---|---|
| Desktop app | ETF (erlpack, its native `discord_erlpack` module) | `zstd-stream` (its native `discord_zstd` module); `zlib-stream` if the module is missing |
| Web, in a browser | JSON | `zlib-stream` (pako, 64 KiB chunks) |

The desktop app on the M1 ships both modules. The protocol lane's gateway speaks
JSON over `zlib-stream`, which is the web client's.

## What is not known

- **iOS's headers.** The app's `User-Agent` form, its `X-Super-Properties` fields
  and build number, and its gateway encoding and compression. These need a capture
  of the app's own traffic: a proxy on Charlie's machine and his phone, so the
  token never leaves them.
- **Whether Discord compares a client's claimed identity with its transport.**
  Exact's native hosts make requests through URLSession (Apple) and its portable
  transport elsewhere; the desktop app's are Chromium's. Only the iOS identity
  shares its transport (URLSession) with the client it names.
- **Header order and the full header set** of a desktop REST request
  (`X-Discord-Locale`, `X-Discord-Timezone`, `X-Debug-Options` appear in the API's
  allowed headers). A desktop capture on the M1 would settle it.

## Consequence for LLP 0001

Identifying as the desktop app on macOS and Windows means speaking ETF over
`zstd-stream`, which needs a pure-JavaScript zstd decoder and an ETF codec in the
data module. Identifying as the web client there keeps the gateway as built. Which
one each host claims is a ruling for Charlie, taken with the transport question
above.
