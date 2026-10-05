# Ocho (phone)

Pick up where the desktop left off. The list mirrors the desktop's windows
and tabs (`desktop.json`, which `fleet serve` publishes); a session opens to
its conversation, with a composer that types into it. One Contract source;
iOS first, the web as the dev loop.

- `app.contract` — the view: the session list (a segmented control per
  desktop window, pull to refresh, a `UIMenu`), the conversation, pairing.
- `data/` — the Rust source. Every event goes to `dispatch`; I/O runs in
  lanes keyed by turn (`model.rs`) against Fleet's mobile API through the
  relay, `https://fleet-relay.fly.dev/m/<machine>/api/…`. Markdown and
  transcript entries are desktop Ocho's (`ocho-data`).

When the Mac stops answering, the phone asks the peers `fleet serve` runs on
every enrolled machine and keeps the Mac's last layout; sessions the answering
machine cannot see are greyed. It returns home when the Mac answers again.

Pairing: scan the QR under Pair Phone (`modules/apple/QRScanner.swift`,
`<qr-scanner>`, the app's own camera module), or paste
`fleet serve --describe --no-qr`. The icon is the existing app's 8-ball
(`assets/icon.png`).

```sh
bun host/apple/build.mjs --ios ocho-mobile-apple --run
bun scripts/agent.mjs ios --app ocho-mobile tree
```
