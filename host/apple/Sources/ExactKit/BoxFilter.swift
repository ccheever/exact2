// @ref LLP 1055.000 D14 — CSS `filter` on a box, on Apple (Filter Effects 1
// §12): the web lowers it natively; here the box's subtree is drawn as it
// would show unfiltered, the function chain runs on the GPU (the SVG
// island's `SvgFilterGPU`, one implementation for boxes and SVG groups),
// and the result shows in a layer standing where the box stands. The box
// itself stays in the tree, masked to nothing, so it still takes hits, is
// still what the accessibility tree and the agent read, and still lays out.
// The picture is drawn again after every batch while the box is filtered,
// and after an image or text raster publishes pixels, so those changes
// show without another batch. An animation under a filtered box is
// sampled instead of lowered (`svg_lower::eligibility`).
//
// A lone `drop-shadow()` is none of that: it is Core Animation's own
// shadow on the box's layer, as a hand-built view casts one under an icon
// over a photo, and the render server draws it. A picture costs a Core
// Image pass on the main thread for every filtered box after every batch,
// which in a photo grid with a shadowed heart on each favourite was most
// of a scroll's main-thread time.
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
    /// the box the result reaches. `darkProgram`: the chain under a dark
    /// appearance, when its colours differ there (LLP 1095 D5).
    private var program: [Float] = []
    private var darkProgram: [Float]?
    /// The chain as a layer's shadow, light and dark, when it is one
    /// `drop-shadow()`; the layer casting it now.
    private var shadow: (light: FilterShadow, dark: FilterShadow)?
    private weak var caster: CALayer?

    init() {
        for l in [picture, content, hide] {
            l.actions = ["contents": NSNull(), "bounds": NSNull(), "position": NSNull(), "transform": NSNull(),
                         "opacity": NSNull(), "hidden": NSNull(), "zPosition": NSNull(), "sublayers": NSNull(), "mask": NSNull()]
        }
        content.anchorPoint = .zero
        picture.addSublayer(content)
    }

    /// The style's `filter` (`{"p": [...], "pd": [...], "rc": [...]}`);
    /// whether the box is filtered (a chain this host runs, on a device with
    /// a GPU).
    func set(_ value: BatchValue?) -> Bool {
        guard case .object(let o)? = value, let p = o["p"]?.numbers, p.count > 5 else { return false }
        program = p.map(Float.init)
        darkProgram = o["pd"]?.numbers.map { $0.map(Float.init) }.flatMap { $0.count == program.count ? $0 : nil }
        shadow = FilterShadow(program: p).flatMap { light in
            guard let pd = o["pd"]?.numbers else { return (light, light) }
            return FilterShadow(program: pd).map { (light, $0) }
        }
        return shadow != nil || SvgFilterGPU.runs(program)
    }

    /// Draw the box's picture again, `scale` pixels per point. `clip`: the
    /// box's `clip-path` as a mask for the picture.
    func render(_ box: CALayer, clip: CALayer?, scale: CGFloat, dark: Bool) {
        let program = dark ? darkProgram ?? self.program : self.program
        guard let parent = box.superlayer, program.count > 5 else { return }
        CATransaction.begin()
        CATransaction.setDisableActions(true)
        defer { CATransaction.commit() }
        // A layer that clips its bounds would clip its shadow, which CSS
        // draws past an `overflow: hidden` box: that box is pictured.
        if let shadow, !box.masksToBounds {
            if picture.superlayer != nil { picture.removeFromSuperlayer(); content.contents = nil }
            // The box shows, and its `clip-path` clips the shadow with it,
            // as CSS clips a filter's result.
            if picture.mask != nil { picture.mask = nil }
            if box.mask !== clip { box.mask = clip }
            (dark ? shadow.dark : shadow.light).cast(on: box)
            caster = box
            return
        }
        uncast()
        if box.mask !== hide { box.mask = hide }
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
        uncast()
    }

    private func uncast() {
        caster.map(FilterShadow.uncast)
        caster = nil
    }
}

/// A lone `drop-shadow()` as Core Animation casts it: the alpha of the
/// layer and everything in it, blurred with `shadowRadius` as the
/// Gaussian's σ (the function's third length, as Filter Effects 1 defines
/// `drop-shadow()`).
struct FilterShadow {
    let color: CGColor
    let opacity: Float
    let radius: CGFloat
    let offset: CGSize

    init?(program: [Double]) {
        guard let drop = SvgScene.dropShadow(program: program) else { return nil }
        (color, opacity, radius, offset) = (drop.color, drop.opacity, drop.sigma, drop.offset)
    }

    func cast(on layer: CALayer) {
        if layer.shadowColor != color { layer.shadowColor = color }
        if layer.shadowOpacity != opacity { layer.shadowOpacity = opacity }
        if layer.shadowRadius != radius { layer.shadowRadius = radius }
        if layer.shadowOffset != offset { layer.shadowOffset = offset }
    }

    static func uncast(_ layer: CALayer) {
        layer.shadowOpacity = 0
    }
}

/// A presenter's filtered boxes. Descendants publish pixels independently
/// of runner batches, so those publications also invalidate their pictures.
final class BoxFilters {
    private struct Entry {
        weak var view: NodeView?
        var render: () -> Void
        var depth: Int {
            var n = 0, parent = view?.superview
            while let v = parent { n += 1; parent = v.superview }
            return n
        }
    }
    private var views: [ObjectIdentifier: Entry] = [:]
    private var pending: Set<ObjectIdentifier> = []
    private var queued = false

    func add(_ view: NodeView, render: @escaping () -> Void) { views[ObjectIdentifier(view)] = Entry(view: view, render: render) }
    func remove(_ view: NodeView) { let key = ObjectIdentifier(view); views.removeValue(forKey: key); pending.remove(key) }
    var isEmpty: Bool { views.isEmpty }

    func changed(_ view: NodeView) {
        guard !views.isEmpty else { return }
        var parent = view.superview
        if views[ObjectIdentifier(view)] != nil { pending.insert(ObjectIdentifier(view)) }
        while let v = parent {
            let key = ObjectIdentifier(v)
            if views[key] != nil { pending.insert(key) }
            parent = v.superview
        }
        guard !pending.isEmpty, !queued else { return }
        queued = true
        DispatchQueue.main.async { [weak self] in
            guard let self else { return }
            queued = false
            let entries = pending.compactMap { self.views[$0] }
            pending.removeAll(keepingCapacity: true)
            draw(entries)
        }
    }

    /// Inner pictures must land before their filtered ancestors copy them.
    private func draw(_ entries: [Entry]) {
        for entry in entries.sorted(by: { $0.depth > $1.depth }) where entry.view != nil { entry.render() }
    }

    func render() {
        pending.removeAll(keepingCapacity: true)
        draw(Array(views.values))
    }
}
