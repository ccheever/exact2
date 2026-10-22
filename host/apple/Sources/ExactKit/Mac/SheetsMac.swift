// @ref LLP 1115.002 D7.2 — a `dialog` that is not alert-shaped is a sheet.
// HTML's `dialog`, shown by `showModal()`, is a box its author laid out over
// an inert page. On the Mac that is a sheet: a window of its own, attached
// to the window it was opened in (`beginSheet`), which AppKit centres on it,
// dims it under and keeps it from taking a click. The dialog's own subtree
// moves into that window, as a painted one moved into the top layer, so its
// fields, their text and the app's state are the same ones; the kernel lays
// it out in a containing block that is a maximum on each axis (D8.2: as
// large as its content, at most the window's content area) and the sheet's
// window is given the size that comes back, which nothing reports by hand.
//
// What was measured first, in a sheet of a window really made key (macOS
// 27.0.1; `~/audits/exact2-no-tells-2026-10-11/mac-sheets/`):
// - A titled window (`[.titled]`, which AppKit makes 65 with its sheet bit)
//   is the key window while it is up and its parent stays main; it has no
//   title bar. A borderless one is never key: its keys go to the parent.
// - It arrives with its first text field holding the caret. Tab goes round
//   its fields and no further (Keyboard Navigation off: no button is a stop).
// - Escape and Command-period press the button whose key equivalent is
//   Escape; with none Escape does nothing and the sheet stays. Return in a
//   field ends its editing, sends its action, and is then handed to the
//   window's key equivalents; with the window or a button first responder it
//   presses the default button, which a key sheet draws in the accent.
// - A click on the parent does nothing and its close button is disabled.
//   A second sheet begun on a window that has one waits until the first has
//   gone, so what opens over a sheet (an alert, another dialog) is a sheet
//   on the sheet's own window.
// - AppKit keeps an attached sheet centred on its parent's frame when
//   either changes size, and puts no part of one above its parent's title
//   bar; it leaves no margin around a large one, so the maximum here is the
//   window's content area.
// - `beginSheet` and `endSheet` return after the sheet's animation, about a
//   quarter of a second each. Nothing makes the parent key again when its
//   sheet ends, so this does.
//
// `closedby` (HTML's, as D13.4 of LLP 1115.000 left it): unwritten, or
// `closerequest`, Escape and Command-period close it; `none`, nothing but
// its own buttons does; `any`, a click on the window behind it closes it
// too, which an AppKit sheet of its own never does. The focus on arrival is
// the author's `autofocus`, else the first stop, which is the field AppKit
// would pick. The page under it gives its focus up as its nodes become
// inert (HTML's modal, as under a painted dialog) and has it back, with a
// field's selection, when the sheet has gone.
//
// The driver's own chrome (`--chrome agent`) paints the dialog in the page
// as before (DialogsMac.swift); `--chrome platform` shows the sheet.
#if os(macOS)
import AppKit

/// A sheet's window. Titled, which is what takes the keyboard.
final class SheetWindow: NSWindow {}

/// A dialog's sheet: its window, the view that holds the dialog's subtree,
/// and the window it is a sheet on. A modal route's sheet is the same, with
/// its own title row and buttons round that view (`RouteChrome`, D7.4).
final class DialogSheet {
    let window: SheetWindow
    let rootEdits = RootEdits()
    let content = BlockView(frame: .zero)
    /// A modal route's sheet: its rows round the content. Nil for a dialog's.
    let chrome: RouteChrome?
    /// The page's window, or the sheet this one stands on.
    private(set) weak var over: NSWindow?
    var begun = false
    /// The content's size as layout last returned it.
    private var used = CGSize.zero
    init(over: NSWindow, size: CGSize, chrome: RouteChrome? = nil) {
        self.over = over
        self.chrome = chrome
        window = SheetWindow(contentRect: NSRect(origin: .zero, size: size), styleMask: [.titled], backing: .buffered, defer: false)
        window.isReleasedWhenClosed = false
        // Light or dark as the window it belongs to, whoever set that.
        window.appearanceSource = over
        content.frame = NSRect(origin: .zero, size: size)
        if let chrome { chrome.hold(content); window.contentView = chrome } else { window.contentView = content }
    }
    /// The content's size, as layout returned it: the window's, with a
    /// route's rows round it.
    func fit(_ size: CGSize) {
        guard size.width > 0, size.height > 0 else { return }
        used = size
        // A window's content is whole points: a fraction's is the next one up.
        var whole = chrome?.size(around: size) ?? size
        whole = CGSize(width: whole.width.rounded(.up), height: whole.height.rounded(.up))
        guard window.contentView?.frame.size != whole else { return }
        // AppKit keeps an attached sheet centred as its size changes.
        window.setContentSize(whole)
    }
    /// The rows round the content changed: the window is the same content and the new rows.
    func refit() { fit(used) }
}

