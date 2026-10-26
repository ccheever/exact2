# Tracking consent on a device (LLP 1133.028 §9 stages 1 and 2)

What the agent cannot drive: the person's answer to App Tracking
Transparency's alert. The package's fixture (`test/fixture`, made by
`test/fixture.mjs`) is the consumer. Each step names what to see; a step that
shows something else is a finding. A person runs them, never an agent on a
phone (LLP 1133.020). No account is needed beyond a development team.

## iPhone (iOS 17 or newer)

1. Make and build the fixture: `bun packages/tracking-consent/test/drives.mjs
   ios --keep` prints its directory; then `EXACT_APP_DIR=<it> bun
   host/apple/build.mjs --ios --device trackingfixture-apple --run`. "Status:
   notDetermined (prompt)", "Identifier: zeros".
2. "Ask About Tracking": the fixture's own screen with one "Continue" and no
   way to close it (D11). Continue: the system's alert, with the purpose
   string from `strings/en.json`.
3. "Allow": "Answered: authorized", "Status: authorized (granted)",
   "Identifier: set".
4. Ask again: no alert; "Answered: authorized".
5. Settings › Privacy & Security › Tracking, turn the app off: back in the app,
   "Status: denied (denied)", "Identifier: zeros".
6. Delete and reinstall, ask, "Ask App Not to Track": "Answered: denied".
7. Settings › Privacy & Security › Tracking › "Allow Apps to Request to Track"
   off: a fresh install reads denied and the request shows no alert.
8. On a fresh install, tap Continue and at once pull down Control Center, so
   the app is inactive when the request arrives; dismiss Control Center: the
   alert shows then, not while the app was inactive (D13's hold).

## Apple TV (tvOS 17 or newer)

9. Steps 1 to 4 with the Siri Remote, on an Apple TV signed with the team.

## Elsewhere

10. macOS, Android, the web, Linux and Windows have no alert: the status reads
    "denied (denied)", the button does not show, and the identifier refuses
    (drive 11, which the agent runs).
