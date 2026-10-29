surfaces: web, macos
agent: Claude Code (Claude Opus 5.5)
task: the mention light (LLP 1133.037 §12.1), a blink(1) lit red while mentions wait

### Rough

- The light follows a derived value (`mentionTotal > 0`), and Contract sends nothing when
  a derive changes, so the light is a resource, `setMentionLight(lightId, on)`, asked
  again whenever its arguments change; its answer is what was sent.
- `attachedDevices` fails on the web and a failed resource at launch stops the web app,
  so `light.ts` chooses between the attached list and the remembered one.

- `app.contract` is at the 1,500-line cap, so the light's state, resources and actions are a
  members file (`light.contract`, `members Excord`) and its menu items a component there.

### Lean in

- The account menu's checkbox items took the light's switch with no new layout.

### Checkpoints

- web: `bun apps/excord/light.drives.mjs web` passes drives 1 and 3 (the chooser, a
  reload finding the light remembered, feature report 1 `63 ff 00 00 00 1e 00` with five
  mentions waiting).
- macOS: `light.drives.mjs macos` passes drives 1 and 5 (the attached light, Not Connected
  when unplugged, the report sent again when plugged in), before the members split.