extension NodeView {
    /// Whether its ground is the window's background and no author wrote
    /// it: the ground the compiler gives a modal route (LLP 1116.009 D3) and
    /// the Mac's grouped form (LLP 1115.002 D6).
    private var groundIsTheWindows: Bool {
        guard case .object(let role)? = style["background_color"], role["sys"]?.string == "windowBackgroundColor" else { return false }
        return !wrote("background_color")
    }

    /// @ref LLP 1095 D3 — the window's background, as a role, names the
    /// surface a box stands on. In a sheet that surface is the sheet's own
    /// ground: AppKit's, lighter than a window's in Dark Mode (`#2c2828`
    /// over `#1e1e1e`, measured) and no colour a box can paint. So a box in
    /// a sheet that was given that ground and whose author wrote none
    /// paints none, and the sheet's shows; a ground an author wrote is
    /// painted as written.
    var standsOnSheetGround: Bool { window is SheetWindow && groundIsTheWindows }

    /// The subtree moved into a sheet's window, or out of it: its boxes on
    /// the window's ground are painted again, on the ground they now have.
    func sheetGroundChanged() {
        if groundIsTheWindows { applyBoxLayer(); needsDisplay = true }
        for case let child as NodeView in container.subviews { child.sheetGroundChanged() }
    }
}

extension NSView {
    /// `rect` of `view` in this view's space, where a person sees it:
    /// through the screen when the two are in two windows (a sheet's or a
    /// popover's, and the page's).
    func convertAcross(_ rect: NSRect, from view: NSView) -> NSRect {
        guard let theirs = view.window, let mine = window, theirs !== mine else { return convert(rect, from: view) }
        return convert(mine.convertFromScreen(theirs.convertToScreen(view.convert(rect, to: nil))), from: nil)
    }
}

extension Presenter {
    /// Whether `window` holds this session's nodes: its view's window, the
    /// sheet of one of its dialogs or alerts, or one of its popovers'.
    func hosts(_ window: NSWindow?) -> Bool {
        guard let window else { return false }
        return window === viewport.window || window === alerts.sheet || dialogs.entries.contains { $0.sheet?.window === window } || menus.hosts(window)
    }
    /// The window a new sheet goes on and the keyboard is in: the open
    /// dialog's sheet, else the page's.
    var frontWindow: NSWindow? { dialogs.entries.last?.sheet?.window ?? viewport.window }
    /// What holds the keyboard for this session: the first responder of the
    /// front-most of its windows in which something is focused (a popover's,
    /// the open dialog's sheet's), else the page's window's. A native
    /// module's editor is no node, and is what holds it all the same.
    var focusResponder: NSResponder? {
        let windows = menus.windowed.reversed().compactMap(\.window) + [frontWindow].compactMap { $0 }
        return windows.lazy.compactMap(\.firstResponder).first { !($0 is NSWindow) } ?? viewport.window?.firstResponder
    }
    /// The sheets on `window`, bottom to top: its own, the one on that, and so on.
    func sheets(on window: NSWindow?) -> [NSWindow] {
        Array(sequence(first: window?.attachedSheet, next: { $0?.attachedSheet }).prefix { $0 != nil }.compactMap { $0 })
    }

    /// The open popovers and dialogs, back to front, each where a person
    /// sees it in the viewport's space: what the runner's point reads and
    /// its `open` answer are told (`Runtime.on`). One painted in the page's
    /// top layer is ordered by its place there; one in a window of its own
    /// stands in front of the page, the sheets in the order they opened and
    /// then the popovers, which every dialog closes as it opens. And the
    /// index of the front modal dialog's, alert's or route's, a
    /// presentation the runner keeps live with what is above it, and the
    /// rest inert (LLP 1123 D2).
    package var openTopLayer: (frames: [TopLayerFrame], modal: Int?) {
        let sheets = dialogs.presented, popovers = menus.presented
        func depth(_ view: NodeView) -> Int {
            if view.window !== viewport.window {
                let front = viewport.subviews.count
                return front + (sheets.firstIndex { $0 === view } ?? sheets.count + (popovers.firstIndex { $0 === view } ?? 0))
            }
            var at: NSView = view
            while let parent = at.superview, parent !== viewport { at = parent }
            return viewport.subviews.firstIndex(of: at) ?? -1
        }
        let clip = viewport.contentView
        // A dialog or popover in a window of its own is its block's root
        // (BlockHostMac.swift): reported by its block, which the root sits
        // in by its own frame (LLP 1123 D2); one in the page by its border box
        func shown(_ view: NodeView) -> CGRect {
            guard blocks.owns(view), let container = view.superview else { return clip.convertAcross(view.bounds, from: view) }
            let size = blocks.used[view.id] ?? container.bounds.size
            return clip.convertAcross(CGRect(origin: .zero, size: size), from: container)
        }
        let ordered = (popovers + sheets).sorted { depth($0) < depth($1) }
        let painted: [TopLayerFrame] = ordered.map {
            let rect = shown($0)
            return ($0.id, CGRect(x: rect.minX - clip.bounds.minX,
                                  y: rect.minY - clip.bounds.minY,
                                  width: rect.width, height: rect.height), nil)
        }
        let modal = ordered.lastIndex { $0 === dialogs.active || dialogs.presentsRoute($0) }
        if let alert = alerts.presented, let node = alert.node {
            return (painted + [(node.id, CGRect.zero, nil)], alert.popover ? modal : painted.count)
        }
        return (painted, modal)
    }
    package var openTopLayerFrames: [(UInt32, CGRect)] { openTopLayer.frames.map { ($0.0, $0.1) } }
}

