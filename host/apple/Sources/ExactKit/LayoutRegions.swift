import CoreGraphics

/// The layout regions' `env()` names (LLP 1123 D10), in the kernel's
/// `Region` order (`kernel/src/style/env.rs`), each followed by an edge.
enum LayoutRegions {
    static let prefixes = [
        "-exact-safe-area-corners-inset-", "-exact-layout-margins-inset-", "-exact-layout-margins-corners-inset-",
        "-exact-readable-content-inset-", "-exact-readable-content-corners-inset-",
    ]

    /// The regions as the agent's `env` facts name them: the host's twenty
    /// insets, or, when it reported none, the safe area's for every region,
    /// as the kernel resolves them.
    static func env(_ regions: [CGFloat]?, safe: (top: CGFloat, right: CGFloat, bottom: CGFloat, left: CGFloat), round: (CGFloat) -> Any) -> [String: Any] {
        let edges = ["top", "right", "bottom", "left"]
        let fallback = [safe.top, safe.right, safe.bottom, safe.left]
        var out: [String: Any] = [:]
        for (r, prefix) in prefixes.enumerated() {
            for (e, edge) in edges.enumerated() {
                let value = regions.map { $0.count == 20 ? $0[r * 4 + e] : fallback[e] } ?? fallback[e]
                out[prefix + edge] = round(value)
            }
        }
        return out
    }
}

extension ExactSession {
    /// The layout regions' insets (LLP 1123 D10), what `env(-exact-<region>-inset-<edge>)`
    /// reads: twenty, five regions of top, right, bottom, left, or none when every region is
    /// the safe area.
    public func regions(_ insets: [CGFloat]?) {
        guard booted, state != .destroyed else { return }
        apply(runtime.regions(insets))
    }
}
