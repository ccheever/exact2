// A swipe row whose actions keep their paint (LLP 1115 §2: what the author
// writes wins). A row that names its swipe controls (`swipeTrailing`,
// `swipeLeading`) takes UIKit's swipe, whose actions UIKit draws: rounded on
// iOS 26 and later, whatever the author painted. When every control it names
// says `appearance="none"`, as a header button keeps its paint by it, the
// row stays the authored horizontal snap scroll the web shows, and this keeps
// what UIKit's swipe would have given it:
//
// - one row open at a time: a drag on one closes the others;
// - the full swipe: dragged on past its actions, more than half the row's
//   width, it presses the outermost action (a haptic when it would), and
//   the row closes, as `performsFirstActionWithFullSwipe` does;
// - VoiceOver: each action is a custom action of the row's content, as a
//   cell's swipe actions are, and the painted controls, off screen behind
//   it, are hidden from it.
#if os(iOS)
import UIKit

final class PaintedSwipes {
    private struct Row {
        weak var owner: NodeView?
        weak var body: NodeView?
        let leading: [Weak], trailing: [Weak]
    }
    private struct Weak { weak var node: NodeView? }
    private unowned let presenter: Presenter
    private var rows: [UInt32: Row] = [:]
    /// The row dragged past its full swipe this drag.
    private var armed: UInt32?
    init(_ presenter: Presenter) { self.presenter = presenter }

    /// Whether every control a row names keeps its paint.
    static func keepsPaint(_ controls: [NodeView]) -> Bool {
        !controls.isEmpty && controls.allSatisfy { $0.wrote("appearance") && $0.style["appearance"]?.string == "none" }
    }

    /// After a batch: `owner` swipes by its scroll, its actions painted.
    func adopt(_ owner: NodeView, body: NodeView, leading: [NodeView], trailing: [NodeView], label: (NodeView) -> String) {
        rows[owner.id] = Row(owner: owner, body: body, leading: leading.map { Weak(node: $0) }, trailing: trailing.map { Weak(node: $0) })
        owner.needScroll()
        let actions = (trailing + leading).map { control in
            UIAccessibilityCustomAction(name: label(control)) { [weak self, weak control] _ in
                guard let self, let control, self.presenter.views[control.id] === control, !control.disabled else { return false }
                self.presenter.press(control.id)
                return true
            }
        }
        for element in [body] + Self.elements(under: body) { element.accessibilityCustomActions = actions }
        for control in leading + trailing { control.accessibilityElementsHidden = true }
    }
    /// Keep only the rows the batch still has.
    func retain(_ owners: Set<UInt32>) {
        rows = rows.filter { owners.contains($0.key) && $0.value.owner != nil }
    }
    func reset() { rows.removeAll(); armed = nil }

    private static func elements(under node: NodeView) -> [NodeView] {
        node.container.subviews.compactMap { $0 as? NodeView }.flatMap { [$0] + elements(under: $0) }
    }
    /// Where `row` rests closed: its content at the scroll's start edge.
    private func rest(_ row: Row, _ sv: UIScrollView) -> CGFloat {
        guard let body = row.body else { return 0 }
        return max(0, min(body.convert(body.bounds, to: sv).minX, sv.contentSize.width - sv.bounds.width))
    }
    /// The action at the edge a full swipe reaches, when the drag is past it.
    private func full(_ row: Row, _ sv: UIScrollView) -> NodeView? {
        let travel = sv.panGestureRecognizer.translation(in: sv).x, maximum = max(0, sv.contentSize.width - sv.bounds.width)
        guard abs(travel) > sv.bounds.width / 2 else { return nil }
        if travel < 0, sv.contentOffset.x > maximum {
            return row.trailing.compactMap(\.node).max { $0.convert($0.bounds, to: sv).maxX < $1.convert($1.bounds, to: sv).maxX }
        }
        if travel > 0, sv.contentOffset.x < 0 {
            return row.leading.compactMap(\.node).min { $0.convert($0.bounds, to: sv).minX < $1.convert($1.bounds, to: sv).minX }
        }
        return nil
    }

    /// A drag begins on `owner`: any other open row closes.
    func began(_ owner: NodeView) {
        guard rows[owner.id] != nil else { return }
        armed = nil
        for (id, row) in rows where id != owner.id {
            guard let sv = row.owner?.scrollView else { continue }
            let x = rest(row, sv)
            if abs(sv.contentOffset.x - x) > 0.5 { sv.setContentOffset(CGPoint(x: x, y: sv.contentOffset.y), animated: true) }
        }
    }
    /// As it is dragged: a haptic once past the full swipe, as UIKit's.
    func scrolled(_ owner: NodeView) {
        guard let row = rows[owner.id], let sv = owner.scrollView, sv.isTracking else { return }
        let past = full(row, sv).map { !$0.disabled } ?? false
        if past, armed != owner.id { UIImpactFeedbackGenerator(style: .medium).impactOccurred() }
        armed = past ? owner.id : (armed == owner.id ? nil : armed)
    }
    /// Released: past the full swipe, the outermost action is pressed and the
    /// row closes.
    func willEnd(_ owner: NodeView, _ sv: UIScrollView, target: UnsafeMutablePointer<CGPoint>) {
        guard let row = rows[owner.id] else { return }
        defer { armed = nil }
        guard let action = full(row, sv), !action.disabled else { return }
        target.pointee.x = rest(row, sv)
        presenter.press(action.id)
    }
}
#endif
