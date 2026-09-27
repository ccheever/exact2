#if os(macOS)
import AppKit

/// The viewport: a click that reached it — on no node that takes the focus
/// or a press — ends the editing, as a click on a page's blank ground blurs
/// the field (LLP 1008 §9).
final class PageScrollView: NSScrollView {
    private struct DocumentFit {
        let document: ObjectIdentifier
        let documentFrame: NSRect
        let clipSize: NSSize
        let frameSize: NSSize
        let style: NSScroller.Style
        let border: NSBorderType
        let vertical: Bool
        let horizontal: Bool
        let autohides: Bool
    }
    private var documentFit: DocumentFit?
    private var prefittingDocument = false
    var permitsDocumentPrefit: (() -> Bool)?

    func invalidateDocumentFit() { documentFit = nil }

    /// Only a completed root fit may certify axes for the next outer shrink.
    func acceptDocumentFit() {
        guard !prefittingDocument, let document = documentView,
              document.frame.origin == .zero, document.bounds.size == document.frame.size else { return }
        documentFit = DocumentFit(document: ObjectIdentifier(document), documentFrame: document.frame,
            clipSize: contentView.bounds.size, frameSize: frame.size, style: scrollerStyle,
            border: borderType, vertical: hasVerticalScroller, horizontal: hasHorizontalScroller,
            autohides: autohidesScrollers)
    }

    override func setFrameSize(_ newSize: NSSize) {
        if newSize != frame.size, !prefittingDocument {
            prefittingDocument = true
            prefitDocument(for: newSize)
            super.setFrameSize(newSize)
            prefittingDocument = false
        } else {
            super.setFrameSize(newSize)
        }
    }

    /// An old fitting document must not manufacture gutters during tiling.
    /// Keep overflowing axes and authored root frames untouched; ordinary layout
    /// can still introduce real overflow and its necessary viewport correction.
    private func prefitDocument(for newSize: NSSize) {
        let accepted = documentFit
        documentFit = nil
        guard permitsDocumentPrefit?() == true, let fit = accepted, let document = documentView,
              ObjectIdentifier(document) == fit.document, document.frame == fit.documentFrame,
              document.bounds.size == document.frame.size, frame.size == fit.frameSize,
              bounds.size == frame.size, contentView.bounds.size == fit.clipSize,
              scrollerStyle == fit.style, borderType == fit.border,
              hasVerticalScroller == fit.vertical, hasHorizontalScroller == fit.horizontal,
              autohidesScrollers == fit.autohides, !automaticallyAdjustsContentInsets,
              contentInsets.top == 0, contentInsets.left == 0,
              contentInsets.bottom == 0, contentInsets.right == 0,
              newSize.width.isFinite, newSize.height.isFinite,
              newSize.width > 0, newSize.height > 0 else { return }
        let width = fit.clipSize.width + newSize.width - fit.frameSize.width
        let height = fit.clipSize.height + newSize.height - fit.frameSize.height
        guard width.isFinite, height.isFinite, width > 0, height > 0 else { return }
        var size = document.frame.size
        if size.width <= fit.clipSize.width, newSize.width < fit.frameSize.width {
            size.width = min(size.width, width)
        }
        if size.height <= fit.clipSize.height, newSize.height < fit.frameSize.height {
            size.height = min(size.height, height)
        }
        if size != document.frame.size { document.setFrameSize(size) }
    }

    override func mouseDown(with event: NSEvent) {
        window?.makeFirstResponder(nil)
        super.mouseDown(with: event)
    }
    /// AppKit turns automatic titlebar insets back on when this view
    /// becomes a window's content view, which leaves a black strip the
    /// height of the titlebar above the document (the night, the deck).
    override func viewDidMoveToWindow() {
        super.viewDidMoveToWindow()
        pinInsets()
    }
    override func tile() {
        super.tile()
        pinInsets()
        syncElasticity()
    }
    /// A page that fits does not rubber-band.
    ///
    /// AppKit's `.automatic` elasticity bounces an axis the page cannot
    /// actually scroll, so an inner pane that reaches its end and chains the
    /// rest of the gesture up here (`ChainingScrollView`) drags the whole
    /// window's content — the app's own chrome with it — even though the
    /// document is exactly the viewport. The browser's rule is the one to
    /// match: an axis is elastic only while it has somewhere to go. Chaining
    /// itself is unchanged; a page that really does scroll still bounces.
    func syncElasticity() {
        let document = documentView?.frame.size ?? .zero
        let visible = contentView.bounds.size
        // Half a point of slack: a fractional layout must not read as scrollable.
        let x: NSScrollView.Elasticity = document.width - visible.width > 0.5 ? .automatic : .none
        let y: NSScrollView.Elasticity = document.height - visible.height > 0.5 ? .automatic : .none
        if horizontalScrollElasticity != x { horizontalScrollElasticity = x }
        if verticalScrollElasticity != y { verticalScrollElasticity = y }
    }
    func pinInsets() {
        if automaticallyAdjustsContentInsets { automaticallyAdjustsContentInsets = false }
        if contentInsets.top != 0 || contentInsets.left != 0 || contentInsets.bottom != 0 || contentInsets.right != 0 {
            contentInsets = NSEdgeInsetsZero
        }
    }
}

#endif
