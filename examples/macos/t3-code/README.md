# T3 Code for Exact on macOS

A macOS client for an existing T3 Code server. Contract draws the interface;
TypeScript owns the client state and projections; an app-local Swift module
handles HTTP, WebSocket RPC, Keychain credentials and the composer's Return key.
Provider execution, workspaces, Git and conversation history remain on the T3
server. The app does not bundle or modify T3 Code.

## Verification status

The native app built and launched. An isolated, unchanged T3 server exercised
pairing, project creation, streaming, approval and single/free-text question
responses, interruption, checkpoint diffs, older-history loading and reconnect
after a server restart. Fifteen consecutive turns with concurrent draft edits
verified the async reply-ownership fix in Exact's JavaScript executor. Light/dark and
minimum-size layouts, sidebar resizing, native newline handling and local draft
restoration were checked. Reopening and connecting without a pairing credential
restored the Keychain session, selected thread and draft. Streaming followed the
bottom while preserving the reading position after scrolling up. These runs used T3 nightly
`0.0.46-nightly.20261003.2610` (`8ed276c`), newer than the protocol reference below.

The tested T3 Grok adapter removes `multiSelect` while constructing provider
questions, so live multiple-selection validation is blocked by that adapter.
The TypeScript tests cover the client-side multiple-selection representation.

The desktop layout follows T3’s 52-point unified header, Nightly sidebar artwork,
compact settled shelf, unboxed assistant transcript and 736-point composer. Native
traffic-light buttons stay in the AppKit window. Reasoning effort comes from the
server’s model catalog, and the settled banner can un-settle a thread.

## Build and connect

