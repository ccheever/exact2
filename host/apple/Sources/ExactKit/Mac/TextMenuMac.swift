// @ref LLP 1115.000 D7.9: the menu for selected page text is AppKit's own.
// AppKit builds that menu only for an `NSTextView` (Look Up and Translate
// with the selection in their titles, Search With the person's search
// engine, Copy, Share, Writing Tools, Speech), and it is not something a
// host can assemble: half of its items belong to private classes that read
// the text view they were made for. Page text is not a text view: Exact
// draws and selects it itself (`TextSelection`). So the menu is asked of a
// text view that only holds what is selected, placed over it and never seen,
// and that menu is what the page's view answers `menu(for:)` with.
#if os(macOS)
import AppKit

/// The text view AppKit's menu for selected page text is asked of. It holds
/// the selected text in the page's own fonts, all of it selected, over the
/// selection's first line, so what the menu's items do (Look Up's panel, the
/// share sheet, Translate) is about that text and stands where it is. It
/// draws nothing, takes no click and no focus, and is no element to
/// assistive technology: the page's text is what a person sees and selects.
final class SelectionMenuText: NSTextView {
    weak var owner: NodeView?

    static func make() -> SelectionMenuText {
        let view = SelectionMenuText(frame: .zero)
        _ = view.layoutManager
        view.isEditable = false
        view.isSelectable = true
        view.isRichText = false
        view.usesFontPanel = false
        view.importsGraphics = false
        view.drawsBackground = false
        view.textContainerInset = .zero
        view.textContainer?.lineFragmentPadding = 0
        view.textContainer?.widthTracksTextView = true
        view.isVerticallyResizable = false
        view.alphaValue = 0
        view.setAccessibilityElement(false)
        return view
    }

    override func hitTest(_ point: NSPoint) -> NSView? { nil }
    override var acceptsFirstResponder: Bool { false }
    /// Copy is the page's: its `copy` event first and then the whole
    /// selection, as ⌘C (`NodeView.copy`).
    override func copy(_ sender: Any?) { owner?.copy(sender) }

    /// Take `text`, what is selected, a click having landed on `node`'s part
    /// of it (`range`, in its shaped text). A selection inside one paragraph
    /// keeps that paragraph's own runs and wraps as it does: the first line
    /// starts where the selection does, and the lines keep the paragraph's
    /// pitch. One across paragraphs, or in a paragraph its raster worker
    /// lays out, is its plain text in the node's font.
    func hold(_ text: String, of node: NodeView, range: NSRange, event: NSEvent) {
        owner = node
        let spec = node.paragraphSpec(), box = node.paragraphBox()
        let base = node.text?.font(spec.strut ?? node.textRun("")) ?? NSFont.systemFont(ofSize: NSFont.systemFontSize)
        var held = NSMutableAttributedString(string: text, attributes: [.font: base, .foregroundColor: NSColor.textColor])
        var top = node.local(event.locationInWindow).y - base.pointSize / 2, indent: CGFloat = 0, pitch: CGFloat?
        if node.readerParagraph == nil, let paragraph = node.paragraphLayout() {
            let everywhere = CGRect(x: -1e7, y: -1e7, width: 2e7, height: 2e7)
            if let first = paragraph.selectionRects(range, align: spec.align, in: box, dirty: everywhere).first {
                top = first.minY; indent = first.minX - box.minX
            }
            if paragraph.baselines.count > 1 { pitch = paragraph.baselines[1] - paragraph.baselines[0] }
            let whole = node.text?.attributed(spec)
            if let whole, NSMaxRange(range) <= whole.length, whole.attributedSubstring(from: range).string == text {
                held = NSMutableAttributedString(attributedString: whole.attributedSubstring(from: range))
            }
        }
        let style = NSMutableParagraphStyle()
        style.firstLineHeadIndent = max(0, indent)
        if let pitch, let manager = layoutManager {
            let font = held.length > 0 ? held.attribute(.font, at: 0, effectiveRange: nil) as? NSFont : nil
            style.lineSpacing = max(0, pitch - manager.defaultLineHeight(for: font ?? base))
        }
        held.addAttribute(.paragraphStyle, value: style, range: NSRange(location: 0, length: held.length))
        frame = NSRect(x: box.minX, y: top, width: max(1, box.width), height: max(1, node.bounds.maxY - top))
        textStorage?.setAttributedString(held)
        setSelectedRange(NSRange(location: 0, length: held.length))
        if superview !== node { node.addSubview(self) }
    }
}

extension TextSelection {
    /// AppKit's own menu for what is selected, a secondary click having
    /// landed on `node`'s part of it: the menu a read-only `NSTextView`
    /// holding that text gives (`SelectionMenuText`). Nothing selected is
    /// no menu, and under the agent none: AppKit's tracking of it would
    /// hold the agent's click.
    func systemMenu(for node: NodeView, event: NSEvent) -> NSMenu? {
        guard !ExactEnv.agentMode, let window = node.window, let range = range(node), range.length > 0 else { return nil }
        let text = selectedText()
        guard !text.isEmpty else { return nil }
        let view = menuText ?? SelectionMenuText.make()
        menuText = view
        view.hold(text, of: node, range: range, event: event)
        // The text view is asked at its own first character, which is in its
        // selection wherever on the page's selection the click was: it lays
        // a long selection out its own way, and a click it took for one
        // outside its selection would select the word there instead.
        let all = view.selectedRange()
        let first = view.firstRect(forCharacterRange: NSRange(location: 0, length: 1), actualRange: nil)
        let at = window.convertPoint(fromScreen: NSPoint(x: first.midX, y: first.midY))
        guard let asked = NSEvent.mouseEvent(with: .rightMouseDown, location: at, modifierFlags: [], timestamp: event.timestamp,
                                             windowNumber: window.windowNumber, context: nil, eventNumber: 0, clickCount: 1, pressure: 1),
              let menu = view.menu(for: asked) else { return nil }
        view.setSelectedRange(all)
        Self.aim(menu, at: view)
        return menu
    }

    /// AppKit pops the menu over the page's view, where an item with no
    /// target would look for one from the first responder. Such an item is
    /// the text view's own (Copy, Search With, the Speech and Spelling
    /// submenus'), so it is pointed at it; the text view validates it then.
    private static func aim(_ menu: NSMenu, at view: NSTextView) {
        for item in menu.items {
            if let submenu = item.submenu { aim(submenu, at: view) }
            else if item.target == nil, let action = item.action, view.responds(to: action) { item.target = view }
        }
    }

    /// The selection changed: the text view that answered for the last one
    /// holds text that is no longer it.
    func forgetMenuText() {
        menuText?.removeFromSuperview()
        menuText?.textStorage?.setAttributedString(NSAttributedString())
    }
}
#endif
