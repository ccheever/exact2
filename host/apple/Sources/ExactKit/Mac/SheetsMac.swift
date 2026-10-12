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
/// and the window it is a sheet on.
final class DialogSheet {
    let window: SheetWindow
    let content = BlockView(frame: .zero)
    /// The page's window, or the sheet this one stands on.
    private(set) weak var over: NSWindow?
    var begun = false
    init(over: NSWindow, size: CGSize) {
        self.over = over
        window = SheetWindow(contentRect: NSRect(origin: .zero, size: size), styleMask: [.titled], backing: .buffered, defer: false)
        window.isReleasedWhenClosed = false
        // Light or dark as the window it belongs to, whoever set that.
        window.appearanceSource = over
        content.frame = NSRect(origin: .zero, size: size)
        window.contentView = content
    }
    /// The content's size, as layout returned it: the window's.
    func fit(_ size: CGSize) {
        guard size.width > 0, size.height > 0, content.frame.size != size else { return }
        // AppKit keeps an attached sheet centred as its size changes.
        window.setContentSize(size)
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
    /// sheet of one of its dialogs, or one of its popovers'.
    func hosts(_ window: NSWindow?) -> Bool {
        guard let window else { return false }
        return window === viewport.window || dialogs.entries.contains { $0.sheet?.window === window } || menus.hosts(window)
    }
    /// The window a new sheet goes on and the keyboard is in: the open
    /// dialog's sheet, else the page's.
    var frontWindow: NSWindow? { dialogs.entries.last?.sheet?.window ?? viewport.window }
    /// What holds the keyboard for this session: the first responder of the
    /// front-most of its windows that has a node focused (a popover's, the
    /// open dialog's sheet's), else the page's window's.
    var focusResponder: NSResponder? {
        let windows = menus.windowed.reversed().compactMap(\.window) + [frontWindow].compactMap { $0 }
        return windows.lazy.compactMap(\.firstResponder).first { self.keyTarget($0) != nil } ?? viewport.window?.firstResponder
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
    /// then the popovers, which every dialog closes as it opens.
    package var openTopLayerFrames: [(UInt32, CGRect)] {
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
        let painted: [(UInt32, CGRect)] = (popovers + sheets).sorted { depth($0) < depth($1) }.map {
            let rect = clip.convertAcross($0.bounds, from: $0)
            return ($0.id, CGRect(x: rect.minX - clip.bounds.minX,
                                  y: rect.minY - clip.bounds.minY,
                                  width: rect.width, height: rect.height))
        }
        let alerts = (carrying("popover") + carrying("tag:dialog"))
            .filter { self.alerts.isOpen($0) }.map { ($0.id, CGRect.zero) }
        return painted + alerts
    }
}

extension DialogHost {
    /// A sheet's maximum: the content area of the window the page is in.
    private var maximum: CGSize? {
        guard let window = presenter?.viewport.window else { return nil }
        return window.contentLayoutRect.size
    }

    /// Make `entry`'s sheet and move the dialog into it, laid out there.
    func prepare(_ entry: Entry, over front: NSWindow) {
        guard let presenter, let maximum else { return }
        // On top of whatever is attached already: AppKit holds a second
        // sheet on one window back until the first has gone.
        var over = front
        while let top = over.attachedSheet { over = top }
        let sheet = DialogSheet(over: over, size: entry.frame.size)
        entry.sheet = sheet
        presenter.blocks.show(entry.dialog, in: sheet.content, block: .atMost(maximum)) { [weak sheet] size in sheet?.fit(size) }
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
        if let over, over.sheets.contains(sheet.window) { over.endSheet(sheet.window) }
        sheet.window.orderOut(nil)
        if live(dialog) { dialog.frame = entry.frame }
        presenter?.blocks.release(dialog)
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
        guard let presenter, let maximum else { return }
        for entry in entries where entry.sheet != nil { presenter.blocks.resize(entry.dialog, .atMost(maximum)) }
    }

    /// A click on the window behind the open dialog's sheet. AppKit gives
    /// it to nothing; under `closedby="any"` it closes the dialog, when both
    /// halves of it were there. True when the click was the dialog's.
    func pointer(_ event: NSEvent) -> Bool {
        guard let entry = entries.last, let sheet = entry.sheet, event.type == .leftMouseDown || event.type == .leftMouseUp,
              let window = event.window, window !== sheet.window, presenter?.hosts(window) == true,
              entry.dialog.props["closedby"] == "any" else { downBehind = false; return false }
        if event.type == .leftMouseDown { downBehind = true; return true }
        let closes = downBehind
        downBehind = false
        if closes { outside() }
        return true
    }
}
#endif
