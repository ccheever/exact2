# Agents

A Mac-first messenger for talking to models, built with Exact2. Each conversation is one
model at one thinking effort (Off, Low, Medium, High, Max); ⌘N starts a new one.

Run from the repository root, in two terminals:

```sh
bun apps/agents/service.ts                        # the OpenRouter transport
bun host/apple/build.mjs agents-apple --run       # the Mac app
bun host/apple/build.mjs --ios agents-apple --run # or the iOS simulator
```

The service reads `~/Dropbox/APIKeys/openrouter.txt`.

`OPENROUTER_API_KEY` or `OPENROUTER_KEY_FILE` override the key's location; `AGENTS_MOCK=1`
streams canned replies, for driving the app without a key. The key stays in the service
process, on loopback; it never enters the app. Conversations are kept in the app's own data
directory.
