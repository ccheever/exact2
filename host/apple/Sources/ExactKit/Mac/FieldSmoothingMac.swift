// `-webkit-font-smoothing: antialiased` in the text AppKit draws for a field
// (LLP 1019 D12): a textarea's text view and an input's field editor lay
// their text out in fragments that draw with Core Graphics font smoothing
// off, and an input's cell draws its value and placeholder so, as Exact
// draws an `antialiased` paragraph.
#if os(macOS)
import AppKit

private final class AntialiasedFragment: NSTextLayoutFragment {
    override func draw(at point: CGPoint, in context: CGContext) {
        context.saveGState()
        context.setShouldSmoothFonts(false)
        super.draw(at: point, in: context)
        context.restoreGState()
    }
}

/// An `antialiased` field's layout delegate. AppKit's own layout, which an
/// `auto` field keeps, has none.
private final class AntialiasedLayout: NSObject, NSTextLayoutManagerDelegate {
    static let shared = AntialiasedLayout()
    func textLayoutManager(_ textLayoutManager: NSTextLayoutManager, textLayoutFragmentFor location: NSTextLocation,
                           in textElement: NSTextElement) -> NSTextLayoutFragment {
        AntialiasedFragment(textElement: textElement, range: textElement.elementRange)
    }
}

extension NodeView {
    var drawsAntialiased: Bool { style["font_smoothing"]?.string == "antialiased" }
}

extension NSTextFieldCell {
    /// The input's node: its field's delegate, or the node that holds it.
    var fieldNode: NodeView? { (controlView as? NSTextField)?.delegate as? NodeView ?? controlView?.superview as? NodeView }
}

extension NSTextView {
    /// The view's fragments draw with font smoothing off, or as AppKit's do.
    /// TextKit 2 keeps a laid-out paragraph's fragment through
    /// `invalidateLayout`; an attributes-only edit makes it ask the delegate
    /// again. That edit is no change of the text: no `input`, no undo step.
    /// A composition keeps the fragments it began with.
    func drawAntialiased(_ on: Bool) {
        guard let manager = textLayoutManager, (manager.delegate === AntialiasedLayout.shared) != on, !hasMarkedText() else { return }
        manager.delegate = on ? AntialiasedLayout.shared : nil
        guard let storage = textStorage, storage.length > 0 else { return }
        storage.beginEditing()
        storage.edited(.editedAttributes, range: NSRange(location: 0, length: storage.length), changeInLength: 0)
        storage.endEditing()
    }
}

/// `draw` with font smoothing off when `on`: a cell's text, a placeholder.
func drawingAntialiased(_ on: Bool, _ draw: () -> Void) {
    guard on, let context = NSGraphicsContext.current?.cgContext else { return draw() }
    context.saveGState()
    context.setShouldSmoothFonts(false)
    draw()
    context.restoreGState()
}
#endif
