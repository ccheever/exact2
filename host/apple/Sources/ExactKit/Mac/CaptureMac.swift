// Agent and canvas pictures follow the same sibling order as input.
// Copy the layer tree, never reorder the live views (Tab and AX tree order).
#if os(macOS)
import AppKit

extension Capture {
    /// AppKit attaches new backing layers at the window display boundary.
    /// Do this once per upload, before Capture.bitmap changes visibility.
    package static func prepareBackingLayers(of view: NSView) {
        view.layoutSubtreeIfNeeded()
        if view.layer == nil, (view as? NSTextView)?.isFieldEditor != true { view.wantsLayer = true }
        func unattached(_ v: NSView) -> Bool {
            // The installed editor inherits backing from its field. Opting it
            // into backing explicitly can interrupt AppKit's editing session.
            if (v as? NSTextView)?.isFieldEditor == true { return false }
            guard let layer = v.layer else { return true }
            if v.superview != nil, layer.superlayer == nil { return true }
            return v.subviews.contains(where: unattached)
        }
        if unattached(view) { view.window?.displayIfNeeded() }
    }

    static func paintOrderBitmap(of view: NSView, scale: CGFloat? = nil, prepared: Bool = false) -> NSBitmapImageRep? {
        if !prepared { prepareBackingLayers(of: view) }
        view.displayIfNeeded()
        let scale = scale ?? view.window?.backingScaleFactor ?? 2
        guard let root = view.layer else { return nil }
        CATransaction.begin(); CATransaction.setDisableActions(true)
        defer { CATransaction.commit() }
        let tree = paintOrderLayer(of: view, root: root)
        guard let image = picture(of: tree, size: view.bounds.size, scale: scale) else { return nil }
        let rep = NSBitmapImageRep(cgImage: image)
        rep.size = view.bounds.size
        return rep
    }

    /// A layer tree drawn into a bitmap of `size` points, its first row at
    /// the top, as AppKit's `cacheDisplay` has it.
    static func picture(of tree: CALayer, size: CGSize, scale: CGFloat) -> CGImage? {
        let width = Int((size.width * scale).rounded()), height = Int((size.height * scale).rounded())
        guard width > 0, height > 0,
              let ctx = CGContext(data: nil, width: width, height: height, bitsPerComponent: 8,
                                  bytesPerRow: width * 4, space: CGColorSpace(name: CGColorSpace.sRGB)!,
                                  bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue) else { return nil }
        // CA renders the root in bottom-up Quartz space.
        ctx.translateBy(x: 0, y: CGFloat(height))
        ctx.scaleBy(x: scale, y: -scale)
        tree.render(in: ctx)
        return ctx.makeImage()
    }
}
#endif
