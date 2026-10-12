// @ref LLP 1115.000 D7.9, D8.5 — the keys the system's key bindings name, at
// a focused node: Shift with an arrow extends the reader's text selection,
// and an arrow, Space or a Page key scrolls the scroller the focus is in.
#if os(macOS)
import AppKit

extension NodeView {
    /// The arrows, Home, End and the Page keys, which the system's key
    /// bindings turn into commands.
    private static let commandKeys: Set<String> = ["ArrowLeft", "ArrowRight", "ArrowUp", "ArrowDown", "Home", "End", "PageUp", "PageDown"]

    /// A key down that no handler, control or press here took (`keyDown`).
    /// One of the command keys is named by the system's key bindings
    /// (`interpretKeyEvents`: the standard ones, and a person's own in
    /// `DefaultKeyBinding.dict`), and the command is performed here
    /// (`doCommand(by:)`). Space is a page down and Shift-Space a page up,
    /// as in every scrolling view of text on a Mac; the bindings would only
    /// insert it. Returns whether the key was taken; one that was not goes
    /// on up the responder chain as before.
    func keyCommand(_ event: NSEvent) -> Bool {
        guard let presenter else { return false }
        let name = NodeView.keyName(event)
        if name == " ", event.modifierFlags.intersection([.command, .control, .option]).isEmpty {
            // Inside a pressable the key goes on up to it, which Space presses.
            if hasPressableAncestor { return false }
            return presenter.keyScroll(event.modifierFlags.contains(.shift) ? "pageUp:" : "pageDown:", from: self)
        }
        guard Self.commandKeys.contains(name) else { return false }
        presenter.keyCommand = (self, false)
        defer { presenter.keyCommand = nil }
        interpretKeyEvents([event])
        return presenter.keyCommand?.taken ?? false
    }

    /// The command the key bindings named for the key `keyCommand` is
    /// interpreting. One that modifies a selection moves the reader's
    /// (`TextSelection.modify`), from a paragraph that has the focus; one
    /// that moves or pages scrolls (`Presenter.keyScroll`). Any other is
    /// left untaken and not passed on, so the key itself goes up the chain;
    /// a command that did not come from there is AppKit's as ever.
    package override func doCommand(by selector: Selector) {
        guard let presenter, presenter.keyCommand?.source === self else { return super.doCommand(by: selector) }
        if (isParagraph && selectsText && presenter.selection.modify(selector)) || presenter.keyScroll(NSStringFromSelector(selector), from: self) {
            presenter.keyCommand?.taken = true
        }
    }
}

extension Presenter {
    /// A key command that scrolls (LLP 1115.000 D8.5): the scroller the
    /// focus is in moves, the nearest one around `view` that scrolls that
    /// way and has somewhere to go, a `scroll` node or the page.
    ///
    /// A page is AppKit's own action, `pageDown:` and `pageUp:`, the two an
    /// `NSScrollView` answers (measured on macOS 27.0: it answers none of the
    /// line or document commands), by its own amount and with its own
    /// animation. A line is the scroll view's own line (`verticalLineScroll`),
    /// and Home and End its document's ends. Under the agent a page is
    /// applied at once, by AppKit's amount, as its wheel is
    /// (`ChainingScrollView.appliesAtOnce`). True when a scroller took it,
    /// at its end too: the key is its.
    func keyScroll(_ command: String, from view: NSView) -> Bool {
        enum Step { case page(CGFloat), line(dx: CGFloat, dy: CGFloat), end(CGFloat) }
        let step: Step
        switch command {
        case "scrollPageDown:", "pageDown:": step = .page(1)
        case "scrollPageUp:", "pageUp:": step = .page(-1)
        case "moveDown:", "scrollLineDown:": step = .line(dx: 0, dy: 1)
        case "moveUp:", "scrollLineUp:": step = .line(dx: 0, dy: -1)
        case "moveRight:": step = .line(dx: 1, dy: 0)
        case "moveLeft:": step = .line(dx: -1, dy: 0)
        case "scrollToEndOfDocument:", "moveToEndOfDocument:": step = .end(1)
        case "scrollToBeginningOfDocument:", "moveToBeginningOfDocument:": step = .end(-1)
        default: return false
        }
        var vertical = true
        if case .line(let dx, _) = step, dx != 0 { vertical = false }
        guard let scroller = keyScroller(around: view, vertical: vertical) else { return false }
        (scroller as? ChainingScrollView)?.collectionWillScroll?()
        let clip = scroller.contentView, document = scroller.documentView?.frame.size ?? .zero
        let limit = NSPoint(x: max(0, document.width - clip.bounds.width), y: max(0, document.height - clip.bounds.height))
        var to = clip.bounds.origin
        switch step {
        case .page(let way):
            if !ChainingScrollView.appliesAtOnce {
                if way > 0 { scroller.pageDown(nil) } else { scroller.pageUp(nil) }
                return true
            }
            to.y += way * max(0, clip.bounds.height - scroller.verticalPageScroll)
        case .line(let dx, let dy): to.x += dx * scroller.horizontalLineScroll; to.y += dy * scroller.verticalLineScroll
        case .end(let way): to.y = way > 0 ? limit.y : 0
        }
        clip.scroll(to: NSPoint(x: min(max(0, to.x), limit.x), y: min(max(0, to.y), limit.y)))
        scroller.reflectScrolledClipView(clip)
        return true
    }

    private func keyScroller(around view: NSView, vertical: Bool) -> NSScrollView? {
        var next = view.enclosingScrollView
        while let scroller = next {
            let document = scroller.documentView?.frame.size ?? .zero, clip = scroller.contentView.bounds.size
            let room = vertical ? document.height - clip.height > 0.5 : document.width - clip.width > 0.5
            let scrolls = (scroller as? ChainingScrollView).map { vertical ? $0.scrollsY : $0.scrollsX } ?? true
            if room && scrolls { return scroller }
            next = scroller.enclosingScrollView
        }
        return nil
    }
}
#endif
