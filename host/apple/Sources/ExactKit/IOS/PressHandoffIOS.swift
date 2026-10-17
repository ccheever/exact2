// A press's focus handoff on UIKit (Charlie, 2026-10-17). An `autofocus`
// never takes a focus an element holds, however it got it, as Chrome's and
// AppKit's do not: a later mount unrelated to a press (a timer's) leaves a
// button a tap focused where it is. The one exception is the press's own
// work: what it mounts (a field, or a sheet, dialog or popover whose field
// autofocuses) may take the focus the press left on its control, as a click
// that shows a field hands it the focus on the web host (host/web-js/focus.js,
// host/web/navigation.js's `pointerTarget`).
#if os(iOS) || os(tvOS)
import UIKit

/// A press's permission to hand the focus it left on its control to an
/// autofocus its own work mounts. While its dispatch runs, and while a sync
/// that dispatch deferred behind a running transition runs, any autofocus
/// may take it. After, only one inside a route that work presented or
/// pushed (a destination), logically: a composer's field that a later data
/// answer mounts in its sheet. It ends once an autofocus takes the focus
/// through it, when no destination is left in the tree and no deferred
/// sync owes it, or at the next contact, key, typed text, focus choice (Tab,
/// `focus(id)`) or the scene leaving the foreground.
final class PressHandoff {
    /// The responder the press left focused (`focusForPress`): the innermost
    /// node that asked for focus, a field the press focused, else the
    /// press's target.
    weak var control: UIView?
    private weak var presenter: Presenter?
    /// Its dispatch is running.
    private var dispatching = true
    /// A sync its dispatch deferred is owed (`pending`) or running (`syncing`).
    var pending = false
    var syncing = false
    private struct Destination { weak var route: NodeView? }
    private var destinations: [Destination] = []
    fileprivate init(_ presenter: Presenter, control: UIView?) { self.presenter = presenter; self.control = control }
    /// Whether its own work is running now: what that work presents or
    /// pushes is its destination.
    var working: Bool { dispatching || syncing }
    /// The press's dispatch returned.
    func dispatched() {
        dispatching = false
        settle()
    }
    fileprivate func record(_ route: NodeView) {
        if !destinations.contains(where: { $0.route === route }) { destinations.append(Destination(route: route)) }
    }
    /// A destination left (its sheet was dismissed); any other stays.
    func drop(_ route: NodeView) {
        destinations.removeAll { $0.route === route }
        settle()
    }
    /// Whether an autofocus on `candidate` may take the focus the press left.
    fileprivate func allows(_ candidate: NodeView) -> Bool {
        if working { return true }
        prune()
        guard let presenter, !destinations.isEmpty else { return false }
        // Through authored parents: a sheet's, a lifted popover's (`listenerParent`).
        return sequence(first: candidate as UIView, next: presenter.listenerParent).contains { view in destinations.contains { $0.route === view } }
    }
    /// A route whose nodes left the presenter's tree is gone.
    private func prune() {
        destinations.removeAll { d in d.route.map { presenter?.views[$0.id] !== $0 } ?? true }
    }
    func settle() {
        prune()
        if !working, !pending, destinations.isEmpty, let presenter, presenter.pressHandoff === self { presenter.pressHandoff = nil }
    }
}

extension Presenter {
    /// A press settles its focus (`focusForPress`, a synthetic tap's field,
    /// VoiceOver's activation): its handoff begins, superseding any other.
    func beginPress(control: UIView?) -> PressHandoff {
        let handoff = PressHandoff(self, control: control)
        pressHandoff = handoff
        return handoff
    }
    /// A route the live press's own work presents or pushes.
    func pressShows(_ route: NodeView?) {
        guard let route, let handoff = pressHandoff, handoff.working else { return }
        handoff.record(route)
    }
    /// A contact going down, a key, typed text, a remote's scroll or focus
    /// move, or the scene leaving the foreground ends the live press's handoff.
    func supersedePress() { pressHandoff = nil }
    /// Whether `view` holds the focus a live press left on its control,
    /// which an autofocus on `candidate` its work mounted may take.
    func pressHandsOff(_ view: UIView, to candidate: NodeView) -> Bool {
        // A field's editor inside its text field holds the focus with it.
        guard let handoff = pressHandoff, let control = handoff.control,
              view === control || !(control is NodeView) && view.isDescendant(of: control) else { return false }
        return handoff.allows(candidate)
    }
    /// An autofocus took the focus: one the press's handoff allowed spends it.
    func autofocused(_ node: NodeView) {
        node.focusedBySurfaceTouch = false
        if pressHandoff?.allows(node) == true { pressHandoff = nil }
    }
    /// Tab, `focus(id)` or Return's next field chose `target`: the app's
    /// focus, which no press hands on.
    func chooseFocus(_ target: NodeView) {
        target.focusedBySurfaceTouch = false
        pressHandoff = nil
    }
}
#endif
