# Independent review of #285

Verdict: no blocking findings (no P0/P1/P2/P3 code findings). Issue acceptance behavior is supported by the reviewed evidence. The repository-wide gate is still running; this review does not assert its final result.

Reviewed base commit: `fa965d3e2b36417a22228900678f7abc68e72270` plus the staged two-file diff. No implementation commit existed at review time.

SHA-256:
- `scripts/agent.mjs`: `f863e659ddbe36fccb33d40cea5febe1c85661cdee737613c57dfd8d45a0fa9a`
- `scripts/caps.test.mjs`: `b7b25402a04b9ad7d9bf0faa43e2b240e704d448c560528f6888d13a98bd4bf6`

The production change calculates elapsed wall time before adding the app clock and clamps requests to the last returned clock. This fixes floating-point cancellation while retaining the existing endpoint cap, sleeps, host strict monotonicity guard, and timer scheduling/API. Apple clock takeover freezes the host clock; ordinary non-settle seeks return their landed time. The change does not introduce tolerance, timer coalescing, scheduling policy, or resource semantics.

The executable regression reads and executes the actual driver method. It covers equal consecutive wall samples, six host branches, a reported clock ahead of the next wall sample, nondecreasing requests, exact from+1 endpoint, returned clock, elapsed report and sleep durations. Source extraction is coupled to the method boundary but will fail visibly if that boundary changes.

Evidence inspected:
- Actual native before log: macOS failed after 3,313 calls by requesting 3312.999999999998 after 3313.
- Actual native after logs: macOS and iOS each completed 40,000 calls, 80,000 seeks, zero backwards requests, and exact final clock 40000. The script asserts the endpoint on every call. Final app state reports frames=2400 and ticks=5714.
- Viewed before/after macOS images and after iOS image; displayed counters agree with runtime evidence. Images support rendered state; the logs and executable assertions establish behavior.
- Independently executed `/Users/daehyeonmun/.bun-1.4.2/bin/bun /tmp/exact285-evidence/repro-deterministic.mjs` from this checkout: all six carrier branches pass with requests [250,250.5,251].
- Reviewed verification/001.stderr.log: 61 driver tests pass, zero failures, including the new regression.
- Independently ran git diff --check: passed.

Limitations: native runtime evidence was produced by the implementer and independently inspected; this reviewer independently reran the deterministic reproduction. No physical iPhone test, actual web browser drive, or Linux runtime drive is claimed. Issue-required iOS coverage is the simulator. Full gate completion and committed-source identity comparison remain the integrator's final checks.
