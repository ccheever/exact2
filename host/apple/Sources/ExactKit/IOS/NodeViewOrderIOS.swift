// Two of NodeView's pure decisions, kept apart so each is tested without a
// view: the order a touch tries siblings in, and where a following scroll
// view's offset goes after a batch.
#if os(iOS) || os(tvOS)
import UIKit

extension NodeView {
    /// Where a following scroll view's offset goes after a batch: the end
    /// when it was at the end, else the reader's place, within range. While
    /// the reader pulls past an edge, or the rubber band carries the content
    /// back (`moving` and out of range), UIKit owns the offset until it
    /// settles: clamping it snapped the bounce on every batch (a spinner's, a
    /// poll's), which read as jitter. A reading anchor still moves it.
    static func followedTop(current: CGFloat, minimum: CGFloat, maximum: CGFloat, end: Bool, top: CGFloat, moving: Bool) -> CGFloat {
        if moving && (current < minimum - 0.5 || current > maximum + 0.5) { return end ? current : top }
        return end ? maximum : min(maximum, max(minimum, top))
    }
}
#endif
