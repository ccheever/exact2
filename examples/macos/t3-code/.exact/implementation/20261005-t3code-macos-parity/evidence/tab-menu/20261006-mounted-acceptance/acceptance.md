# Mounted right-panel acceptance, 2026-10-06

Final native bundle build passed (`/tmp/t3-repair-final-native-2.log`). An isolated actual ExactMac app loaded persisted tabs and paired with the synthetic server through proxy 16844. Actual Contract components and production TS/native modules ran. `source.json` records source identity.

Passed observations:
- Five mounted tabs: file, two devices on host A, one device on host B sharing device ID with A, and diff. Device identities remain distinct.
- Native right-click menu visibly exposed Rename, Close, Close others, Close to the right, Close all in order (`device-menu.txt`). No file Copy path or excluded Mute entry appeared for the device.
- Closing inactive device B through its real mounted close button left active device A selected. Closing active device A then selected the surviving device at the same index; file and diff remained (`tab-transitions.json`).

Blocked observations:
- Agent double-click activates the device tab but does not open its editor. Targeted CG double-click and Orca keyboard menu selection did not produce an observable editor. The agent host runs with accessory activation and AXWindow focused=false; input delivery remains unestablished, and no application defect is established by these observations. Stop after the three bounded routes; no production repair inferred from this result.
- Native selected text, Enter/Escape/blur/empty rename, renamed process-relaunch persistence, integrated bulk actions, middle click, keyboard menu and clipboard remain unproved in this pass. Existing policy/native tests are separate evidence, not substitutes for these observations.
- One inspection incorrectly called `clock settle` while the native menu was still tracking and reproduced the known 20-second driver reentrancy timeout. It is excluded from application-failure evidence. No further settle was used with visible tracking menus.

Local-only fixture/helper/diagnostic files are under `target/t3-repair/tab-acceptance` and `target/t3-repair/gui`. Pairing data and the pairing screenshot are deliberately omitted from tracked evidence. Pixel matching was not performed, per user instruction. The GUI remains available for independent timeline and activity verification.

## Corrected normal-activation fixture

After the accessory input routes ended, one isolated normal-activation host launched with `EXACT_AGENT=live` and both `HOME` and `CFFIXED_USER_HOME` pointed at `target/t3-repair/live-home`. Pairing reached successful environment discovery and token exchange through the fixture proxy, but did not reach WebSocket ticket acquisition. This prevented mounting the connected tab scene in this fixture.

A one-second read-only sample of PID 65801 captured the transport queue inside `T3Credentials.save` → `SecItemAdd` (677 samples); the exact frame excerpt is in `normal-pairing-stack.txt`. This identifies the prerequisite where normal-host verification stopped. The app attempted its own synthetic fixture credential save; no user keychain unlock or changes to unrelated entries were attempted. No tab interaction success, failure, or renamed persistence is claimed from this blocked launch.
