# Empty-string `exact_in` force-unwraps a nil buffer pointer

**Status:** Closed
**Resolution:** Apple ABI writes and plan boots now handle zero-length input without dereferencing a null buffer.
**Systems:** Apple host
**Severity:** P2
**Author:** Grok 4.6 for Charlie Cheever
**Date:** 2026-09-01

`Exact.write` (`host/apple/swift/Bridge.swift`) does:

```
let bytes = Array(text.utf8)
let ptr = exact_in(bytes.count)!
bytes.withUnsafeBufferPointer { ptr.update(from: $0.baseAddress!, count: bytes.count) }
```

An empty `Array` may yield a nil `baseAddress`. `baseAddress!` traps. Clearing a text field calls `Exact.change(id, "", now)` → `write("")`. An empty guest `message` does the same. `bootPlan` of empty `Data` has the same shape on the other write path.

The GPU loaders pass `baseAddress` without `!` and the C ABI refuses a null. The bridge does not.

Fix: if `bytes.isEmpty`, call `exact_in(0)` and return 0 without copying. Same for `bootPlan` of empty data (refuse named, don't trap).
