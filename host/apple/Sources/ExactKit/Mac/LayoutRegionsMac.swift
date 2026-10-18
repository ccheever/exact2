#if os(macOS)
import AppKit

/// The layout regions AppKit lays out beside the safe area (LLP 1123 D10),
/// what `env(-exact-<region>-inset-<edge>)` resolves to: each region of the
/// session's view, as an inset per edge from its bounds, the viewport.
/// AppKit is the source: its corner adaptation is macOS 26's
/// `NSView.LayoutRegion`, and a region below macOS 26 is the one without
/// it. AppKit has no readable-content region: it is the margins.
enum LayoutRegionsMac {
    /// Twenty insets, five regions of top, right, bottom, left, in
    /// `Region`'s order (`kernel/src/style/env.rs`), none less than zero.
    static func insets(of view: NSView) -> [CGFloat] {
        let bounds = view.bounds
        // How far `rect` lies inside the bounds at each edge, top, right,
        // bottom, left, in the view's own orientation
        func edges(_ rect: NSRect) -> [CGFloat] {
            let top = view.isFlipped ? rect.minY - bounds.minY : bounds.maxY - rect.maxY
            let bottom = view.isFlipped ? bounds.maxY - rect.maxY : rect.minY - bounds.minY
            return [top, bounds.maxX - rect.maxX, bottom, rect.minX - bounds.minX].map { max(0, $0) }
        }
        let safe: [CGFloat], margins: [CGFloat], marginsCorners: [CGFloat]
        if #available(macOS 26.0, *) {
            func adapted(_ region: (NSView.LayoutRegion.AdaptivityAxis) -> NSView.LayoutRegion) -> [CGFloat] {
                let h = edges(view.rect(for: region(.horizontal))), v = edges(view.rect(for: region(.vertical)))
                return [v[0], h[1], v[2], h[3]]
            }
            safe = adapted { .safeArea(cornerAdaptation: $0) }
            margins = edges(view.rect(for: .margins()))
            marginsCorners = adapted { .margins(cornerAdaptation: $0) }
        } else {
            // The safe area and the margins guide, without adaptation
            safe = edges(view.safeAreaRect)
            margins = edges(view.layoutMarginsGuide.frame)
            marginsCorners = margins
        }
        return safe + margins + marginsCorners + margins + marginsCorners
    }
}
#endif
