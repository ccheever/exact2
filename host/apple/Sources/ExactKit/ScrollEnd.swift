// DOM's `scrollend` on a scroll container or a list (both Apple platforms):
// owed once the offset moves, and sent when that scroll is over — the
// person's gesture with its momentum, or an animation — after the last
// `scroll`. A move nothing brackets (a jump the app wrote, a wheel that
// reports no gesture) is over once the offset has been still for `quiet`,
// as a browser ends a programmatic scroll's.
import Foundation

/// One scroller's owed `scrollend`.
final class ScrollEnd {
    /// How long an unbracketed move waits for the next one.
    static let quiet: TimeInterval = 0.1
    private(set) var owed = false
    /// Where the offset was last seen (and its top inset on iOS): a report
    /// that moves nothing (a resize, an inset that leaves the offset) owes
    /// nothing, as a `scroll` would not fire.
    private var seen: [CGFloat] = [0, 0, 0]
    /// On the Mac: a live scroll (a trackpad's, its momentum included) is running.
    var live = false
    var elementAnimation: UUID?
    private var settle: DispatchWorkItem?

    /// The scroller reports `at`; whether that moved it, an end now owed.
    func moved(to at: [CGFloat]) -> Bool {
        guard at != seen else { return false }
        seen = at
        owed = true
        return true
    }

    /// Run `check` once the offset has been still for `quiet`, unless it
    /// moves again first.
    func settle(_ check: @escaping () -> Void) {
        settle?.cancel()
        let work = DispatchWorkItem(block: check)
        settle = work
        DispatchQueue.main.asyncAfter(deadline: .now() + Self.quiet, execute: work)
    }
    /// A gesture or animation whose end is reported holds the scroll.
    func hold() { settle?.cancel(); settle = nil }

    /// Whether a `scrollend` is owed now; it is no longer.
    func take() -> Bool {
        settle?.cancel(); settle = nil
        defer { owed = false }
        return owed
    }
    /// Nothing owed or waiting: the view goes to another node or scroller.
    func reset() { settle?.cancel(); settle = nil; owed = false; live = false; elementAnimation = nil; seen = [0, 0, 0] }
}
