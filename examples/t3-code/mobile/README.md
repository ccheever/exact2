# T3 Code for iOS

The mobile app uses the shared T3 client with a separate Contract entry and app-local UIKit modules. It requires a running T3 server. The implementation is still in development; route and visual parity are incomplete.

Use the Bun version pinned by the repository. Install the root dependencies, then the pinned browser/device viewer dependency:

```sh
bun install --frozen-lockfile
cd examples/t3-code/mobile/browser-mobile-renderer
bun install --frozen-lockfile
```

The iOS crate is a member of the app workspace at `examples/t3-code/Cargo.toml`. It is excluded from the framework root workspace, so Cargo commands for it use `--manifest-path examples/t3-code/Cargo.toml`.

From the repository root, the native build command is:

```sh
EXACT_APP_DIR="$PWD/examples/t3-code/mobile" bun host/apple/build.mjs --ios t3-code-ios --run
```

The app build bundles the pinned viewer sources with Rolldown before the Exact bake. It does not install dependencies or use the network. Generated assets under `assets/browser-mobile/` are ignored by Git.

Generate the pinned app tables before data tests, strict TypeScript checks or a direct Contract build:

```sh
bun examples/t3-code/tools/generate/generate.mjs
bun test examples/t3-code/mobile/*.test.ts
```

The native build runs the same generator before bundling the mobile viewers and baking the app. Generated tables are ignored by Git.

The app-local [LLP 1130](docs/1130-t3-code-ios.rfc.md) and its children record the design, source pins and verification limits. They remain Draft. Their placement in the framework's root LLP corpus is not approved; the mobile documents and review records stay with this app.
