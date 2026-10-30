#if os(macOS)
import AppKit

// LLP 1057.001 §8.2 on AppKit: a node's swipe is the discrete swipe AppKit
// offers a view, the swipe event (`swipe(with:)`), which a trackpad sends for a
// three-finger swipe where "Swipe between pages" uses three fingers, and some
// mouse drivers send for their side buttons. It goes up the responder chain from
// the view under the pointer, so the innermost node whose swipe names its
// direction takes it, before history's Back and Forward at the root view. The
// two-finger "Swipe between pages" is AppKit's page navigation, which history
// takes (`BackSwipe`). A swipe event carries no finger count, so a node that
// names a `touch-count` hears none.
extension NodeView {
    /// Whether this node takes a swipe event in `word`: it is still its
    /// node's view, `SwipeInput.allows` admits it (as it must a pan or a
    /// drag), and it names no count and names that direction.
    func takesSwipe(_ word: String) -> Bool {
        handlers.contains("swipe") && props["touchCount"] == nil && presenter?.views[id] === self && SwipeInput.allows(self)
            && (props["swipeDirection"] ?? "").split(separator: " ").contains { $0 == word }
    }
    package override func swipe(with event: NSEvent) {
        if let word = Self.swipeWord(deltaX: event.deltaX, deltaY: event.deltaY), takesSwipe(word) { presenter?.swipe(id, word); return }
        super.swipe(with: event)
    }

    /// A swipe event's direction: `deltaX` is 1 for a swipe left and -1 for
    /// one right, `deltaY` 1 up and -1 down (`NSEvent.h`). Natural scrolling
    /// inverts only scroll events (`NSEvent.h`): three fingers moving right
    /// gave -1 with it on (macOS 27).
    static func swipeWord(deltaX: CGFloat, deltaY: CGFloat) -> String? {
        deltaX > 0 ? "left" : deltaX < 0 ? "right" : deltaY > 0 ? "up" : deltaY < 0 ? "down" : nil
    }
}
#endif
