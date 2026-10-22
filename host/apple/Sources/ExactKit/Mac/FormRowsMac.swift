// @ref LLP 1115.002 D6 — the Mac's grouped form, where a row's look is a
// control's and not a number: a `destructive` row is AppKit's push button,
// left-aligned and in the label's colour, as SwiftUI's grouped `Form` shows
// a `Button`. The sheet says so in the one way a table of metrics can: the
// row's `appearance` is the metric `grouped-button-appearance`, `auto` in
// the Mac's column and `none` in every other, on a row sized as the button
// (24 points high, its title 13 in from the bezel's ends). The row stays
// Exact's pressable, with its handlers, its focus and its accessibility; the
// bezel under its parts is the platform's picture of it and bounds its hit.
#if os(macOS)
import AppKit

/// The bezel: a push button that draws and does nothing else.
private final class RowBezel: NSButton {
    override func hitTest(_ point: NSPoint) -> NSView? { nil }
    override var acceptsFirstResponder: Bool { false }
    override func isAccessibilityElement() -> Bool { false }
}

extension NodeView {
    var inGroupedForm: Bool {
        for view in sequence(first: self as NSView, next: \.superview) {
            if let node = view as? NodeView, node.kind == "list" { return node.props["listStyle"] != nil }
        }
        return false
    }

    /// A pressable that is no control, on which the sheet's `appearance` is
    /// `auto`: a row the platform draws as its push button.
    var drawsRowBezel: Bool { kind != "control" && pressable && style["appearance"]?.string == "auto" }

    /// The bezel round the row's parts: from the row's padding edge to as
    /// far past its last part as its first is in, as high as its content.
    func layoutRowBezel() {
        let existing = subviews.first { $0 is RowBezel } as? RowBezel
        let parts = subviews.compactMap { $0 as? NodeView }.filter { !$0.isHidden && $0.frame.width > 0 }
        guard drawsRowBezel, let end = parts.map(\.frame.maxX).max() else { existing?.removeFromSuperview(); return }
        let content = contentBox(), lead = padding("left")
        let box = NSRect(x: content.minX - lead, y: content.minY, width: end + lead - (content.minX - lead), height: content.height)
        let bezel = existing ?? RowBezel(title: "", target: nil, action: nil)
        if existing == nil {
            bezel.bezelStyle = .push
            addSubview(bezel, positioned: .below, relativeTo: nil)
        }
        if bezel.isEnabled == disabled { bezel.isEnabled = !disabled }
        let frame = bezel.frame(forAlignmentRect: box)
        if bezel.frame != frame { bezel.frame = frame }
    }

    var rowBezelRect: NSRect? {
        guard let bezel = subviews.first(where: { $0 is RowBezel }) else { return nil }
        return bezel.alignmentRect(forFrame: bezel.frame)
    }

    /// What the driver's `state` says of it: the view, and its box in the row.
    var rowBezelObservation: [String: Any]? {
        guard let box = rowBezelRect else { return nil }
        return ["view": "NSButton(push)", "frame": [Agent.r2(box.minX), Agent.r2(box.minY), Agent.r2(box.width), Agent.r2(box.height)]]
    }
}
#endif
