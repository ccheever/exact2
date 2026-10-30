#if os(macOS)
import AppKit

/// A textarea's `resize` grip (`ResizeGrip.swift`): above its scroller, it
/// takes the mouse down in its corner and follows the drag.
final class ResizeGrip: NSView {
    weak var owner: NodeView?
    var rtl = false { didSet { if rtl != oldValue { needsDisplay = true } } }
    /// The axes its cursor was chosen for.
    var axes = (width: false, height: false)

    override var isFlipped: Bool { true }
    override func acceptsFirstMouse(for event: NSEvent?) -> Bool { true }

    override func draw(_ dirtyRect: NSRect) {
        NSColor.secondaryLabelColor.setStroke()
        let path = NSBezierPath()
        path.lineWidth = 1
        for (from, to) in ResizeGripShape.strokes(rtl: rtl) {
            path.move(to: from)
            path.line(to: to)
        }
        path.stroke()
    }

    override func viewDidChangeEffectiveAppearance() {
        super.viewDidChangeEffectiveAppearance()
        needsDisplay = true
    }

    /// The axis's resize cursor; a corner's for `both`.
    override func resetCursorRects() {
        let name = axes.width && axes.height ? (rtl ? "nesw-resize" : "nwse-resize") : axes.width ? "ew-resize" : "ns-resize"
        if let cursor = CSSCursor.value(name) { addCursorRect(bounds, cursor: cursor) }
    }

    private var start: (local: NSPoint, across: NSPoint, size: CGSize)?

    override func mouseDown(with event: NSEvent) {
        guard let owner, let across = owner.superview?.convert(event.locationInWindow, from: nil) else { return }
        start = (owner.convert(event.locationInWindow, from: nil), across, owner.laidOutFrame.size)
    }

    override func mouseDragged(with event: NSEvent) {
        guard let owner, let start, let across = owner.superview?.convert(event.locationInWindow, from: nil) else { return }
        owner.resizeGripDragged(pointer: owner.convert(event.locationInWindow, from: nil), start: start.local,
                                size: start.size, movedX: across.x - start.across.x)
    }

    override func mouseUp(with event: NSEvent) { start = nil }
}

extension NodeView {
    /// Shows the grip at the corner `resize` gives one, or takes it away.
    func layoutResizeGrip() {
        let existing = subviews.lazy.compactMap { $0 as? ResizeGrip }.first
        let axes = resizeAxes
        guard axes.width || axes.height else { existing?.removeFromSuperview(); return }
        let grip = existing ?? ResizeGrip(frame: .zero)
        if grip.superview == nil {
            grip.owner = self
            addSubview(grip)
        } else if subviews.last !== grip {
            addSubview(grip, positioned: .above, relativeTo: nil)
        }
        let (rtl, frame) = (resizeGripRTL, resizeGripFrame)
        if grip.frame != frame || grip.rtl != rtl || grip.axes != axes {
            (grip.rtl, grip.frame, grip.axes) = (rtl, frame, axes)
            window?.invalidateCursorRects(for: grip)
        }
    }
}
#endif
