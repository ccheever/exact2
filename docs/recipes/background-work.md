# Recipe: work that runs while the app is unseen

`@exact/background` (LLP 1133.017) has Background Sync's and Periodic
Background Sync's names. The app registers a tag; the platform later runs the
root's `sync` or `periodicsync` handler in a **wake**: a session that may have
no window, with a 30 s budget. Nothing is shown to the person while it runs.

1. **Declare** the grant lines, bare (no purpose: no platform shows one):

   ```ts
   export const grants = 'device.background-sync\ndevice.periodic-background-sync';
   ```

2. **Register** from data code or an action. A `sync` fires once, at the next
   chance the platform gives the app to run online: on the web while the page
   is open, on a Mac while it runs, on an iPhone at the next refresh or
   processing wake or launch, which can be hours.
   A `periodicsync` fires no sooner than `minInterval` (at least 15 minutes)
   after its last run, when the platform chooses, and never on the web.

   ```ts
   import { registerSync } from '@exact/background';
   if (!round.ok) await registerSync({ tag: `outbox:${persona}` });
   ```

3. **Handle** on the root. The handler sends to data code; the wake ends when
   that work settles, or expires at 30 s, the work then settling as the app's.
   A handler that registers its own tag again retries it after a backoff (5
   minutes, doubling to an hour; `lastChance` is true on the third attempt).

```text
use registerPeriodicSync, unregisterPeriodicSync, backgroundSyncStatus, periodicSyncTags, sync, periodicsync, BackgroundSyncStatus, SyncEvent, SyncRegistration from "@exact/background"

shape Flushed
  sent: bool

component App
  resource background = backgroundSyncStatus() as shape BackgroundSyncStatus
  resource catchUps = periodicSyncTags() as shape SyncRegistration[]
  mutation flushed as shape Flushed
  action outboxDue(tag: string, e: SyncEvent)
    send flushed = flushOutbox(tag)
  action caughtUp(tag: string)
    send flushed = catchUp(tag)
  action setCatchUp(on: bool)
    if on
      registerPeriodicSync(tag="catch-up", minInterval=1800000)
    else
      unregisterPeriodicSync(tag="catch-up")
  view
    main sync=outboxDue periodicsync=caughtUp
      if background.periodic
        input type="checkbox" switch checked=(catchUps.length > 0) change=setCatchUp aria-label="Check for mentions"
```

A wake may not open system UI, ask the person or play sound (`share`,
`openURL`, the pickers, …): each is refused into the journal. No timer fires
in a wake with no window or a hidden one. A quit Mac app has no wake:
`"keepRunning": true` under `host.macos.window` keeps it running.

**Driving it:** `state.backgroundSync` lists each registration (`ref`,
`kind`, `due`, `lastRun`); `tap @<ref>` runs it as a warm wake (refused `not
due` early, `offline` while `prefer online false`); `state.wake` shows its
outcome; `--wake '<entry>'` launches the app cold for one. Harmony's
`packages/background/test/harmony.mjs` is a whole example.

Transfers the system finishes: `contract vocab background-transfers`.
