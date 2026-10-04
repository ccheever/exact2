// @ref LLP 1055.000 D14 — CSS `filter` on a box, on Apple (Filter Effects 1
// §12): the web lowers it natively; here the box's subtree is drawn as it
// would show unfiltered, the function chain runs on the GPU (the SVG
// island's `SvgFilterGPU`, one implementation for boxes and SVG groups),
// and the result shows in a layer standing where the box stands. The box
// itself stays in the tree, masked to nothing, so it still takes hits, is
// still what the accessibility tree and the agent read, and still lays out.
// The picture is drawn again after every batch while the box is filtered,
// so a change or a sampled animation inside it shows (a lowered animation
// under a filtered box is sampled instead: `svg_lower::eligibility`).
import CoreGraphics
import QuartzCore

final class BoxFilter {
    /// Stands where the box stands (its geometry mirrored), clipped by the
    /// box's `clip-path` after the filter, as CSS orders them.
    let picture = CALayer()
    /// The filtered pixels, over the region in the box's bounds space.
    private let content = CALayer()
    /// The box's mask while filtered: nothing of the box itself shows.
    let hide = CALayer()
    /// `Filter::encode` over a box of no size: its region is how far past
    /// the box the result reaches.
    private var program: [Float] = []

    init() {
        for l in [picture, content, hide] {
            l.actions = ["contents": NSNull(), "bounds": NSNull(), "position": NSNull(), "transform": NSNull(),
                         "opacity": NSNull(), "hidden": NSNull(), "zPosition": NSNull(), "sublayers": NSNull(), "mask": NSNull()]
        }
        content.anchorPoint = .zero
        picture.addSublayer(content)
    }

    /// The style's `filter` (`{"p": [...], "rc": [...]}`); whether the box
    /// is filtered (a chain this host runs, on a device with a GPU).
    func set(_ value: BatchValue?) -> Bool {
        guard case .object(let o)? = value, let p = o["p"]?.numbers, p.count > 5 else { return false }
        program = p.map(Float.init)
        return SvgFilterGPU.runs(program)
    }

    /// Draw the box's picture again, `scale` pixels per point. `clip`: the
    /// box's `clip-path` as a mask for the picture.
    func render(_ box: CALayer, clip: CALayer?, scale: CGFloat) {
        guard let parent = box.superlayer, program.count > 5 else { return }
        CATransaction.begin()
        CATransaction.setDisableActions(true)
        defer { CATransaction.commit() }
        let b = box.bounds
        let region = CGRect(x: b.minX + CGFloat(program[0]), y: b.minY + CGFloat(program[1]),
                            width: b.width + CGFloat(program[2]), height: b.height + CGFloat(program[3]))
        if picture.superlayer !== parent || parent.sublayers?.firstIndex(of: picture) != (parent.sublayers?.firstIndex(of: box)).map({ $0 + 1 }) {
            picture.removeFromSuperlayer()
            parent.insertSublayer(picture, above: box)
        }
        picture.bounds = b
        picture.anchorPoint = box.anchorPoint
        picture.position = box.position
        picture.transform = box.transform
        picture.opacity = box.opacity
        picture.isHidden = box.isHidden
        picture.mask = clip
        guard let (rect, w, h, _) = SvgIsland.extent(region, k: scale, seen: nil, limit: SvgIsland.budget / 24),
              let space = CGColorSpace(name: CGColorSpace.sRGB),
              let ctx = CGContext(data: nil, width: w, height: h, bitsPerComponent: 8, bytesPerRow: w * 4, space: space,
                                  bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue) else {
            content.contents = nil
            return
        }
        // Top row first, points to pixels, the region's corner at zero.
        ctx.translateBy(x: 0, y: CGFloat(h))
        ctx.scaleBy(x: 1, y: -1)
        ctx.scaleBy(x: CGFloat(w) / rect.width, y: CGFloat(h) / rect.height)
        ctx.translateBy(x: -rect.minX, y: -rect.minY)
        // The box as it shows unfiltered: without the mask that hides it
        // (or its clip, which the picture takes), at its own opacity (the
        // picture's).
        let (mask, opacity) = (box.mask, box.opacity)
        box.mask = nil
        box.opacity = 1
        box.render(in: ctx)
        box.mask = mask
        box.opacity = opacity
        guard let source = ctx.makeImage(),
              let out = SvgFilterGPU.run(program, source: source, origin: rect.origin,
                                         scale: CGSize(width: CGFloat(w) / rect.width, height: CGFloat(h) / rect.height)) else {
            content.contents = nil
            return
        }
        content.frame = rect
        content.contents = out
    }

    /// The box is no longer filtered.
    func remove() {
        picture.removeFromSuperlayer()
        content.contents = nil
    }
}

/// A presenter's filtered boxes, drawn again after each batch.
final class BoxFilters {
    private var views: [ObjectIdentifier: () -> Void] = [:]

    func add(_ view: AnyObject, render: @escaping () -> Void) { views[ObjectIdentifier(view)] = render }
    func remove(_ view: AnyObject) { views.removeValue(forKey: ObjectIdentifier(view)) }
    var isEmpty: Bool { views.isEmpty }

    /// Every filtered box's picture again.
    func render() { for r in views.values { r() } }
}
