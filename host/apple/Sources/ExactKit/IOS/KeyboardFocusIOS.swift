#if os(iOS)
import UIKit

extension ExactView {
    /// Keep a responder only for authored keyboard input. A field, controller
    /// or native menu retains ownership of the window's input.
    func syncKeyboardFocus(afterMenu: Bool = false) {
        let p = session.presenter
        guard session.state != .destroyed else { return }
        guard canBecomeFirstResponder else {
            if isFirstResponder { _ = resignFirstResponder() }
            return
        }
        guard !isFirstResponder, let window, window.isKeyWindow, !p.menus.focus.suspended,
              !p.navigation.defersFocus, !p.modals.defersFocus, p.pendingFocusNode == nil else { return }
        if let responder = FirstResponder.current, responder !== self,
           !(afterMenu && String(describing: type(of: responder)).hasPrefix("_UITypeSelect")) { return }
        func occupied(_ view: UIView) -> Bool {
            if view !== self, view.isFirstResponder { return true }
            if let controller = view.next as? UIViewController, controller.isFirstResponder { return true }
            return view.subviews.contains(where: occupied)
        }
        guard !occupied(window) else { return }
        _ = becomeFirstResponder()
    }
}
#endif
