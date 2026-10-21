//! The design server's build, stopped at the design's retention (LLP 1121
//! stage 12 B1's follow-up, theme2's 11:32 ruling).

thread_local! {
    /// Lowering stops once the design's retention and synthetic tokens are
    /// known ([`retention_only`]).
    static RETENTION_ONLY: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// Lower, on this thread, only as far as the design's retention and
/// synthetic tokens (`retention`, `synthetic_candidates`), which are
/// fixed before the design is folded: the design server's, which reads
/// nothing else of the build (stage 12 B1's follow-up). The build then
/// stops with no refusal of its own; every pass before lowering runs as
/// it always does. The setting it replaces.
pub fn retention_only(on: bool) -> bool {
    RETENTION_ONLY.with(|r| r.replace(on))
}

/// Whether lowering stops at the design's retention ([`retention_only`]).
pub(crate) fn stops_at_retention() -> bool {
    RETENTION_ONLY.with(|r| r.get())
}
