# Evaluator corrections

The evaluator is independent of implementation and edit agents. Corrections
apply globally; an evaluator rerun is not another model attempt.

Before completed trials, two setup corrections were made: click the unobscured
canvas center instead of the HUD-covered corner, and poll readiness because
PlayCanvas publishes its command interface before assets finish loading.
Godot's builder required locally extracted Chromium shared libraries and the
Chromium headless-shell executable; a browser launch failure is setup evidence,
not a game failure.

At approximately 05:31 UTC on September18, the asynchronous readiness polling
was found defective in this Bun1.3.14/Playwright1.63.0 environment. The isolated
`readiness-probe.mjs` set a flag false for1200ms; `waitForFunction(async () =>
flag)` returned after16.5ms while the flag was still false. Therefore the
previous readiness guard was not trustworthy. The replacement explicitly
awaits `page.evaluate` results in a bounded host-side polling loop.

The initial PlayCanvas A1 trial's reload failure was affected and is not scored
as an engine failure. Completed trials, accepted baselines, and startup metrics
are rerun using the corrected evaluator, without giving trial agents feedback
or changing their submitted source. Original reports remain available. Godot's
initial reload result was also potentially affected; its implementation author
had already added a localStorage fallback before this diagnosis, so that
change's necessity cannot be inferred from the first failed evaluator alone.

The batch-versus-single-tick case was subsequently strengthened to compare
crate position and velocity as well as the player, at the same0.001 tolerance.
The Godot author's own smoke exposed a crate that did not advance identically;
checking only the character would have missed it. This v3 change applies to
all baselines and submitted trial outputs; earlier reports are preserved.
