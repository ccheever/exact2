# Narrow re-check: kept iOS module roster (50ac0e4456), Astra

RIGHT

1. [Simulator selection](/tmp/rv-roster-astra/host/apple/build.mjs:1262) now ignores unknown and non-iOS names, falling back to a booted iOS simulator or `simulator(null)`, matching pre-HEAD~1 behavior. A named iOS simulator takes precedence and boots when needed. Shutdown remains conditional on `bootedHere`, including after probe failure.

2. [D8.4](/tmp/rv-roster-astra/llp/1024-native-modules.rfc.md:740) adequately resolves the OS-dependence concern: it explicitly makes OS-dependent tags and `firstFrame` unsupported. First-frame membership already comes from the build-time probe and is baked into the device’s Info.plist; caching preserves that contract.

3. **NIT — [QUEUE wording](/tmp/rv-roster-astra/QUEUE.md:9).** The package-key diagnosis and proposed normalization plus object-content hashing are accurate. “Boots a simulator again” slightly overstates the consequence: the route reruns the probe, booting only when necessary; an already-booted simulator is reused.

4. [build.mjs](/tmp/rv-roster-astra/host/apple/build.mjs) is exactly **1,499 lines**.

Validation: 18 in-memory cases using the actual selection/lifecycle code passed with simulator commands mocked. No files changed, builds run, or simulators invoked.
