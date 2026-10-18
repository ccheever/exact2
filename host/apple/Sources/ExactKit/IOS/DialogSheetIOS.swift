// @ref LLP 1115.000 D11.5 — a `dialog` that is not a confirmation is a sheet.
// HTML's `dialog`, shown by `showModal()`, is a box its author laid out over
// an inert page. A confirmation by its rows is the platform's alert
// (MenusIOS.swift, LLP 1115 D6); any other is UIKit's sheet, the other half
// of that split by shape. The dialog's own subtree moves into the sheet, as
// macOS moves it into its top layer (DialogsMac.swift), so its fields, their
// text and the app's state are the same ones; the kernel lays it out, and
// the sheet is as tall as that box, up to UIKit's large detent, where it
// scrolls. UIKit dims and blocks the page beneath, and VoiceOver stays in
// the sheet: the dialog is its modal view, and what it is presented over is
// hidden from assistive technology, as a modal route's background is, so the
// driver's `tree --ax` reads what VoiceOver reaches. `closedby` is the
// sheet's dismissal, as a modal route's is (ModalIOS.swift): `none` refuses
// the swipe down; `closerequest`, HTML's default, permits it; `any` also
// takes a tap on the dimmed page. A dismissal the person made fires HTML's
// `cancel` at the dialog.
#if os(iOS) || os(tvOS)
import UIKit

final class DialogSheetController: UIViewController, UIGestureRecognizerDelegate {
    fileprivate weak var host: DialogSheets?
    let scroller = UIScrollView()
    /// The dialog's box as the kernel laid it out: the sheet's height.
    private(set) var box = CGSize.zero
    private var backdropTap: UITapGestureRecognizer?
    #if os(iOS)
    /// What the dialog's authored ancestors listen for: the sheet is out of
    /// the page their recognizers see (`MountRelay`).
    var relay: MountRelay?
    #endif

    fileprivate init(host: DialogSheets) {
        self.host = host
        super.init(nibName: nil, bundle: nil)
        #if os(tvOS)
        // tvOS has no sheets; every modal covers the screen.
        modalPresentationStyle = .overFullScreen
        #else
        modalPresentationStyle = .pageSheet
        // One resting height, the content's: overscroll and dismissal stay
        // UIKit's, and there is no grabber, as for a route's single detent.
        sheetPresentationController?.detents = [.custom(identifier: .init("exact.dialog")) { [weak self] context in
            guard let height = self?.box.height, height > 0 else { return context.maximumDetentValue }
            return min(height.rounded(.up), context.maximumDetentValue)
        }]
        #endif
    }
    required init?(coder: NSCoder) { nil }

