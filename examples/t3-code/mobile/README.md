# T3 Code for iOS

The mobile app uses the shared T3 client with a separate Contract entry and app-local UIKit modules. It requires a running T3 server. The implementation is still in development; route and visual parity are incomplete.

Use the Bun version pinned by the repository. Install the root dependencies, then the pinned browser/device viewer dependency:

```sh
bun install --frozen-lockfile
cd examples/t3-code/mobile/browser-mobile-renderer
bun install --frozen-lockfile
```

From the repository root, the native build command is:

```sh
EXACT_APP_DIR="$PWD/examples/t3-code/mobile" bun host/apple/build.mjs --ios t3-code-ios --run
```

The app build bundles the pinned viewer sources with Rolldown before the Exact bake. It does not install dependencies or use the network. Generated assets under `assets/browser-mobile/` are ignored by Git.

Run the mobile data tests from the repository root:

```sh
bun test examples/t3-code/mobile/*.test.ts
```

LLP 1107 and its children record the design, source pins and verification limits. They remain Draft.
