# Panel: LLP 1126 §7 questions 4, 5 and 7

**Family:** GPT-6 Astra (OpenAI) · **Provider/runtime:** codex exec -m gpt-6-astra, model_reasoning_effort=max, --sandbox read-only · **Date:** 2026-10-17 · **Redacted:** no · **Method:** blind (no adviser saw another's answer), read-only worktree from b4d558107 (r2); Charlie's rulings on questions 1-3 and 6-9 were committed to it during the run (ecb28d6eb, 4ab34365a), and an adviser may have read them, the same brief to each; Charlie asked for the panel on the questions the author rated at 75% confidence or less

---

**A. The word**

**Recommendation:** Use kind-specific manifest keys: `widgets`, `live_activities`, and later `complications` or `snippets`. Use those same names in agent selectors and documentation. Contract supplies an ordinary component or root; call it a “widget entry” or “Live Activity entry.” Use the existing word “entry” when discussing them collectively.

**Confidence:** 93%.

**Reasoning:** Authors already know what they want to build. These names let them express it without learning Exact terminology. The [terminal-entry precedent](/Users/ccheever/projects/exact2-wt-surfaces/llp/1101-terminal-apps.rfc.md:102) supplies the underlying concept. A common renderer and compilation pipeline can serve several explicitly named kinds.

Renaming canvas surfaces would create work without making “surface” uniquely guessable. “Placement” fits a location within an entry, such as the Dynamic Island’s compact region. The other proposed words require teaching.

**What would change my mind:** Unprompted authoring exercises showing that people and agents consistently discover another term more easily, or a concrete author operation spanning several kinds that becomes awkward without a common declaration. Internal implementation convenience would carry little weight.

**B. Live Activity pushes**

**Recommendation:** Admit them **at stage 2**, with Expo Services build tracking as the named consumer. Initially support server updates and endings for activities started by the app. Exact exposes and maintains the ActivityKit token lifecycle; the application’s backend owns APNs sending. **General remote notifications should receive a separate ruling.**

**Confidence:** 88%.

**Reasoning:** A build activity that stops learning about the build when the dashboard is suspended misses its principal purpose. Foreground updates and countdowns are useful implementation increments, but insufficient completion criteria for that consumer.

The shared entitlement is real, but Apple uses ActivityKit-specific registration, per-activity tokens, and a `liveactivity` APNs topic. Sharing provisioning machinery does not require exposing every notification capability. Token rotation belongs in the initial implementation. [Apple’s ActivityKit push documentation](https://developer.apple.com/documentation/ActivityKit/starting-and-updating-live-activities-with-activitykit-push-notifications)

The existing delivery decision already contemplates a developer’s own push while stating that Exact sends and relays none. A clearly recorded exception to the notification refusal can preserve that service boundary. [LLP 1030.000 D6](/Users/ccheever/projects/exact2-wt-surfaces/llp/1030.000-dev-server-as-deployer.rfc.md:337) Validate the complete serialized push payload—including the generic envelope—against APNs’ 4 KB limit. [Apple’s payload limits](https://developer.apple.com/documentation/usernotifications/generating-a-remote-notification?changes=_3)

**What would change my mind:** If nobody owns a deployable sender for the chosen app, I would defer pushes until that named integration exists. I would also reconsider sequencing if device probes reveal substantially broader lifecycle work than anticipated. Shared entitlement plumbing alone would not persuade me to admit general remote notifications.

**C. The first consumer**

**Recommendation:**

- **Stage 0: Caltrain** for the hand-built SwiftUI comparison. Use Expo Services’ actual compiled TypeScript module and representative authenticated refresh in the RFC’s existing Q3 feasibility probe.
- **Stage 1: Expo Services**, with one small widget showing a selected project’s latest build and when its information was refreshed.
- **Stage 2: Expo Services again**, adding server-updated build Live Activities and a specifically useful widget interaction. Bring Interview in after it has a named feature people want.

**Confidence:** 78%.

**Reasoning:** Caltrain isolates rendering problems cheaply. This checkout actually generates departures on a [fixed date](/Users/ccheever/projects/exact2-wt-surfaces/apps/caltrain/data/src/lib.rs:117); that makes it an excellent deterministic fixture, but weak evidence for production freshness or authentication.

Expo Services introduces the consequential constraints before the widget architecture hardens: the real Hermes module, extension memory, shared credential access, expired sessions, account changes, and protected data being unavailable. Stage 1 should establish a safe placeholder before first unlock and deliberate treatment of cached account information. Apple’s `AfterFirstUnlockThisDeviceOnly` Keychain class remains inaccessible after restart until the first unlock. [Apple’s accessibility definition](https://developer.apple.com/documentation/security/ksecattraccessibleafterfirstunlockthisdeviceonly?language=objc%2Cobjc)

Have people who actually wait for EAS builds use it during stage 1. Interview’s existing users are valuable, but shipping status does not establish demand for a particular widget or Live Activity; no such job is named here.

I cannot verify a universal 30 MB extension ceiling, or whether Expo’s complete Hermes workload fits the supported devices. Those remain measurements, not assumptions. I also have not established that the Expo port has willing pilot users or an owned APNs integration.

**What would change my mind:** A concrete, frequently used Interview widget with committed users could make it the stronger stage-1 consumer. If Expo integration cannot be bounded and owned, retain Caltrain for stage 1 while resolving that dependency. A failed Hermes probe should reopen the extension design before claiming TypeScript support.

The RFC treats the first consumer chiefly as an application choice; it should name the user’s job and the evidence each stage must produce. That also connects B and C: choosing server-side build tracking makes suspended-app updates part of completion. Finally, D8’s “render touches no secret” does not settle privacy or availability. Cached account content can still be sensitive, and plain storage does not guarantee readability before first unlock. The authenticated consumer needs an explicit redaction and protected-data policy; Apple provides widget mechanisms for both. [Apple’s widget privacy guidance](https://developer.apple.com/documentation/WidgetKit/Creating-a-Widget-Extension?changes=_9)