    override func loadView() {
        let view = UIView()
        #if os(tvOS)
        view.backgroundColor = .white
        #else
        // The sheet's surface where the dialog painted none: the platform's
        // (LLP 1115 D2), which UIKit elevates in dark mode.
        ThemeRoles.adopt(view, from: host?.root)
        // Its presenter's tint, as the root's (LLP 1095 D4): a sheet sits outside it.
        if let tint = ThemeRoles.color("AccentColor", in: view) { view.tintColor = tint }
        view.backgroundColor = ThemeRoles.color("background", in: view) ?? ThemeRoles.color("Canvas", in: view, or: .systemBackground)
        scroller.keyboardDismissMode = .interactive
        #endif
        scroller.frame = view.bounds
        scroller.autoresizingMask = [.flexibleWidth, .flexibleHeight]
        scroller.contentInsetAdjustmentBehavior = .always
        view.addSubview(scroller)
        self.view = view
    }
    override func viewDidLayoutSubviews() {
        super.viewDidLayoutSubviews()
        place()
    }
    override func viewDidAppear(_ animated: Bool) {
        super.viewDidAppear(animated)
        host?.appeared(self)
        guard backdropTap == nil, let container = presentationController?.containerView else { return }
        host?.observePopovers(in: container)
        let tap = UITapGestureRecognizer(target: self, action: #selector(tappedBackdrop))
        tap.delegate = self
        container.addGestureRecognizer(tap)
        backdropTap = tap
    }
    /// Any dismissal (the app's, a swipe, UIKit's of a stack) or a cover
    /// begins while its container is still in the window.
    override func viewWillDisappear(_ animated: Bool) {
        super.viewWillDisappear(animated)
        host?.disappearing(self)
    }
    func gestureRecognizer(_ gestureRecognizer: UIGestureRecognizer, shouldReceive touch: UITouch) -> Bool {
        if PopoverLayer.containing(touch.view) != nil { return false }
        guard let container = presentationController?.containerView, let presented = presentationController?.presentedView,
              host?.closesOnBackdrop(self) == true else { return false }
        return !presented.convert(presented.bounds, to: container).contains(touch.location(in: container))
    }
    @objc private func tappedBackdrop() { host?.closeRequest(self) }

    /// The dialog's box is `size` now: the sheet follows it, in UIKit's own
    /// animation of a detent change.
    func fit(_ size: CGSize) {
        if size != box {
            box = size
            #if !os(tvOS)
            if let sheet = sheetPresentationController {
                if viewIfLoaded?.window != nil, !ExactEnv.agentFreezes { sheet.animateChanges { sheet.invalidateDetents() } }
                else { sheet.invalidateDetents() }
            }
            #endif
        }
        place()
    }
    /// The dialog at the sheet's top, centred across what the sheet shows
    /// between its side insets as HTML centres a modal dialog; a dialog wider
    /// than that, or taller than the sheet, scrolls. The content is that
    /// wide, not the sheet's whole width: UIKit adds the side insets around
    /// it (`.always`), so content as wide as the sheet would scroll sideways
    /// by them (a phone in landscape, the iPhone Duo's status column).
    private func place() {
        guard isViewLoaded, let dialog = scroller.subviews.lazy.compactMap({ $0 as? NodeView }).first else { return }
        let inset = scroller.adjustedContentInset
        let width = max(box.width, scroller.bounds.width - inset.left - inset.right)
        // Its origin on the pixel grid, as every box's is (`PixelGridIOS.swift`).
        let s = max(1, view.traitCollection.displayScale), size = dialog.bounds.size
        let origin = CGPoint(x: PixelGrid.round((width - size.width) / 2, scale: s),
                             y: PixelGrid.round((box.height - size.height) / 2, scale: s))
        let center = CGPoint(x: origin.x + size.width / 2, y: origin.y + size.height / 2)
        if dialog.center != center { dialog.center = center }
        let content = CGSize(width: width, height: box.height)
        if scroller.contentSize != content { scroller.contentSize = content }
    }
}

/// The dialogs shown as sheets, in the order they opened. An entry is kept
/// until UIKit has finished dismissing its sheet, so the dialog stays where
/// the person sees it until then.
final class DialogSheets: NSObject, UIAdaptivePresentationControllerDelegate {
    private weak var presenter: Presenter?
    /// Its presenter's root: what a sheet's surface resolves the theme by.
    var root: UIView? { presenter?.root }
    private final class Entry {
        let dialog: NodeView
        weak var parent: UIView?
        var index: Int
        /// The kernel's frame for the dialog, in its authored parent.
        var frame: CGRect
        weak var source: NodeView?
        /// What held the keyboard's focus when it opened.
        weak var previous: UIView?
        let controller: DialogSheetController
        /// What the sheet is over (the page, or the sheet beneath), hidden
        /// from assistive technology while this one shows, and how it stood.
        weak var under: UIView?
        var underHidden = false
        var closing = false
        /// Going off screen (`viewWillDisappear`): it hosts no popover meanwhile.
        var leaving = false
        var notifyClose = false
        /// The dismissal of a sheet closed while UIKit was still presenting
        /// it, which UIKit would drop: run when the presentation ends.
        var afterPresenting: (() -> Void)?
        init(_ dialog: NodeView, source: NodeView?, previous: UIView?, controller: DialogSheetController) {
            self.dialog = dialog; self.source = source; self.previous = previous; self.controller = controller
            parent = dialog.superview
            index = parent?.subviews.firstIndex(of: dialog) ?? 0
            frame = dialog.frame
        }
    }
    private var entries: [Entry] = []
    /// HTML's `cancel` at a dialog a close request closed. Unset, it goes to
    /// the session's runner; a test without a session sets it.
    var onCancel: ((UInt32) -> Void)?

