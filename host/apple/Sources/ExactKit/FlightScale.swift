// LLP 1013.000 D4.4 (amended 2026-10-05, approved by Charlie): how a flying
// view that is not an image is shown. It keeps its own layout, at its slot's
// size, and the whole view (its surface and its children together) is scaled
// uniformly to the shown width, top-anchored, inside a clip that is the shown
// box with the interpolated radius, as CSS draws a view transition's snapshot
// (`inline-size: 100%; block-size: auto`) in its group. Shown at its own
// children's laid-out places in a resized box, a story card opening from its
// thumbnail showed only its top-left corner, and the thumbnail landing drew
// small in a card-sized box (Signal Clone, 2026-10-05).
import CoreGraphics

enum FlightScale {
    /// The shown width over the layout's; nil for an empty box or layout,
    /// which shows nothing to scale.
    static func of(shown: CGSize, layout: CGSize) -> CGFloat? {
        guard layout.width > 0, layout.height > 0, shown.width > 0, shown.height > 0 else { return nil }
        return shown.width / layout.width
    }
}
