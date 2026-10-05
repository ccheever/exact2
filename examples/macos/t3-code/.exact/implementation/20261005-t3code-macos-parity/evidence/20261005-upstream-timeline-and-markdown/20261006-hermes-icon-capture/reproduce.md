# Pinned Hermes async loop capture reproduction

The running app failed `timelineReadsRefreshed` after a successful native-app icon URL response with `Cannot read property 'set' of undefined`. This reduced reproduction uses the same provisioned Hermes compiler and lean engine as the app. It isolates the async IIFE's captured `const destination` in a `for...of` iteration followed by a skipped iteration. No Exact bridge or app network is involved.

`before.js` reports the same TypeError. `after.js` uses the explicit-argument named async helper pattern now in `timeline-tool-icons.ts`; two distinct keys separated/followed by skipped rows produce `PASS 2`. Bun tests alone cannot expose this compiler/engine behavior.

Run from the repository root after provisioning its normal build dependencies:

```sh
proof=examples/macos/t3-code/.exact/implementation/20261005-t3code-macos-parity/evidence/20261005-upstream-timeline-and-markdown/20261006-hermes-icon-capture
engine=target/t3-tools/hermes-6badada76212/engine
libs=$(dirname "$(rg --files target/debug/build | rg '/libhermesvmlean_a.a$' | head -1)")
xcrun clang++ -std=c++17 "$proof/engine.cpp" -I"$engine/hermes-headers" -L"$libs" -lhermesvmlean_a -ljsi -lboost_context -framework Foundation -framework CoreFoundation -o /tmp/t3-hermes-icon-engine
for variant in before after; do
  target/t3-tools/hermes-6badada76212/hermesc -O -emit-async-break-check -emit-binary -out "/tmp/t3-hermes-icon-$variant.hbc" "$proof/$variant.js"
  /tmp/t3-hermes-icon-engine "/tmp/t3-hermes-icon-$variant.hbc"
done
```

The engine executable prints the promise result, including errors; process exit zero alone is not a pass. Inspect `before.log` and `after.log`. The reduced test is diagnostic evidence, not proof that the final app passes its full mounted acceptance.
