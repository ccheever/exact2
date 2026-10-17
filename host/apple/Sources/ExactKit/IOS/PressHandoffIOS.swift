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

/// A press while its work may run: its dispatch, and a presentation that
/// dispatch started, which UIKit finishes after the press returns (a modal
/// route's sheet, a push), its route's autofocus field mounting meanwhile.
/// It ends when the last of that work does, or once another touch, a key or
/// a focus choice (Tab, `focus(id)`) supersedes it.
final class PressHandoff {
    /// The node the press's focus decision left focused (`focusForPress`):
    /// the innermost that asked for focus, else the press's target.
    weak var control: NodeView?
    private weak var presenter: Presenter?
    private var work = 1
    fileprivate init(_ presenter: Presenter, control: NodeView?) { self.presenter = presenter; self.control = control }
    /// The press's dispatch returned.
    func dispatched() { release() }
    /// A presentation the dispatch started, released when it completes.
    fileprivate func hold() -> () -> Void {
        work += 1
        var released = false
        return { [weak self] in
            guard !released else { return }
            released = true
            self?.release()
        }
    }
    private func release() {
        work -= 1
        if work == 0, let presenter, presenter.pressHandoff === self { presenter.pressHandoff = nil }
    }
}

extension Presenter {
    /// A press settles its focus (`focusForPress`): its handoff begins,
    /// superseding any other.
    func beginPress(control: NodeView?) -> PressHandoff {
        let handoff = PressHandoff(self, control: control)
        pressHandoff = handoff
        return handoff
    }
    /// The presentation the live press's work starts holds the handoff until
    /// `release` runs, when it has completed; nil without a live press.
    func holdPress() -> (() -> Void)? { pressHandoff?.hold() }
    /// Another touch or a key supersedes the live press: one landing in its
    /// control is that control's next press.
    func supersedePress(touching view: UIView? = nil) {
        if let control = pressHandoff?.control, let view, view.isDescendant(of: control) { return }
        pressHandoff = nil
    }
    /// The focus a live press left on its control, which an autofocus its
    /// work mounts may take.
    func pressHandsOff(_ view: UIView) -> Bool { pressHandoff?.control.map { $0 === view } == true }
    /// Tab, `focus(id)` or Return's next field chose `target`: the app's
    /// focus, which no press hands on.
    func chooseFocus(_ target: NodeView) {
        target.focusedBySurfaceTouch = false
        pressHandoff = nil
    }
}
#endif