extension DialogHost {
    /// A sheet's maximum: the content area of the window the page is in.
    var maximum: CGSize? {
        guard let window = presenter?.viewport.window else { return nil }
        return window.contentLayoutRect.size
    }

    /// Make `entry`'s sheet and move the dialog into it, laid out there.
    func prepare(_ entry: Entry, over front: NSWindow) {
        guard let presenter, maximum != nil else { return }
        // On top of whatever is attached already: AppKit holds a second
        // sheet on one window back until the first has gone.
        var over = front
        while let top = over.attachedSheet { over = top }
        let sheet = DialogSheet(over: over, size: entry.frame.size, chrome: entry.route ? RouteChrome() : nil)
        sheet.rootEdits.presenter = presenter
        sheet.rootEdits.join(sheet.window)
        sheet.content.pointerTracking = { [weak presenter] in presenter?.trackPointer($0) }
        entry.sheet = sheet
        if entry.route { adopt(entry, sheet) }
        guard let block = block(of: entry) else { return }
        presenter.blocks.show(entry.dialog, in: sheet.content, block: block) { [weak sheet] size in sheet?.fit(size) }
        entry.dialog.sheetGroundChanged()
        // A presenter with no session lays nothing out again: the dialog
        // keeps the box the page gave it, at the sheet's corner.
        if presenter.blocks.used[entry.dialog.id] == nil { entry.dialog.setFrameOrigin(.zero) }
    }

    /// Attach the sheet. AppKit returns when it has arrived; false when the
    /// dialog closed meanwhile.
    func begin(_ entry: Entry) -> Bool {
        guard let sheet = entry.sheet, let over = sheet.over, !sheet.begun else { return false }
        sheet.begun = true
        // The focus this host chose is the window's first, or AppKit picks
        // its own first key view as the sheet comes in.
        let focus = focusOwner(sheet.window)
        sheet.window.initialFirstResponder = focus
        over.beginSheet(sheet.window) { _ in }
        guard entries.contains(where: { $0 === entry }) else { return false }
        if let focus, focus.window === sheet.window, focusOwner(sheet.window) !== focus { sheet.window.makeFirstResponder(focus) }
        return true
    }

    /// The sheet leaves and the dialog goes back under its authored parent,
    /// where the kernel lays it out again.
    func end(_ entry: Entry, _ sheet: DialogSheet) {
        let dialog = entry.dialog, over = sheet.over
        let key = sheet.window.isKeyWindow
        sheet.rootEdits.leave(sheet.window)
        if let over, over.sheets.contains(sheet.window) { over.endSheet(sheet.window) }
        sheet.window.orderOut(nil)
        if entry.route { release(entry) }
        if live(dialog) { dialog.frame = entry.frame }
        presenter?.blocks.release(dialog)
        if live(dialog) { dialog.sheetGroundChanged() }
        if key, let over, over.isVisible { over.makeKey() }
    }

    /// What stands on `dialog`'s sheet goes before it does: an alert shown
    /// over it, and the dialogs opened over it.
    func clear(above dialog: NodeView) {
        guard let window = entries.first(where: { $0.dialog === dialog })?.sheet?.window else { return }
        if presenter?.alerts.presented?.window === window { presenter?.alerts.reset() }
        while let top = entries.last, top.dialog !== dialog, top.sheet != nil, entries.contains(where: { $0.dialog === dialog }) {
            close(top.dialog, restoreFocus: false)
        }
    }

    /// The window changed size: a sheet's maximum is its content area.
    func fitSheets() {
        guard let presenter else { return }
        for entry in entries where entry.sheet != nil { if let block = block(of: entry) { presenter.blocks.resize(entry.dialog, block) } }
    }

    /// A click on the window behind the open dialog's sheet. AppKit gives
    /// it to nothing; under `closedby="any"` it closes the dialog, when both
    /// halves of it were there. True when the click was the dialog's.
    func pointer(_ event: NSEvent) -> Bool {
        // Behind it: the page's window, or a sheet under this one. A popover
        // the dialog opened is in front of it, and no click there is outside.
        guard let entry = entries.last, entry.sheet != nil, event.type == .leftMouseDown || event.type == .leftMouseUp,
              let window = event.window, window === presenter?.viewport.window || entries.dropLast().contains(where: { $0.sheet?.window === window }),
              entry.dialog.props["closedby"] == "any" else { downBehind = false; return false }
        if event.type == .leftMouseDown { downBehind = true; return true }
        let closes = downBehind
        downBehind = false
        if closes { outside() }
        return true
    }
}
#endif
