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

/// A press while its work may run. Its dispatch, and a presentation or push
/// that dispatch started while UIKit runs it (or a sync a running transition
/// deferred), let any autofocus take the focus. Then each route it presented
/// or pushed (a destination) still lets the first autofocus mounted inside it
/// take the focus, as a composer's field that a later data answer mounts,
/// until that route leaves the tree. It ends once an autofocus takes the
/// focus through it, once none of that is left, or at the next touch, key or
/// focus choice (Tab, `focus(id)`).
final class PressHandoff {
    /// The responder the press left focused (`focusForPress`): the innermost
    /// node that asked for focus, a field the press focused, else the
    /// press's target.
    weak var control: UIView?
    private weak var presenter: Presenter?
    private var work = 1
    private struct Destination { weak var route: NodeView? }
    private var destinations: [Destination] = []
    fileprivate init(_ presenter: Presenter, control: UIView?) { self.presenter = presenter; self.control = control }
    /// The press's dispatch returned.
    func dispatched() { release() }
    /// A presentation or push the press's work started, released when UIKit
    /// has finished it; `destination`, the route it shows.
    fileprivate func hold(destination: NodeView?) -> () -> Void {
        work += 1
        if let destination { destinations.append(Destination(route: destination)) }
        var released = false
        return { [weak self] in
            guard !released else { return }
            released = true
            self?.release()
        }
    }
    /// A route the press presented left (its sheet was dismissed): the
    /// press's work is over, and it hands nothing on.
    func drop(_ route: NodeView) {
        guard destinations.contains(where: { $0.route === route }), let presenter, presenter.pressHandoff === self else { return }
        presenter.pressHandoff = nil
    }
    /// Its dispatch, or what that started, still runs: a sync it deferred
    /// or a presentation may hold it. A handoff only a destination keeps
    /// alive takes no more work.
    fileprivate var holdsWork: Bool { work > 0 }
    /// Whether an autofocus on `candidate` may take the focus the press left.
    fileprivate func allows(_ candidate: NodeView) -> Bool {
        prune()
        return work > 0 || destinations.contains { $0.route.map { candidate === $0 || candidate.isDescendant(of: $0) } == true }
    }
    private func release() {
        work -= 1
        settle()
    }
    /// A route whose nodes left the presenter's tree is gone.
    private func prune() {
        destinations.removeAll { d in d.route.map { presenter?.views[$0.id] !== $0 } ?? true }
    }
    private func settle() {
        prune()
        if work <= 0, destinations.isEmpty, let presenter, presenter.pressHandoff === self { presenter.pressHandoff = nil }
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
    /// The presentation or push the live press's work starts holds the
    /// handoff until `release` runs, when UIKit has finished it; nil without
    /// a live press.
    func holdPress(destination: NodeView? = nil) -> (() -> Void)? { pressHandoff?.holdsWork == true ? pressHandoff?.hold(destination: destination) : nil }
    /// A contact going down or a key supersedes the live press: one landing
    /// in its control is that control's next press.
    func supersedePress(touching view: UIView? = nil) {
        if let control = pressHandoff?.control, let view, view.isDescendant(of: control) { return }
        pressHandoff = nil
    }
    /// Whether `view` holds the focus a live press left on its control,
    /// which an autofocus on `candidate` its work mounted may take.
    func pressHandsOff(_ view: UIView, to candidate: NodeView) -> Bool {
        // A field's editor inside its text field holds the focus with it.
        guard let handoff = pressHandoff, let control = handoff.control,
              view === control || !(control is NodeView) && view.isDescendant(of: control) else { return false }
        return handoff.allows(candidate)
    }
    /// Tab, `focus(id)`, Return's next field or an autofocus chose `target`:
    /// the app's focus, which no press hands on.
    func chooseFocus(_ target: NodeView) {
        target.focusedBySurfaceTouch = false
        pressHandoff = nil
    }
}
#endif
