# Real native onboarding install pretype

The GUI Install controls for Codex and Claude opened actual native terminals on isolated backend16342/client16343. The UI displayed “Review the command, then press Enter to run it.” No Enter or other terminal keyboard input was sent.

A fixture shell wrapper recorded incoming PTY bytes and forwarded them to a real interactive /bin/sh. Codex received exactly52 bytes: `curl -fsSL https://chatgpt.com/codex/install.sh | sh`. Claude received exactly46 bytes: `curl -fsSL https://claude.ai/install.sh | bash`. Neither input contained CR or LF. Screenshots show the pending shell command after a further clock advance; no installer output appeared. This is raw destination-PTY evidence, not a captured WebSocket RPC frame.

Each Close removed the native setup terminal, re-enabled its Install control, and terminated the corresponding wrapper and child shell. The driver app was then stopped. No provider settings or preferences outside this new disposable fixture changed; normal42138 and backend16334 were untouched.

One allowed retry was used: the recorder initially called tty.setraw with its default flush option, which discarded early queued command bytes. Changing the fixture to TCSANOW preserved them. The screenshots contain extra command echo from the wrapper/child-PTY startup; the byte log proves each command arrived only once. This is a fixture echo artifact, not an additional native write.

Evidence: codex-pending-final.png, claude-pending.png, closed.png, sanitized-proof.json, shell-input.ndjson and source-fingerprints.json. Do not publish pairing.private or raw driver result files.
