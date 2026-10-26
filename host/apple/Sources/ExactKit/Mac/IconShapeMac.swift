// A template-path icon on AppKit (LLP 1121 stage 14 C1): an image node whose
// source is an icon path (no SF Symbol, a `symbolPath`) draws that path, each
// `;` path its own shape layer filled by the icon's rule (nonzero, or even-odd
// by `symbolFillRule`, stage 14 C2), at the node's own fractional
// frame in its tint. An NSImageView showing the path as a template NSImage
// resamples it off a whole pixel, lighter and offset (theme2's 07:01 ruling,
// case 3b); SF Symbols and bitmaps keep their NSImageView (NodeSymbolMac.swift).
#if os(macOS)
import AppKit
import ExactCore

/// The leaf in a symbol's clip that fills an icon's paths: top-left origin,
/// never hit, never an accessibility element (the node is the image).
///
/// The shapes sit in one group layer the size of the em square placed in
/// the box, clipped to it as the template image's own bounds clipped it.
/// Each path is filled in the tint's colour opaque, and the tint's alpha is
/// this view's layer opacity (`alphaValue`), applied once over the union:
/// overlapping paths are not composited twice (as the template image was tinted once, Linux's
/// svg paint pushes one opacity and Snapshot masks one tinted rect).
final class IconShapeView: NSView {
    override var isFlipped: Bool { true }
    override func hitTest(_ point: NSPoint) -> NSView? { nil }
    /// The icon's paths in viewBox units, one shape layer each.
    private(set) var paths: [CGPath] = []
    private(set) var shapes: [CAShapeLayer] = []
    /// The em square's layer: clips the shapes.
    let group: CALayer = {
        let layer = CALayer()
        layer.masksToBounds = true
        layer.actions = ["bounds": NSNull(), "position": NSNull(), "contentsScale": NSNull(), "sublayers": NSNull()]
        return layer
    }()
    private var placement = CGAffineTransform.identity, square = CGRect.zero
    private var fill: CGColor?

    /// New paths (the node's `symbolPath`), each filled on its own by `rule`.
    func show(_ paths: [CGPath], rule: CGPathFillRule = .winding) {
        self.paths = paths
        wantsLayer = true
        guard let host = layer else { return }
        if group.superlayer !== host { host.addSublayer(group) }
        while shapes.count > paths.count { shapes.removeLast().removeFromSuperlayer() }
        while shapes.count < paths.count {
            let shape = CAShapeLayer()
            // A path, colour or frame changes at once, as the image did.
            shape.actions = ["path": NSNull(), "fillColor": NSNull(), "bounds": NSNull(), "position": NSNull(), "contentsScale": NSNull()]
            group.addSublayer(shape); shapes.append(shape)
        }
        for shape in shapes { shape.fillRule = rule == .evenOdd ? .evenOdd : .nonZero }
        redraw()
    }

    /// The viewBox placed in this view's flipped bounds (`IconPath.fitted`
    /// into `square`, the box `IconPath.placed` gives, which clips it).
    func place(_ transform: CGAffineTransform, in square: CGRect) {
        placement = transform; self.square = square; redraw()
    }

    /// The fill, resolved by the caller in this view's appearance: its
    /// colour opaque on each path, its alpha once on this view's layer, at
    /// once (the node's own opacity and `visibility` stay on the node and its clip).
    func paint(_ color: CGColor?) {
        fill = color?.copy(alpha: 1)
        wantsLayer = true; layer?.allowsGroupOpacity = true
        CATransaction.begin(); CATransaction.setDisableActions(true)
        alphaValue = color?.alpha ?? 1
        CATransaction.commit()
        for shape in shapes { shape.fillColor = fill }
    }

    override func viewDidChangeBackingProperties() {
        super.viewDidChangeBackingProperties()
        redraw()
    }

    private func redraw() {
        let scale = window?.backingScaleFactor ?? NSScreen.main?.backingScaleFactor ?? 2
        group.frame = square; group.contentsScale = scale
        // The group's own coordinates start at the square's corner.
        let local = placement.concatenating(CGAffineTransform(translationX: -square.minX, y: -square.minY))
        for (shape, path) in zip(shapes, paths) {
            var transform = local
            shape.frame = group.bounds; shape.contentsScale = scale
            shape.path = path.copy(using: &transform); shape.fillColor = fill
        }
    }
}

extension NodeView {
    /// The icon's ink: its tint source (`symbolInk`, as the image view's
    /// `contentTintColor` was), or in a selected row drawn emphasized the
    /// selected item's, as AppKit drew the template cell's image whatever
    /// tinted it (TableInkMac.swift), resolved in this view's appearance.
    /// `emphasized`: the row's state when the caller knows it, else read.
    func tintIconShape(_ emphasized: Bool? = nil) {
        guard let shape = symbolShape else { return }
        let ink = symbolBackground(emphasized ?? inEmphasizedRow) == .emphasized ? NSColor.alternateSelectedControlTextColor : symbolInk
        var fill: CGColor?
        effectiveAppearance.performAsCurrentDrawingAppearance { fill = ink?.cgColor }
        shape.paint(fill)
    }

    /// The icon's paths placed by `object-fit` over the content box: its em
    /// square (`font-size` points) by `IconPath.placed`, the viewBox met in it.
    func placeIconShape(_ shape: IconShapeView) {
        let square = IconPath.placed(em: number("font_size", PageFacts.defaultRootFontSize), fit: style["object_fit"]?.string ?? "fill", in: shape.bounds)
        shape.place(IconPath.fitted(IconPath.viewBox(props["symbolViewBox"] ?? ""), in: square), in: square)
    }
}
#endif
