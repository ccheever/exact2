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

final class DialogSheetController: UIViewController, UIGestureRecognizerDelegate, UIScrollViewDelegate {
    fileprivate weak var host: DialogSheets?
    let scroller = ScrollView()
    /// The dialog's box as the kernel laid it out: the sheet's height.
    private(set) var box = CGSize.zero
    private var backdropTap: UITapGestureRecognizer?
    #if !os(tvOS)
    private var maximumDetentHeight: CGFloat?
    #endif
    /// The presentation's height offer stays independent of the chosen detent.
    var availableHeight: CGFloat? {
        #if !os(tvOS)
        if let maximumDetentHeight { return maximumDetentHeight }
        #endif
        return presentationController?.containerView?.safeAreaLayoutGuide.layoutFrame.height
    }
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
            guard let self else { return context.maximumDetentValue }
            maximumDetentHeight = context.maximumDetentValue
            let height = box.height
            return height > 0 ? min(height.rounded(.up), context.maximumDetentValue) : context.maximumDetentValue
        }]
        #endif
    }
    required init?(coder: NSCoder) { nil }
    #if os(iOS)
    // LLP 1133.029 D5: UIKit asks a presented controller, whose default is
    // not its presenter's answer; a dialog over a locked screen keeps the lock.
    override var supportedInterfaceOrientations: UIInterfaceOrientationMask {
        host?.orientationLock?.supported ?? super.supportedInterfaceOrientations
    }
    @available(iOS 26, *)
    override var prefersInterfaceOrientationLocked: Bool { host?.orientationLock?.locked ?? false }
    #endif

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
        scroller.delegate = self
        scroller.frame = view.bounds
        scroller.autoresizingMask = [.flexibleWidth, .flexibleHeight]
        scroller.contentInsetAdjustmentBehavior = .always
        view.addSubview(scroller)
        self.view = view
    }
    override func viewDidLayoutSubviews() {
        super.viewDidLayoutSubviews()
        place()
        host?.containingBlockChanged()
        host?.geometryChanged()
    }
    func scrollViewDidScroll(_ scrollView: UIScrollView) {
        host?.geometryChanged()
        host?.contentMoved()
    }
    override var keyCommands: [UIKeyCommand]? {
        [UIKeyCommand(input: UIKeyCommand.inputEscape, modifierFlags: [], action: #selector(escape))]
    }
    @objc private func escape() { host?.closeRequest(self) }
    override func accessibilityPerformEscape() -> Bool { host?.closeRequest(self) ?? false }
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
    override func viewDidDisappear(_ animated: Bool) {
        super.viewDidDisappear(animated)
        host?.disappeared(self)
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
        let resized = size != box
        box = size
        // The frame op arrived in the dialog's authored parent coordinates.
        // Recenter before UIKit captures the detent animation's starting geometry.
        place()
        guard resized else { return }
        #if !os(tvOS)
        if let sheet = sheetPresentationController {
            if viewIfLoaded?.window != nil, !ExactEnv.agentFreezes { sheet.animateChanges { sheet.invalidateDetents() } }
            else { sheet.invalidateDetents() }
        }
        #endif
    }
    /// The dialog at the sheet's top, centred across what the sheet shows
    /// between its side insets as HTML centres a modal dialog; a dialog wider
    /// than that, or taller than the sheet, scrolls. The content is that
    /// wide, not the sheet's whole width: UIKit adds the side insets around
    /// it (`.always`), so content as wide as the sheet would scroll sideways
    /// by them (a phone in landscape, the iPhone Duo's status column).
    fileprivate func place() {
        guard isViewLoaded, let dialog = scroller.subviews.lazy.compactMap({ $0 as? NodeView }).first else { return }
        let inset = scroller.adjustedContentInset
        let width = max(box.width, scroller.bounds.width - inset.left - inset.right)
        // Its origin on the pixel grid, as every box's is (`PixelGridIOS.swift`).
        let s = max(1, view.traitCollection.displayScale), size = dialog.bounds.size
        let origin = CGPoint(x: PixelGrid.round((width - size.width) / 2, scale: s),
                             y: PixelGrid.round((box.height - size.height) / 2, scale: s))
        let center = CGPoint(x: origin.x + size.width / 2, y: origin.y + size.height / 2)
        if dialog.center != center { dialog.center = center }
        // A dialog its sheet's height caps (`HostBlock.Axis.fit`) overflows
        // its box, and the sheet scrolls the overflow, as the web's modal
        // dialog (`overflow: auto` under its `max-height`) scrolls itself.
        let overflow = dialog.scrollView != nil ? 0 : dialog.container.subviews.lazy.compactMap { $0 as? NodeView }.map(\.laidOutFrame.maxY).max() ?? 0
        let port = max(0, scroller.bounds.width - inset.left - inset.right)
        let content = CGSize(width: max(port, origin.x + size.width),
                             height: max(box.height, origin.y + max(size.height, overflow)))
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
    #if os(iOS)
    /// Its presenter's orientation lock, which each sheet answers.
    var orientationLock: OrientationLock? { presenter?.orientationLock }
    #endif
    private final class Entry {
        let dialog: NodeView
        let incarnation: UInt64
        weak var parent: UIView?
        var index: Int
        /// The kernel's frame for the dialog, in its authored parent.
        var frame: CGRect
        var usedBlock: CGSize?
        weak var source: NodeView?
        /// What held the keyboard's focus when it opened.
        weak var previous: UIView?
        weak var focused: NodeView?
        let controller: DialogSheetController
        /// What the sheet is over (the page, or the sheet beneath), hidden
        /// from assistive technology while this one shows, and how it stood.
        weak var under: UIView?
        var underHidden = false
        var closing = false
        var retiring = false
        var cancelled = false
        /// Going off screen (`viewWillDisappear`): it hosts no popover meanwhile.
        var leaving = false
        var notifyClose = false
        /// The dismissal of a sheet closed while UIKit was still presenting
        /// it, which UIKit would drop: run when the presentation ends.
        var afterPresenting: (() -> Void)?
        init(_ dialog: NodeView, source: NodeView?, previous: UIView?, controller: DialogSheetController) {
            self.dialog = dialog; self.source = source; self.previous = previous; self.controller = controller
            incarnation = dialog.incarnation
            parent = dialog.superview
            index = parent?.subviews.firstIndex(of: dialog) ?? 0
            frame = dialog.frame
        }
    }
    private var entries: [Entry] = []
    /// Still open in the document, waiting for a retiring route's UIKit stack.
    private var resuming: [Entry] = []
    /// HTML's `cancel` at a dialog a close request closed. Unset, it goes to
    /// the session's runner; a test without a session sets it.
    var onCancel: ((UInt32) -> Void)?

    init(presenter: Presenter) { self.presenter = presenter }
    fileprivate func geometryChanged() { presenter?.nativeGeometry.changed() }

    /// Shown, or still on screen while its sheet leaves.
    func owns(_ node: NodeView) -> Bool { (entries + resuming).contains { $0.dialog === node } }
    /// A dialog's authored parent while it is in its sheet; nil for any other node.
    func authoredParent(of node: NodeView) -> UIView? { (entries + resuming).first { $0.dialog === node }?.parent }
    func preservesFocus(_ id: UInt32) -> Bool { resuming.contains { $0.focused?.id == id && $0.focused === presenter?.views[id] } }
    func isOpen(_ node: NodeView) -> Bool {
        entries.contains { $0.dialog === node && !$0.closing } || resuming.contains { $0.dialog === node }
    }
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
    var containingBlocks: [(UInt32, HostBlock)] {
        entries.compactMap { entry in
            let scroller = entry.controller.scroller
            guard scroller.window != nil, !scroller.bounds.isEmpty,
                  let height = entry.controller.availableHeight, height > 0 else { return nil }
            let inset = scroller.adjustedContentInset
            let size = CGSize(width: max(0, scroller.bounds.width - inset.left - inset.right), height: height)
            return (entry.dialog.id, HostBlock(width: .size(size.width), height: .fit(size.height)))
        }
    }
    fileprivate func containingBlockChanged() { presenter?.owners.publish(); contentMoved() }
    /// A popover over a sheet is in its presentation container, outside
    /// the scroller: it follows its opener as the sheet's content moves.
    fileprivate func contentMoved() {
        presenter?.menus.rehostPopovers()
        // A sticky box in a dialog sticks to the sheet's port (`Sticky.swift`).
        for entry in entries { presenter?.stickies.scrolled(entry.dialog.id) }
    }
    var inTransition: Bool {
        !resuming.isEmpty || entries.contains { $0.closing || $0.controller.isBeingPresented || $0.controller.isBeingDismissed }
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
    /// The laid-out dialog moves with the sheet's scroller; its visible
    /// card clips it without rescaling its descendants (LLP 1051.000 D1).
    var presented: [TopLayerBox] {
        guard let presenter else { return [] }
        return entries.compactMap { entry in
            let controller = entry.controller
            guard !entry.closing, !controller.isBeingDismissed,
                  let view = controller.viewIfLoaded, view.window != nil else { return nil }
            let scroller = controller.scroller
            let clip = NativeGeometry.bounds(of: scroller, in: presenter.viewport)
                .intersection(NativeGeometry.bounds(of: view, in: presenter.viewport))
            let block = entry.usedBlock.map { CGRect(origin: CGPoint(x: -entry.frame.minX, y: -entry.frame.minY), size: $0) }
            return TopLayerBox(view: entry.dialog,
                               frame: (entry.dialog.id, entry.dialog.topLayerFrame(in: presenter.viewport, containingBlock: block), presenter.viewportPoints(clip)),
                               modal: controller)
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
        show(dialog, from: source, over: owner, restoring: nil)
    }
    private func show(_ dialog: NodeView, from source: NodeView?, over owner: UIViewController, restoring old: Entry?) -> Bool {
        guard let presenter, !owns(dialog), live(dialog), dialog.superview != nil else { return false }
        // Laid out first: a closed dialog has no box (`MenuHost.layOut`).
        presenter.menus.layOut(dialog)
        guard live(dialog), dialog.superview != nil else { return false }
        // The focus moves into the dialog, as `showModal()` moves it: a field
        // editing on the page ends its editing, and takes it back at the close.
        let previous: UIView? = old == nil ? presenter.focusedNode.map { $0.textArea ?? $0.field ?? $0 } : old?.previous
        let controller = DialogSheetController(host: self)
        let entry = Entry(dialog, source: source, previous: previous, controller: controller)
        let under: UIView = top?.viewIfLoaded ?? presenter.modals.coordinateView ?? presenter.session?.view ?? presenter.viewport
        entry.under = under
        entry.underHidden = under.accessibilityElementsHidden
        under.accessibilityElementsHidden = true
        dialog.accessibilityViewIsModal = true
        entries.append(entry)
        if let old { presenter.menus.replacePopoverHost(old.controller, with: controller) }
        presenter.controls.syncShown()
        if old == nil { previous?.resignFirstResponder() }
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
            if let leave = entry.afterPresenting {
                entry.afterPresenting = nil
                // Let UIKit finish unwinding the presentation before dismissing.
                DispatchQueue.main.async(execute: leave)
            }
            // The focus asked for below waits for the sheet's window.
            else if let self, let presenter = self.presenter {
                presenter.flushPendingFocus()
                if let field = old?.focused, live(field), presenter.focusedNode == nil { presenter.focusNode(field) }
            }
            DispatchQueue.main.async { self?.presenter?.menus.resumeAfterRouteDismissal() }
        }
        // `autofocus` takes the keyboard each time it opens, as `showModal()` focuses it.
        let field = old == nil ? Self.autofocus(in: dialog) : old?.focused
        if let field, live(field), old == nil || presenter.focusedNode == nil { presenter.focusNode(field) }
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
        if let at = resuming.firstIndex(where: { $0.dialog === dialog }) {
            let entry = resuming.remove(at: at)
            if entries.contains(where: { $0 === entry }) { entry.notifyClose = true }
            else { dialog.queueDialogClose(); presenter?.menus.coverClosed() }
            return
        }
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
        let owner = controller.presentingViewController
        // UIKit drops a dismissal asked for while it presents: that one waits.
        func leave(_ work: @escaping () -> Void) { if controller.isBeingPresented { entry.afterPresenting = work } else { work() } }
        // Only the sheet's own window makes its dismissal observable. Its
        // presenter can have a window before UIKit attaches the sheet, and
        // without a scene that presentation may never complete.
        guard animated, controller.viewIfLoaded?.window?.windowScene != nil, controller.presentingViewController != nil else {
            ended(entry, cancelled: cancelled)
            if controller.presentingViewController != nil {
                leave { owner?.dismiss(animated: false) { [weak self] in self?.presenter?.menus.resumeAfterRouteDismissal() } }
            }
            return
        }
        leave { owner?.dismiss(animated: animated) { [weak self] in self?.ended(entry, cancelled: cancelled) } }
    }
    /// The sheet has gone: the dialog goes back under its authored parent,
    /// hidden, at its kernel frame, and the focus goes back where it was.
    private func ended(_ entry: Entry, cancelled: Bool, restoreFocus: Bool = true) {
        guard let at = entries.firstIndex(where: { $0 === entry }) else { return }
        entries.remove(at: at)
        let cancelled = cancelled || entry.cancelled
        defer {
            DispatchQueue.main.async { [weak self] in
                guard entry.controller.presentingViewController == nil else { return }
                self?.presenter?.menus.resumeAfterRouteDismissal()
            }
        }
        let dialog = entry.dialog
        if dialog.pressContact?.responderContact?.routesBubbledPress == true { dialog.cancelPressContact() }
        dialog.accessibilityViewIsModal = false
        entry.under?.accessibilityElementsHidden = entry.underHidden
        // A focus or scroll still waiting for the sheet's window has nowhere to go.
        if !resuming.contains(where: { $0 === entry }), presenter?.pendingFocusNode?.isDescendant(of: dialog) == true { presenter?.cancelPendingFocus() }
        presenter?.elementScrolls.cancel(inside: dialog)
        dialog.isHidden = true
        if live(dialog), let parent = entry.parent {
            parent.insertSubview(dialog, at: min(entry.index, parent.subviews.count))
            dialog.frame = entry.frame
            presenter?.regrid(dialog)
        } else {
            dialog.removeFromSuperview()
        }
        presenter?.owners.publish()
        // After the dialog is home: a popover anchored in it is placed from there.
        presenter?.menus.rehostPopovers()
        presenter?.menus.coverClosed()
        guard let presenter else { return }
        // Unless the closing action sent the keyboard somewhere itself.
        if restoreFocus, let previous = entry.previous, previous.window != nil, presenter.focusedNode == nil {
            previous.becomeFirstResponder()
        }
        presenter.syncAccessibility()
        // VoiceOver goes back to what opened it, as a closed dialog's focus does.
        if restoreFocus, let source = entry.source, live(source), source.window != nil {
            let element: UIView = source.isNativeButton ? presenter.controls.controls[source.id] ?? source : source
            UIAccessibility.post(notification: .screenChanged, argument: element)
        }
        if cancelled || entry.notifyClose { dialog.queueDialogClose() }
        if cancelled, live(dialog), dialog.handlers.contains("cancel") {
            if let onCancel { onCancel(dialog.id) }
            else if let session = presenter.session { session.applyInput(session.runtime.pickerCancel(dialog.id, now: session.now())) }
        }
    }

    /// A tap on the dimmed page closes only a `closedby="any"` dialog.
    func closesOnBackdrop(_ controller: DialogSheetController) -> Bool {
        guard let entry = entries.first(where: { $0.controller === controller }), !entry.closing else { return false }
        return entry.dialog.props["closedby"] == "any"
    }
    /// A close request the person made outside UIKit's own gesture.
    @discardableResult func closeRequest(_ controller: DialogSheetController) -> Bool {
        guard let entry = entries.first(where: { $0.controller === controller }), !entry.closing, !refuses(entry.dialog) else { return false }
        dismiss(entry, animated: !ExactEnv.agentFreezes, cancelled: true)
        return true
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
    fileprivate func disappeared(_ controller: DialogSheetController) {
        // UIKit clears the presentation links after the appearance callback.
        // Its interactive-dismiss delegate gets first chance to send cancel.
        DispatchQueue.main.async { [weak self, weak controller] in
            guard let self, let controller, controller.presentingViewController == nil,
                  entries.contains(where: { $0.controller === controller && !$0.closing }) else { return }
            for entry in entries.reversed() where !entry.closing && entry.leaving
                && entry.controller.presentingViewController == nil {
                entry.closing = true
                entry.dialog.endEditing(true)
                ended(entry, cancelled: false)
            }
        }
    }
    func presentationControllerDidAttemptToDismiss(_ presentationController: UIPresentationController) {
        guard let entry = entries.first(where: { $0.controller === presentationController.presentedViewController }) else { return }
        presenter?.session?.log("dialog \(entry.dialog.props["id"] ?? "?"): dismissal refused, closedby is none")
    }
    func presentationControllerDidDismiss(_ presentationController: UIPresentationController) {
        guard let entry = entries.first(where: { $0.controller === presentationController.presentedViewController }),
              !entry.closing || entry.retiring else { return }
        resuming.removeAll { $0 === entry }
        entry.cancelled = true
        entry.closing = true
        entry.dialog.endEditing(true)
        // UIKit still uses this hierarchy on its callback stack.
        DispatchQueue.main.async { [weak self] in self?.ended(entry, cancelled: true) }
    }

    func retire(over controller: UIViewController, removing route: NodeView) -> () -> Void {
        resuming = resuming.filter { presenter?.authored($0.dialog, inside: route) != true }
        let affected = entries.filter {
            ViewPresentation.isPresented($0.controller, above: controller)
        }
        for entry in affected where !entry.closing {
            if presenter?.authored(entry.dialog, inside: route) == false {
                entry.focused = (presenter?.pendingFocusNode ?? presenter?.focusedNode).flatMap { presenter?.authored($0, inside: entry.dialog) == true ? $0 : nil }
                resuming.append(entry)
            }
            entry.retiring = true
            entry.closing = true
            if entry.dialog.pressContact?.responderContact?.routesBubbledPress == true { entry.dialog.cancelPressContact() }
            entry.dialog.endEditing(true)
        }
        return { [weak self] in
            guard let self else { return }
            // Unwind accessibility hiding from the top down.
            for entry in affected.reversed() {
                ended(entry, cancelled: false, restoreFocus: resuming.isEmpty)
            }
        }
    }

    /// One at a time: UIKit must finish presenting a sheet before it can present another.
    func resumeAfterRouteDismissal() -> Bool {
        guard let presenter, !presenter.modals.inTransition, !presenter.navigation.inTransition,
              !entries.contains(where: { $0.closing || $0.controller.isBeingPresented || $0.controller.isBeingDismissed }) else { return false }
        while let entry = resuming.first {
            let dialog = entry.dialog
            guard live(dialog), dialog.incarnation == entry.incarnation, entry.parent?.window != nil, dialog.props["semanticTag"] == "dialog",
                  dialog.props["popover"] == nil, !presenter.navigation.isInactiveRoute(containing: dialog),
                  let owner = presenter.menus.controller(for: dialog),
                  owner.viewIfLoaded?.window != nil else {
                resuming.removeFirst()
                presenter.menus.coverClosed()
                continue
            }
            // A busy presenter is temporary; its completion retries the queue.
            guard owner.presentedViewController == nil, !owner.isBeingPresented, !owner.isBeingDismissed else { return false }
            resuming.removeFirst()
            if show(dialog, from: entry.source, over: owner, restoring: entry) { return false }
            presenter.menus.coverClosed()
        }
        return true
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
                // Its children's frames came after its own: the overflow the sheet scrolls.
                entry.controller.place()
            }
        }
        // A destroyed dialog leaves a closing sheet at once: its view may be reused.
        for entry in entries where entry.closing && !live(entry.dialog) && entry.dialog.superview === entry.controller.scroller {
            entry.dialog.removeFromSuperview()
        }
    }
    /// The runner maps a block's viewport rectangle through its used size,
    /// which can exceed the authored dialog's border box.
    func applied(_ op: BatchOp) {
        guard op.payload["op"] as? String == "block",
              let entry = entries.first(where: { $0.dialog.id == op.id && live($0.dialog) }),
              let w = op.payload["w"] as? NSNumber, let h = op.payload["h"] as? NSNumber else { return }
        entry.usedBlock = CGSize(width: w.doubleValue, height: h.doubleValue)
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
        resuming.removeAll { $0.parent === parent && !wanted.contains($0.dialog) }
        for entry in entries where entry.parent === parent {
            if let i = wanted.firstIndex(where: { $0 === entry.dialog }) { entry.index = i }
            else if !entry.closing { dismiss(entry, animated: false, cancelled: false) }
        }
    }
    func reset() {
        resuming.removeAll()
        for entry in entries.reversed() where !entry.closing { dismiss(entry, animated: false, cancelled: false) }
    }
}
#endif
