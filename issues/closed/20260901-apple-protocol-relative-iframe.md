# Apple misclassifies protocol relative iframe URLs

**Status:** Closed
**Resolution:** Apple iframe sources now normalize protocol-relative URLs against the live plan scheme with an HTTPS fallback.
**Systems:** Apple host, WebView
**Severity:** P2
**Author:** Codex (GPT-5) for Charlie Cheever
**Date:** 2026-09-01
**Related:** LLP 1020

`host/apple/webarm/WebArm.swift:232-284` treats any WebView source without an
explicit scheme as a local filesystem path. A valid protocol-relative URL such
as `//example.com/deck` is therefore misclassified, while a browser resolves it
against the page scheme.

Resolve sources through the same URL algorithm and base URL vocabulary as the
web host before deciding local versus remote. Add relative, root-relative,
protocol-relative, `http`, `https`, and disallowed-scheme fixtures.
