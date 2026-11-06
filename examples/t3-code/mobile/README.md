# T3 Code for iOS

See [the clone handoff](../CLONE.md) for the current project state and next steps.

The mobile app uses the shared T3 client with a separate Contract entry and app-local UIKit modules. It requires a running T3 server. The implementation is still in development; route and visual parity are incomplete.

Builds require macOS with Xcode and an iOS Simulator, Rustup, and the Bun version pinned by the repository. From the repository root, install the toolchains and root dependencies, then the pinned browser/device viewer dependency:

```sh
cd /path/to/exact2
bun scripts/exact.mjs setup
bun install --frozen-lockfile
(cd examples/t3-code/mobile/browser-mobile-renderer && bun install --frozen-lockfile)
```

`setup` may download the pinned Rust, Hermes and web tools the first time. Use the same Bun version for every command below.

The iOS crate is a member of the app workspace at `examples/t3-code/Cargo.toml`. It is excluded from the framework root workspace, so Cargo commands for it use `--manifest-path examples/t3-code/Cargo.toml`.

From the repository root, build and launch the iOS Simulator app with:

```sh
EXACT_APP_DIR="$PWD/examples/t3-code/mobile" bun host/apple/build.mjs --ios t3-code-ios --run
```

The command uses an already booted simulator, or chooses the newest iPhone Pro. Set `EXACT_SIM` (or pass `--sim <name-or-udid>`) to choose another simulator. To build for a connected iPhone instead, use `--device --run`; the phone must be paired, unlocked and have Developer Mode enabled.

The app build bundles the pinned viewer sources with Rolldown before the Exact bake. It uses the viewer dependency installed above. If that dependency is missing, the build stops and names the install command. Generated assets under `assets/browser-mobile/` are ignored by Git.

The native build runs the generator automatically. Run it explicitly before data tests, strict TypeScript checks or a direct Contract build:

```sh
bun examples/t3-code/tools/generate/generate.mjs
bun test examples/t3-code/mobile/*.test.ts
```

The native build runs the same generator before bundling the mobile viewers and baking the app. The generator installs its pinned packages and may fetch missing packages. Run it once while online to prepare an offline build. Generated tables are ignored by Git.

Run the cache transport fixture from `examples/t3-code/mobile` on macOS:

```sh
swiftc -o /tmp/t3-cache-transport-tests modules/apple/T3MobileClientCache.swift modules/apple/T3MobileCacheTransport.swift tests/ClientCacheTransportTests.swift -lsqlite3
/tmp/t3-cache-transport-tests
```

After the app launches, connect it to a T3 server. Start `t3` (or the server's normal source-checkout command), create a pairing link in **Settings → Connections**, then paste that link into the app's welcome screen or **Add environment**. The server must be reachable from the Mac or device running the app.

It uses the real SQLite store to check complete transfers, exact payload text, tickets, conditional deletion, expiry and restart. Its optional `--bridge <directory>` mode accepts one cache request per JSON line and returns one bridge reply per line.

The app-local [LLP 1130](docs/1130-t3-code-ios.rfc.md) and its children record the design, source pins and verification limits. They remain Draft. These numbers are local to the app; the root `llp/1130-a-page-under-the-status-bar.rfc.md` is a different document. Their placement in the framework's root LLP corpus is not approved; the mobile documents and review records stay with this app.
