// @ref LLP 1055.000 D14 — CSS `filter` on a box, on Apple (Filter Effects 1
// §12): the web lowers it natively; here the box's subtree is drawn as it
// would show unfiltered, the function chain runs on the GPU (the SVG
// island's `SvgFilterGPU`, one implementation for boxes and SVG groups),
// and the result shows in a layer standing where the box stands. The box
// itself stays in the tree, masked to nothing, so it still takes hits, is
// still what the accessibility tree and the agent read, and still lays out.
// The picture is drawn again when a batch touches the box or anything in
// it, when its size, scale or appearance changes, and after an image or
// text raster under it publishes pixels; after any other batch it only
// follows the box. An animation under a filtered box is sampled instead of
// lowered (`svg_lower::eligibility`).
//
// The main thread draws the subtree; the chain runs on a queue of its own,
// whose Core Image render waits for the GPU, and the old picture shows
// until the new one lands. Only a box's first picture, while it is on
// screen, is waited for, so it never shows blank. In a virtualized list of
// filtered thumbnails, every filtered box drawn after every batch, each
// waiting for the GPU, was most of a scroll's main-thread time.
//
// A lone `drop-shadow()` is none of that: it is Core Animation's own
// shadow on the box's layer, as a hand-built view casts one under an icon
// over a photo, and the render server draws it.
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
    /// What the picture shows: the node, its size, scale and appearance.
    private struct Drawn: Equatable { var node: UInt32, size: CGSize, scale: CGFloat, dark: Bool }
    private var drawn: Drawn?
    /// A chain running off the main thread, and the newest drawing waiting
    /// for it to finish. Each drawing is numbered; one numbered `cleared`
    /// or below is not shown.
    private struct Job { let source: CGImage, program: [Float], rect: CGRect, scale: CGSize, generation: Int }
    private var running = false
    private var waiting: Job?
    private var generation = 0, cleared = 0
    var drawing: Bool { running }
    /// The box casts its filter as a layer shadow: nothing to draw.
    var casts: Bool { caster != nil }
    private static let queue = DispatchQueue(label: "exact.box-filter", qos: .userInteractive)

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

    /// Stand where the box stands and, when `fresh` (what the box shows
    /// changed) or its size, scale or appearance did, draw its picture
    /// again, `scale` pixels per point. `clip`: the box's `clip-path` as a
    /// mask for the picture. `node`: the box's node, a new one in a reused
    /// view. `shown`: whether the box is on screen, where its first picture
    /// is waited for. `landed`: a picture drawn off the main thread showed.
    func render(_ box: CALayer, clip: CALayer?, scale: CGFloat, dark: Bool, node: UInt32, fresh: Bool = true,
                shown: Bool = true, landed: @escaping () -> Void = {}) {
        let program = dark ? darkProgram ?? self.program : self.program
        guard let parent = box.superlayer, program.count > 5 else { return }
        CATransaction.begin()
        CATransaction.setDisableActions(true)
        defer { CATransaction.commit() }
        // A layer that clips its bounds would clip its shadow, which CSS
        // draws past an `overflow: hidden` box: that box is pictured.
        if let shadow, !box.masksToBounds {
            if picture.superlayer != nil { picture.removeFromSuperlayer(); clear() }
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
        if picture.superlayer !== parent || parent.sublayers?.firstIndex(of: picture) != (parent.sublayers?.firstIndex(of: box)).map({ $0 + 1 }) {
            picture.removeFromSuperlayer()
            parent.insertSublayer(picture, above: box)
        }
        if picture.bounds != b { picture.bounds = b }
        if picture.anchorPoint != box.anchorPoint { picture.anchorPoint = box.anchorPoint }
        if picture.position != box.position { picture.position = box.position }
        if !CATransform3DEqualToTransform(picture.transform, box.transform) { picture.transform = box.transform }
        if picture.opacity != box.opacity { picture.opacity = box.opacity }
        if picture.isHidden != box.isHidden { picture.isHidden = box.isHidden }
        if picture.mask !== clip { picture.mask = clip }
        let now = Drawn(node: node, size: b.size, scale: scale, dark: dark)
        guard fresh || drawn != now else { return }
        // Another node's picture (a reused view) is not this one's to show
        // while this one's is drawn.
        let first = drawn?.node != node
        if first { clear() }
        drawn = now
        let region = CGRect(x: b.minX + CGFloat(program[0]), y: b.minY + CGFloat(program[1]),
                            width: b.width + CGFloat(program[2]), height: b.height + CGFloat(program[3]))
        guard let (rect, w, h, _) = SvgIsland.extent(region, k: scale, seen: nil, limit: SvgIsland.budget / 24),
              let space = CGColorSpace(name: CGColorSpace.sRGB),
              let ctx = CGContext(data: nil, width: w, height: h, bitsPerComponent: 8, bytesPerRow: w * 4, space: space,
                                  bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue) else {
            clear()
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
        guard let source = ctx.makeImage() else { clear(); return }
        generation += 1
        let job = Job(source: source, program: program, rect: rect,
                      scale: CGSize(width: CGFloat(w) / rect.width, height: CGFloat(h) / rect.height), generation: generation)
        if first && shown {
            show(Self.run(job), in: rect)
        } else {
            run(job, landed: landed)
        }
    }

    private static func run(_ job: Job) -> CGImage? {
        SvgFilterGPU.run(job.program, source: job.source, origin: job.rect.origin, scale: job.scale)
    }

    /// The chain on the queue, one job at a time, the newest waiting.
    private func run(_ job: Job, landed: @escaping () -> Void) {
        guard !running else { waiting = job; return }
        running = true
        Self.queue.async { [weak self] in
            let out = Self.run(job)
            DispatchQueue.main.async { [weak self] in
                guard let self else { return }
                running = false
                if job.generation > cleared {
                    CATransaction.begin()
                    CATransaction.setDisableActions(true)
                    show(out, in: job.rect)
                    CATransaction.commit()
                    landed()
                }
                if let next = waiting {
                    waiting = nil
                    run(next, landed: landed)
                }
            }
        }
    }

    private func show(_ image: CGImage?, in rect: CGRect) {
        guard let image else { content.contents = nil; return }
        content.frame = rect
        content.contents = image
    }

    /// No picture, and none on its way.
    private func clear() {
        content.contents = nil
        cleared = generation
        waiting = nil
    }

    /// The box is no longer filtered.
    func remove() {
        picture.removeFromSuperlayer()
        clear()
        drawn = nil
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

/// A presenter's filtered boxes, and which of them to draw again: those a
/// batch touched (the box or anything in it), and those under which pixels
/// were published between batches.
final class BoxFilters {
    private struct Entry {
        weak var view: NodeView?
        /// Draw the picture again (`true`), or follow the box and draw only
        /// if its size, scale or appearance changed.
        var render: (_ fresh: Bool) -> Void
        var depth: Int {
            var n = 0, parent = view?.superview
            while let v = parent { n += 1; parent = v.superview }
            return n
        }
    }
    private var views: [ObjectIdentifier: Entry] = [:]
    private var pending: Set<ObjectIdentifier> = []
    private var queued = false
    /// Whether any box is pictured, as the last batch left them: a screen
    /// whose filters are all layer shadows walks up from no touched node.
    private var pictured = false

    func add(_ view: NodeView, render: @escaping (_ fresh: Bool) -> Void) {
        views[ObjectIdentifier(view)] = Entry(view: view, render: render)
        pictured = true
    }
    func remove(_ view: NodeView) { let key = ObjectIdentifier(view); views.removeValue(forKey: key); pending.remove(key) }
    var isEmpty: Bool { views.isEmpty }
    /// A picture is on its way or waits to be drawn.
    var drawing: Bool { queued || views.values.contains { $0.view?.boxFilter?.drawing == true } }

    /// A batch changed `view`: it and every filtered box above it draw
    /// again as the batch ends.
    func touched(_ view: PlatformView) {
        guard pictured else { return }
        var at: PlatformView? = view
        while let v = at {
            let key = ObjectIdentifier(v)
            if views[key] != nil { pending.insert(key) }
            at = v.superview
        }
    }

    /// `view` published pixels between batches: it and every filtered box
    /// above it draw again.
    func changed(_ view: PlatformView) {
        touched(view)
        schedule()
    }

    /// `view`'s picture landed: the filtered boxes above it, which drew the
    /// old one, draw again.
    func landed(_ view: NodeView) {
        guard let parent = view.superview else { return }
        changed(parent)
    }

    private func schedule() {
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
        for entry in entries.sorted(by: { $0.depth > $1.depth }) where entry.view != nil { entry.render(true) }
    }

    /// As a batch ends: what it touched draws again; every other box
    /// follows its own.
    func render() {
        let fresh = pending.compactMap { views[$0] }
        for (key, entry) in views where !pending.contains(key) { entry.render(false) }
        pending.removeAll(keepingCapacity: true)
        draw(fresh)
        pictured = views.values.contains { $0.view?.boxFilter?.casts == false }
    }
}
