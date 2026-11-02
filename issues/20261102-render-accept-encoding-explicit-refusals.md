# Render can select a content encoding the client explicitly refuses

**Status:** Closed
**Severity:** P2
**Systems:** Render HTTP serving, Response compression
**Author:** Codex
**Date:** 2026-11-02
**Reviewed commit:** `5b94c216a6fee8fca42026f5cb2e9a80063da03a`

The render server selects Brotli for both `Accept-Encoding: br;q=0, *;q=1`
and `Accept-Encoding: br;Q=0, gzip;q=1`. Both explicitly refuse Brotli.
A client that cannot decode Brotli can consequently fail to load a page or
asset despite allowing another available encoding.

`Accepts::parse` drops a refused entry without retaining its exclusion,
then lets `*` enable it again (`host/render/src/encode.rs:33-50`). It also
recognizes only lowercase `q=` (`encode.rs:36-40`). The resulting flags
control cached static variants (`encode.rs:343-355`) and page compression
(`host/render/src/serve.rs:1034-1036`).

[RFC 9110 §12.5.3](https://www.rfc-editor.org/rfc/rfc9110.html#section-12.5.3)
limits the wildcard to codings not explicitly listed and excludes a listed
coding with quality zero.
[Section 12.4.2](https://www.rfc-editor.org/rfc/rfc9110.html#section-12.4.2)
explicitly makes the `q` parameter name case-insensitive.

Reproduce from the repository root with the unchanged parser, without
building the render crate:

```sh
python3 - <<'PY'
from pathlib import Path
source = Path('host/render/src/encode.rs').read_text()
parser = source[source.index('/// The encodings a request accepts'):source.index('/// A content coding the server makes')]
probe = parser + '''
#[derive(Debug, PartialEq, Eq)] enum Encoding { Br, Gzip }
fn main() {
    for header in ["br;q=0, *;q=1", "br;Q=0, gzip;q=1"] {
        let actual = Accepts::parse(header).pick();
        println!("{header} => {actual:?}");
        assert_eq!(actual, Some(Encoding::Br));
    }
}
'''
Path('/tmp/exact-review-render-accepts.rs').write_text(probe)
PY
rustc --edition=2021 /tmp/exact-review-render-accepts.rs -o /tmp/exact-review-render-accepts
/tmp/exact-review-render-accepts
```

Observed output:

```text
br;q=0, *;q=1 => Some(Br)
br;Q=0, gzip;q=1 => Some(Br)
```

Resolve an explicit coding before applying the wildcard and recognize
parameter names without case sensitivity. Existing tests cover a plain
lowercase `br;q=0,gzip` exclusion but omit these cases.

The reproduction proves parser selection; it does not run a compressor,
HTTP exchange, or browser. No matching issue was found in either issue
corpus, including closed tickets.

**Resolution.** Explicit br/gzip quality values override wildcard admission regardless of order. Quality parsing accepts only finite values above zero and at most one, with case-insensitive parameter names.

**Verification after the fix.** All 227 render library tests passed, including explicit zero-quality refusals beside wildcards in both orders.
