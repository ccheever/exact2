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
        let width = Int((view.bounds.width * scale).rounded()), height = Int((view.bounds.height * scale).rounded())
        guard let root = view.layer, width > 0, height > 0,
              let ctx = CGContext(data: nil, width: width, height: height, bitsPerComponent: 8,
                                  bytesPerRow: width * 4, space: CGColorSpace(name: CGColorSpace.sRGB)!,
                                  bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue) else { return nil }
        CATransaction.begin(); CATransaction.setDisableActions(true)
        defer { CATransaction.commit() }
        let tree = paintOrderLayer(of: view, root: root)
        // CA renders the root in bottom-up Quartz space; the agent's
        // bitmap (like AppKit's cacheDisplay) has its first row at the top.
        ctx.translateBy(x: 0, y: CGFloat(height))
        ctx.scaleBy(x: scale, y: -scale)
        tree.render(in: ctx)
        guard let image = ctx.makeImage() else { return nil }
        let rep = NSBitmapImageRep(cgImage: image)
        rep.size = view.bounds.size
        return rep
    }
}
#endif
