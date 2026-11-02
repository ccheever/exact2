# Components

A fixture for the site's tests: every component the generator renders, written as the Markdown that makes it. Not a docs page.

## Collapsible

<details><summary>Why one plan runs on every host</summary>

The plan is baked at build, so the first frame needs nothing else. Inline `code` stays code here too.

</details>

## Prerequisites

<!-- prerequisites -->
- Bun 1.4.2, as `package.json` pins it.
- Rust through rustup.

## Steps

<!-- steps -->
1. Make the app with `bun scripts/exact.mjs new hello`.
2. Start the dev loop with `bun exact.mjs web`.
3. Edit `app.contract` and save.

## Tabs

<!-- tabs -->
<!-- tab: macOS -->

```sh
bun host/apple/build.mjs --run
```

<!-- tab: iOS -->

```sh
bun host/apple/build.mjs --ios --run
```

<!-- tab: Linux -->

```sh
cargo build --profile host-dev -p caltrain-linux
```

<!-- /tabs -->

## File tree

```filetree
hello/app.contract
hello/app.ts
hello/app.test.contract
hello/assets/
```

## Callouts

> [!NOTE]
> A note: what a reader should know.

> [!TIP]
> A tip: a shorter way to do it with `bun exact.mjs web`.

> [!WARNING]
> A warning: what breaks if the reader skips it.

## Code title

```json app.json
{ "name": "Hello" }
```

## Diff

```diff app.contract
 component App
-  view
-    text "Hello"
+  state name = "Ada"
+  view
+    text `Hello, ${name}`
```

## Image

![The Exact mark](../../../assets/brand/exact-mark.svg "A golden rectangle, split into a square and a smaller golden rectangle.")

## Cards

<!-- cards -->
- [Start here](../../start-here.md): build an app with Exact, from the first command.
- [Contract for humans](../../contract-for-humans.md): the language, one idea at a time.
<!-- /cards -->
