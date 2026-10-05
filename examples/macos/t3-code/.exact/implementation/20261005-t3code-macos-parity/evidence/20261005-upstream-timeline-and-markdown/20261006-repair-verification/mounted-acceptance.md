# Mounted timeline observations before asynchronous disclosure repair

The existing final-bundle agent app PID 24661 was paired to real pinned runtime 16843 through verification proxy 16844. The coordinator's tab worker retained agent session 12636 and relayed explicit JSON operation batches because exec sessions are agent-scoped. This worker used actual AXPress for model selection, offscreen disclosure activation and Copy Message. Source was subsequently changed by the repair worker; these captures preserve observations of the preceding bundle and are not a final passing source report.

## Established

- `timeline-outputs.json`: actual server command detail fetched and displayed as `verified output\nsecond result line`, with command input retained.
- `timeline-error.json`: actual read output belongs to its read row. The verification proxy replaced only the skill detail response with an explicit Effect RPC failure; the mounted output renders `Couldn't load output: Isolated verification forced detail failure`.
- `timeline-missing-final.json`: proxy replaced only dynamic detail with `{item:null}`; mounted output renders `Output is no longer available.` The empty dynamic row is disabled in the accessibility tree.
- `timeline-known-skill.json/png`: selected Claude provider/model through the actual model picker (no turn invocation). Real cwd-scoped discovery yields the `Verify` chip. Price `$5`, code `$verify`, and linked `$verify` retain their intended representations. Three file link label cases render prose+chip, chip only, and chip only.
- `timeline-copied-source.txt`: actual AXPress Copy Message wrote the exact original source Markdown, including known skill token and file-link labels. No reconstructed clipboard substitute.
- `detail-wire.json`: real requests, upstream responses and deliberate fault substitutions/delay policy are retained. Proxy records original response before injecting a failure; it does not pretend those faults came from the server.

## Observed failure returned for repair

Command detail was delayed 3.5 seconds and read detail configured for one second. The driver delivered the two disclosure taps consecutively, before the first response. Only the command request was sent. `timeline-loading.json` still shows both disclosures collapsed; `timeline-outputs.json` has command expanded with output and read collapsed. Root confirmed global command gating plus awaiting the read in the disclosure command. The separate read tap after command completion succeeds. This fails prompt loading and concurrent row-open acceptance; the repair worker is changing the app flow, so a fresh mounted rerun is required.

## Unproved on the accessory agent host

Actual AX scroll action returned unsupported; a real CGEvent pointer move and wheel posted to the application produced no observed bounds change. `timeline-wheel.json/png` and layout files preserve unchanged geometry; they are not a passing physical-wheel test. Lower icon rows are clipped by the inner group scroll, so native website/themed/Finder/broken/failed rendering was not visually certified. Keyboard focus/Space/Return/output movement and timestamp hover/focus-scroll need the normal live host after the repair. No pixel-perfect comparisons were attempted.

After clearing proxy rules, a 61-second agent-clock advance produced `timeline-recovered.json/png` but did not establish refetch or error recovery. Static snapshot readiness may require another refresh; no recovery claim is made. Proxy remains transparent by default (`fault-rules.json={}`).