    init(presenter: Presenter) { self.presenter = presenter }

    /// Shown, or still on screen while its sheet leaves.
    func owns(_ node: NodeView) -> Bool { entries.contains { $0.dialog === node } }
    /// A dialog's authored parent while it is in its sheet; nil for any other node.
    func authoredParent(of node: NodeView) -> UIView? { entries.first { $0.dialog === node }?.parent }
    func isOpen(_ node: NodeView) -> Bool { entries.contains { $0.dialog === node && !$0.closing } }
    func pressLifetime(of view: UIView) -> (() -> Bool)? {
        guard let entry = entries.first(where: { $0.dialog === view }) else { return nil }
        return { [weak self, weak entry] in
            guard let entry, !entry.closing else { return false }
            return self?.entries.contains { $0 === entry } == true
        }
    }
    /// The sheet a new presentation goes over, and the dialog in it: while
    /// one is up only shortcuts inside it are heard (Shortcuts.swift).
    var top: UIViewController? { entries.last { !$0.closing }?.controller }
    /// The open sheets whose dialog holds `view` in its DOM ancestry, in
    /// opening order. This hosting policy is narrower than HTML's paint
    /// order: inertness alone does not keep a later popover below a modal.
    func controllers(holding view: UIView) -> [UIViewController] {
        var ancestors: [UIView] = [], at: UIView? = view
        while let current = at {
            ancestors.append(current)
            at = presenter?.listenerParent(of: current) ?? current.superview
        }
        return entries.filter { entry in !entry.closing && !entry.leaving && ancestors.contains { $0 === entry.dialog } }.map(\.controller)
    }
    var topDialog: NodeView? { entries.last { !$0.closing }?.dialog }
    var inTransition: Bool {
        entries.contains { $0.closing || $0.controller.isBeingPresented || $0.controller.isBeingDismissed }
    }
    var observation: [String: Any]? {
        guard let entry = entries.last else { return nil }
        let moving = entry.closing || entry.controller.isBeingPresented || entry.controller.isBeingDismissed
        return ["kind": "dialog", "dialog": Int(entry.dialog.id), "source": entry.source.map { Int($0.id) as Any } ?? NSNull(),
                "closedby": entry.dialog.props["closedby"] ?? "closerequest", "presentation": "sheet", "phase": moving ? "transition" : "open"]
    }
    /// LLP 1080.001 D3: the views this host adds.
    func inspectionOwns(_ view: UIView) -> Bool { entries.contains { $0.controller.viewIfLoaded === view || $0.controller.scroller === view } }
    /// The sheets' own views, oldest first: roots of the session's
    /// accessibility tree beside its view (`tree --ax`).
    func surfaces(includingRetiring: Bool) -> [SessionSurface] {
        entries.compactMap { entry in
            guard includingRetiring || !entry.closing, let view = entry.controller.viewIfLoaded else { return nil }
            return SessionSurface(view: view, label: "dialog #\(entry.dialog.id)\(entry.closing ? " (retiring)" : "")",
                                  kind: .dialog, owned: [entry.controller.scroller])
        }
    }
    /// Whether `view` is in one of the sheets: a real touch there is this
    /// session's, though the sheet is not under the session's view.
    func shows(_ view: UIView) -> Bool {
        guard let surface = presenter?.surface(containing: view) else { return false }
        return surface.kind == .dialog || surface.kind == .popover
    }

