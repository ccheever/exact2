// Agent and canvas pictures follow the same sibling order as input.
// Copy the layer tree, never reorder the live views (Tab and AX tree order).
#if os(macOS)
import AppKit

extension Capture {
    static func paintOrderBitmap(of view: NSView, scale: CGFloat? = nil) -> NSBitmapImageRep? {
        // A canvas overlay can be captured before AppKit has installed its
        // inherited backing layer (its window need not have displayed yet).
        view.wantsLayer = true
        view.layoutSubtreeIfNeeded()
        view.displayIfNeeded()
        let scale = scale ?? view.window?.backingScaleFactor ?? 2
        guard let root = view.layer else { return nil }
        CATransaction.begin(); CATransaction.setDisableActions(true)
        defer { CATransaction.commit() }
        guard let image = picture(of: paintOrderLayer(of: view, root: root), size: view.bounds.size, scale: scale) else { return nil }
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
