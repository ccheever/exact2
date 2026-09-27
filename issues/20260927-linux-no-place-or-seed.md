# The Linux host never reports locale, time zone or launch seed, so `t()` stays on the base table and the documented `Intl` idiom throws

**Status:** Open
**Systems:** Linux host (`host/linux/src/app.rs`), runner time facts (`runner/src/runner/time.rs`, `runner/src/time.rs`)
**Severity:** P2
**Author:** Claude (Opus 5.5) for Charlie Cheever
**Date:** 2026-09-27
**Related:** LLP 1027.000.000 (the seed and place facts), LLP 1060

`host/linux/src/app.rs:287-296` calls only `set_time`. Nothing under `host/linux/src` calls `set_place` or `set_seed` (`runner/src/runner/time.rs:50,149`), so `locale` and `timeZone` stay `""` and `seed` stays `0`.

**Failures:**
- LLP 1027.000.000 tells TypeScript to write `new Intl.DateTimeFormat(time.locale, { timeZone: time.timeZone })`. With `""`/`""` that throws `RangeError` for the life of a Linux process.
- It also throws once on Apple and the web for any `exactTime` answer computed before `set_place` lands.
- `t()` always reads the base table on Linux, so the headless parity and smoke host cannot cover localization.
- The seed is always 0 on Linux, so ids a source mixes from it repeat across launches. That defeats the seed's purpose.

**Fix:**
- On Linux, derive locale from `LC_ALL`/`LC_MESSAGES`/`LANG` (normalize `en_US.UTF-8` to `en-US`) and the zone from the tz database. Draw a seed from `getrandom`.
- Give the pre-report values usable defaults (`en-US`/`UTC`), or document that sources must guard the empty strings.
- Set both facts in one runner call so launch commits once, not twice (the web and Apple currently commit twice, `host/web/src/host.rs:742-759`).

Found in the 2026-09-27 review of Seth's PR #47 (`seth/grnl-port-and-motion`, merge 240b418f), reviewed at `c74615a3`. Reviewers: Opus 5.5 max, Grok 4.7 xhigh, Astra max (design). Verification: confirmed by reading; `set_place`/`set_seed` have no caller on Linux.