    fileprivate func observePopovers(in container: UIView) {
        guard let presenter else { return }
        container.addGestureRecognizer(PopoverTouch(host: presenter.menus))
        presenter.menus.rehostPopovers()
    }

    private func live(_ node: NodeView) -> Bool { presenter?.views[node.id] === node }
    private func refuses(_ dialog: NodeView) -> Bool { NavigationRules.modalRefusesDismissal(closedby: dialog.props["closedby"]) }

    /// Present `dialog` over `owner`. Its subtree leaves its authored parent
    /// for the sheet and comes back, hidden, when the sheet has gone.
    func show(_ dialog: NodeView, from source: NodeView?, over owner: UIViewController) -> Bool {
        guard let presenter, !owns(dialog), live(dialog), dialog.superview != nil else { return false }
        // Laid out first: a closed dialog has no box (`MenuHost.layOut`).
        presenter.menus.layOut(dialog)
        guard live(dialog), dialog.superview != nil else { return false }
        // The focus moves into the dialog, as `showModal()` moves it: a field
        // editing on the page ends its editing, and takes it back at the close.
        let previous: UIView? = presenter.focusedNode.map { $0.textArea ?? $0.field ?? $0 }
        let controller = DialogSheetController(host: self)
        let entry = Entry(dialog, source: source, previous: previous, controller: controller)
        let under: UIView = top?.viewIfLoaded ?? presenter.modals.coordinateView ?? presenter.session?.view ?? presenter.viewport
        entry.under = under
        entry.underHidden = under.accessibilityElementsHidden
        under.accessibilityElementsHidden = true
        dialog.accessibilityViewIsModal = true
        entries.append(entry)
        presenter.controls.syncShown()
        previous?.resignFirstResponder()
        controller.loadViewIfNeeded()
        #if os(iOS)
        controller.relay = MountRelay(presenter, mount: controller.view)
        #endif
        controller.scroller.addSubview(dialog)
        dialog.isHidden = false
        controller.isModalInPresentation = refuses(dialog)
        controller.fit(dialog.bounds.size)
        controller.presentationController?.delegate = self
        owner.present(controller, animated: !ExactEnv.agentFreezes) { [weak self, entry] in
            if let leave = entry.afterPresenting { entry.afterPresenting = nil; leave() }
            // The focus asked for below waits for the sheet's window.
            else { self?.presenter?.flushPendingFocus() }
        }
        // `autofocus` takes the keyboard each time it opens, as `showModal()` focuses it.
        if let field = Self.autofocus(in: dialog) { presenter.focusNode(field) }
        presenter.syncAccessibility()
        return true
    }

    private static func autofocus(in view: UIView) -> NodeView? {
        for sub in view.subviews {
            if let node = sub as? NodeView, let value = node.props["autofocus"], value != "false" { return node }
            if let found = autofocus(in: sub) { return found }
        }
        return nil
    }

