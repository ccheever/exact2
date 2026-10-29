surfaces: web, ios, macos, linux
agent: Claude Code (Claude Opus 5.5)
task: receive text and files from other apps' share sheets

### Rough

- The share target is new in exact2 (LLP 1133.024 stages 1 and 2), so most of the time
  went into exact2: the manifest's `share_target`, the runner's token table and
  `sharedFiles`, the claim on each host, the Apple share extension and its inbox.
- A text file shared from another app's provider (a file URL with no suggested name)
  was taken for the sheet's own text and left out; once kept, it took the temporary
  name of the provider's copy. The extension now treats a provider with a file URL as
  a file and names it by that URL. Found by the extension's XCTest, about 20 minutes.
- A cold share's text arrives before its files, by design (D3), so a drive that checked
  as soon as the file's name appeared sometimes read the body before the file's words
  had joined it. The drive now waits for the body.
- iOS simulator drives hung twice with CoreSimulator not answering `simctl list` while
  about ten agents built on the Mac. Nothing in the app; rerunning later is the fix.

### Lean in

- The driver's `share:` form (`type fieldnotes "share:?text=…&files=…"`, and `--url`
  cold) drove every host the same way, the web included.
- Writing the extension's inbox by hand from the drive tested the real App Group claim
  on the simulator without a person sharing from Photos.

### Needed

- share target (receiving): provided, new (LLP 1133.024)
- share sheet (sending): provided
- files: provided

### Checkpoints

- editing app.contract / app.ts: smooth
- iOS build: smooth
- macOS build: smooth
- driving the app (`share.drives.mjs`, `share-inbox.drives.mjs`): rough (see Rough, the
  cold claim's order and the simulator hangs)
