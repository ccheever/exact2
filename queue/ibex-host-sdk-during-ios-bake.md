**ibex host SDK during iOS bake** (2026-09-07): its host `darwin_http.mm`
compile inherits the iPhone SDK; target-specific macOS CXXFLAGS unblocked this run.
Fix SDK selection in the sibling build script rather than relying on that override.

*Filed under “Next, in order (2026-08-29)”.*