    /// `close(id)`, or the dialog's own `command="close"` button: the sheet
    /// leaves as UIKit dismisses one, without sending a `cancel` event.
    func close(_ dialog: NodeView, animated: Bool = !ExactEnv.agentFreezes) {
        guard let entry = entries.first(where: { $0.dialog === dialog && !$0.closing }) else { return }
        entry.notifyClose = true
        dismiss(entry, animated: animated, cancelled: false)
    }
    private func dismiss(_ entry: Entry, animated: Bool, cancelled: Bool) {
        if entry.dialog.pressContact?.responderContact?.routesBubbledPress == true { entry.dialog.cancelPressContact() }
        // A finger is taking it down already: UIKit's delegate ends it, and
        // should the finger let it back up, this closes it then.
        if entry.controller.isBeingDismissed, let coordinator = entry.controller.transitionCoordinator, coordinator.isInteractive {
            coordinator.notifyWhenInteractionChanges { [weak self] context in
                if context.isCancelled { DispatchQueue.main.async { self?.dismiss(entry, animated: animated, cancelled: cancelled) } }
            }
            return
        }
        entry.closing = true
        // Sheets over this one leave with it, as UIKit dismisses a stack.
        for above in entries where above !== entry && !above.closing
            && sequence(first: above.controller.presentingViewController, next: { $0?.presentingViewController }).contains(where: { $0 === entry.controller }) {
            above.closing = true
            ended(above, cancelled: false)
        }
        entry.dialog.endEditing(true)
        let controller = entry.controller
        // UIKit drops a dismissal asked for while it presents: that one waits.
        func leave(_ work: @escaping () -> Void) { if controller.isBeingPresented { entry.afterPresenting = work } else { work() } }
        // Nothing to watch leave (the driver's frozen timing, a sheet off
        // screen): the dialog goes home now, the sheet when UIKit takes it.
        guard animated, controller.viewIfLoaded?.window?.windowScene != nil, controller.presentingViewController != nil else {
            ended(entry, cancelled: cancelled)
            if controller.presentingViewController != nil { leave { controller.dismiss(animated: false) } }
            return
        }
        leave { controller.dismiss(animated: true) { [weak self] in self?.ended(entry, cancelled: cancelled) } }
    }
    /// The sheet has gone: the dialog goes back under its authored parent,
    /// hidden, at its kernel frame, and the focus goes back where it was.
    private func ended(_ entry: Entry, cancelled: Bool) {
        guard let at = entries.firstIndex(where: { $0 === entry }) else { return }
        entries.remove(at: at)
        let dialog = entry.dialog
        dialog.accessibilityViewIsModal = false
        entry.under?.accessibilityElementsHidden = entry.underHidden
        // A focus or scroll still waiting for the sheet's window has nowhere to go.
        if presenter?.pendingFocusNode?.isDescendant(of: dialog) == true { presenter?.cancelPendingFocus() }
        presenter?.elementScrolls.cancel(inside: dialog)
        dialog.isHidden = true
        if live(dialog), let parent = entry.parent {
            parent.insertSubview(dialog, at: min(entry.index, parent.subviews.count))
            dialog.frame = entry.frame
            presenter?.regrid(dialog)
        } else {
            dialog.removeFromSuperview()
        }
        // After the dialog is home: a popover anchored in it is placed from there.
        presenter?.menus.rehostPopovers()
        presenter?.menus.coverClosed()
        guard let presenter else { return }
        // Unless the closing action sent the keyboard somewhere itself.
        if let previous = entry.previous, previous.window != nil, presenter.focusedNode == nil {
            previous.becomeFirstResponder()
        }
        presenter.syncAccessibility()
        // VoiceOver goes back to what opened it, as a closed dialog's focus does.
        if let source = entry.source, live(source), source.window != nil {
            let element: UIView = source.isNativeButton ? presenter.controls.controls[source.id] ?? source : source
            UIAccessibility.post(notification: .screenChanged, argument: element)
        }
        if cancelled || entry.notifyClose { dialog.queueDialogClose() }
        if cancelled, live(dialog), dialog.handlers.contains("cancel") {
            if let onCancel { onCancel(dialog.id) }
            else if let session = presenter.session { session.apply(session.runtime.pickerCancel(dialog.id, now: session.now())) }
        }
    }

    /// A tap on the dimmed page closes only a `closedby="any"` dialog.
    func closesOnBackdrop(_ controller: DialogSheetController) -> Bool {
        guard let entry = entries.first(where: { $0.controller === controller }), !entry.closing else { return false }
        return entry.dialog.props["closedby"] == "any"
    }
    /// A close request the person made outside UIKit's own gesture.
    func closeRequest(_ controller: DialogSheetController) {
        guard let entry = entries.first(where: { $0.controller === controller }), !entry.closing, !refuses(entry.dialog) else { return }
        dismiss(entry, animated: !ExactEnv.agentFreezes, cancelled: true)
    }