Requires macOS 14 or later, Xcode, the repository's Rust toolchain, Bun version
from `package.json`, and the native TypeScript Hermes toolchain described in the
[root setup instructions](../../../README.md#1-install-the-tools). Run from the
Exact repository root:

```sh
bun install --frozen-lockfile
export EXACT_APP_DIR="$PWD/examples/macos/t3-code"
bun host/apple/build.mjs macos-t3-code-apple --bundle --run
```

Start your existing T3 installation with `t3`, or use its normal source-checkout
startup command. Configure and authenticate at least one provider in T3 Code.
This client implements orchestration protocol 2 and requires the server's
`serverResolvedCommandContext` capability for writes. Its protocol reference is
T3 Code commit `4f7760e6`.

1. In T3 Code, open **Settings → Connections** and create a fresh pairing link.
   A command-line installation can also use `t3 pair` for a running server.
2. Open this app's connection settings. Enter the server address and paste the
   pairing URL or credential. A full pairing URL can be pasted into either field.
   The default server address is `http://127.0.0.1:3773`; use the address your T3
   server actually exposes.
3. Connect and wait for synchronization. Select a project and thread, or add an
   existing workspace by its absolute path **on the server**. Adding a project
   does not create the directory.
4. Choose an available provider/model and send a message. New threads start in
   **Ask for approval** mode with the default interaction mode and project root
   workspace. Existing threads retain their server-owned modes.

## Supported workflows

- Project and thread selection, thread search, grouped sidebar and saved drafts.
- Bounded conversation history with older-page loading and live assistant/tool
  updates.
- New threads, message submission, stop, provider/model selection and supported
  permission/interaction modes, model reasoning options and un-settling threads.
- Live approvals, single/multiple-choice questions and custom answers when the
  provider permits them. Requests that cannot resume explain why they are disabled.
- Read-only checkpoint diffs, including changed files and unified diff lines.
- Light/dark appearance, a resizable/collapsible sidebar and a narrower-window
  changes overlay.

Return sends; Shift-Return inserts a newline. Return that commits marked IME
text remains an editor action. The send button also supports Command-Return.
Command-N starts a thread, Command-B toggles the sidebar, Command-D toggles
changes, and Command-Shift-M opens the model picker.

The Swift module keeps access credentials in Keychain, scoped to the server
origin and environment. It atomically saves the versioned `t3-code.json` preference
file under Exact's app data directory on its serial queue. The file holds selections,
drafts, sidebar preferences and pending operation identities. **Disconnect** keeps the saved credential;
**Forget** removes it from this device. Read-only credentials can view history but
cannot submit changes. After reopening the app, choose **Connect** with the pairing
field empty to restore the saved session.

After a lost connection, the client resynchronizes before permitting writes.
A submission with an uncertain result retains its original command identifiers
and draft. It is never retried automatically; inspect the synchronized thread and
use the explicit retry action if needed.

This example supports one connected environment at a time. Provider installation,
authentication, terminals, source-control writes, attachments, cloud/SSH connection
setup and requests that require T3's own UI remain in T3 Code. Web, iOS and Linux
are outside this example's scope.

## Source and checks

`app.contract`, `panels.contract`, `shapes.contract`, `icons.contract` and
`nightly.contract` define the UI.
`app.ts`, `client.ts`, `protocol.ts`, `domain.ts` and `presentation.ts` implement the
data source, commands and event projection. `modules/apple/` contains the native
transport, credentials, composer hook and window chrome; `apple/` contains the Exact bake adapter
and native test sources. T3's MIT notice is retained in `LICENSE-T3`.

```sh
bun test examples/macos/t3-code/client.test.ts examples/macos/t3-code/domain.test.ts
bun node_modules/typescript/bin/tsc --noEmit --strict --target ES2020 --module ESNext --moduleResolution bundler --skipLibCheck --lib ES2020,DOM examples/macos/t3-code/app.ts
cargo run -q -p contract -- build examples/macos/t3-code/app.contract -o /tmp/t3-code.plan
```

The TypeScript tests exercise snapshot/replay reduction, stale-response rejection,
chunked native responses, abandoned async answers, inbox acknowledgment races,
subscription completion, provider choices, requests, diffs, scoped drafts and
explicit retry/reconciliation after uncertain writes.
The Apple crate is a workspace member outside the default Cargo members; a Cargo
build alone does not compile or launch its Swift module. Use the app build above
for an integrated check.

Compile and run the native transport tests from the repository root:

```sh
T3_XCODE=$(xcode-select -p)
T3_TEST_FRAMEWORKS="$T3_XCODE/Platforms/MacOSX.platform/Developer/Library/Frameworks"
T3_TEST_LIBRARIES="$T3_XCODE/Platforms/MacOSX.platform/Developer/usr/lib"
xcrun swiftc -swift-version 5 \
  -F "$T3_TEST_FRAMEWORKS" -I "$T3_TEST_LIBRARIES" -L "$T3_TEST_LIBRARIES" \
  -Xlinker -rpath -Xlinker "$T3_TEST_FRAMEWORKS" \
  -Xlinker -rpath -Xlinker "$T3_TEST_LIBRARIES" \
  examples/macos/t3-code/modules/apple/T3Protocol.swift \
  examples/macos/t3-code/modules/apple/T3Credentials.swift \
  examples/macos/t3-code/modules/apple/T3Transport.swift \
  examples/macos/t3-code/apple/tests/transport/main.swift \
  -o /tmp/exact-t3-transport-tests
/tmp/exact-t3-transport-tests
```

The live transport test skips unless `T3_TRANSPORT_PAIRING_FILE` points at a fresh
disposable pairing JSON file and `T3_TRANSPORT_ORIGIN` names its server. The local
transport tests require no running server. The live test exercises real HTTP/RPC
authentication and subscriptions; it does not require a provider turn.

The composer tests use real AppKit text views and Exact's module facade. Reuse the
Xcode test paths above, then run:

```sh
export T3_COMPOSER_TEST_DIR=$(mktemp -d /tmp/exact-t3-composer.XXXXXX)
bun -e 'import { writeDataKeys } from "./host/apple/data-keys.mjs"; import manifest from "./examples/macos/t3-code/app.json"; writeDataKeys({ manifest }, process.env.T3_COMPOSER_TEST_DIR + "/ExactDataKeys.swift");'
xcrun swiftc -swift-version 5 -module-name T3ComposerTests \
  -F "$T3_TEST_FRAMEWORKS" -I "$T3_TEST_LIBRARIES" -L "$T3_TEST_LIBRARIES" \
  -Xlinker -rpath -Xlinker "$T3_TEST_FRAMEWORKS" \
  -Xlinker -rpath -Xlinker "$T3_TEST_LIBRARIES" \
  host/apple/modules/ExactNativeModule.swift \
  "$T3_COMPOSER_TEST_DIR/ExactDataKeys.swift" \
  examples/macos/t3-code/modules/apple/T3Composer.swift \
  examples/macos/t3-code/modules/apple/T3WindowChrome.swift \
  examples/macos/t3-code/apple/tests/composer/main.swift \
  -o "$T3_COMPOSER_TEST_DIR/composer-tests"
"$T3_COMPOSER_TEST_DIR/composer-tests"
```