    // UIKit's own dismissal: the swipe down, and whatever else it gives a sheet.
    func presentationControllerShouldDismiss(_ presentationController: UIPresentationController) -> Bool {
        guard let entry = entries.first(where: { $0.controller === presentationController.presentedViewController }) else { return true }
        return !entry.closing && !refuses(entry.dialog)
    }
    /// Its popovers leave its container while that is still in the window:
    /// UIKit's removal would make a field focused in one resign.
    fileprivate func disappearing(_ controller: DialogSheetController) {
        guard let entry = entries.first(where: { $0.controller === controller }), !entry.leaving else { return }
        entry.leaving = true
        presenter?.menus.rehostPopovers()
    }
    /// On screen again (a cancelled swipe, a cover gone): it hosts them again.
    fileprivate func appeared(_ controller: DialogSheetController) {
        guard let entry = entries.first(where: { $0.controller === controller }), entry.leaving else { return }
        entry.leaving = false
        presenter?.menus.rehostPopovers()
    }
    func presentationControllerDidAttemptToDismiss(_ presentationController: UIPresentationController) {
        guard let entry = entries.first(where: { $0.controller === presentationController.presentedViewController }) else { return }
        presenter?.session?.log("dialog \(entry.dialog.props["id"] ?? "?"): dismissal refused, closedby is none")
    }
    func presentationControllerDidDismiss(_ presentationController: UIPresentationController) {
        guard let entry = entries.first(where: { $0.controller === presentationController.presentedViewController }) else { return }
        entry.closing = true
        entry.dialog.endEditing(true)
        // UIKit still uses this hierarchy on its callback stack.
        DispatchQueue.main.async { [weak self] in self?.ended(entry, cancelled: true) }
    }

    /// After a batch: a dialog that is gone, no longer a dialog, or whose
    /// authored parent left the screen takes its sheet with it; the others
    /// follow their `closedby`.
    func sync() {
        guard let presenter else { return }
        for entry in entries where !entry.closing {
            let dialog = entry.dialog
            let parent = entry.parent
            let shown = parent?.window != nil && parent.map { !presenter.navigation.isInactiveRoute(containing: $0) } == true
            if !live(dialog) || dialog.props["semanticTag"] != "dialog" || dialog.props["popover"] != nil || !shown {
                dismiss(entry, animated: false, cancelled: false)
            } else {
                assign(entry.controller, \.isModalInPresentation, refuses(dialog))
            }
        }
        // A destroyed dialog leaves a closing sheet at once: its view may be reused.
        for entry in entries where entry.closing && !live(entry.dialog) && entry.dialog.superview === entry.controller.scroller {
            entry.dialog.removeFromSuperview()
        }
    }
    /// A frame op on an open dialog: the box is the sheet's content, its
    /// origin the sheet's to choose.
    func framed(_ node: NodeView) {
        guard let entry = entries.first(where: { $0.dialog === node }) else { return }
        entry.frame = node.frame
        if node.superview === entry.controller.scroller { entry.controller.fit(node.bounds.size) }
    }
    /// A children op on a dialog's authored parent: the dialog stays in its
    /// sheet, its index kept for its return; one the op drops closes.
    func children(_ parent: UIView, _ wanted: [NodeView]) {
        for entry in entries where entry.parent === parent {
            if let i = wanted.firstIndex(where: { $0 === entry.dialog }) { entry.index = i }
            else if !entry.closing { dismiss(entry, animated: false, cancelled: false) }
        }
    }
    func reset() {
        for entry in entries.reversed() where !entry.closing { dismiss(entry, animated: false, cancelled: false) }
    }
}
#endif
