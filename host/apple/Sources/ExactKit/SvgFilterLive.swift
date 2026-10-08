// @ref LLP 1055.000 D14 — a filtered element whose input changes frame to
// frame (an animation inside it, sampled because a layer's own animation
// would not show in pixels), kept on the GPU on iOS. Its first picture is
// drawn as any island's; after that its sub-scene stays, is updated in
// place, and is rendered by Core Animation on the GPU (`CARenderer`, as the
// capture's shadow tree is) into a texture the filter chain reads, into an
// IOSurface the layer shows: no CPU raster, no upload, no readback. The
// feature bench's F3 (a CSS chain over a 400 × 300 scene with an orbit
// turning inside it) spent 480 ms/s of an iPhone's main thread re-making the
// island on the CPU. The latest input wins: an update is drawn once after
// the commit that asked for it, however many came in between; it is drawn on
// a serial queue off the main thread (a sub-scene with text stays on the
// main thread, where the session's fonts are), and the main thread only
// swaps the layer's surface.
import CoreImage
import IOSurface
import Metal
import QuartzCore

final class SvgFilterLive {
    /// The picture's layer in the scene; its contents are the last drawn.
    let layer: CALayer
    private let scene = SvgScene()
    /// The renderer's root, in pixels, flipped as the capture's is; `top`
    /// scales the sub-scene (points) to it and mirrors it back upright.
    private let root = CALayer()
    private let top = CALayer()
    /// The renderer's target. One: a second renderer of the same tree draws
    /// nothing. The next draw's render is ordered after this draw's chain
    /// by the one command queue both are encoded on.
    private var targets: [(texture: MTLTexture, renderer: CARenderer)] = []
    private var surfaces: [IOSurface] = []
    /// The target is transparent (queue only).
    private var cleared = false

    private static func clear(_ texture: MTLTexture, on cb: MTLCommandBuffer) {
        let pass = MTLRenderPassDescriptor()
        pass.colorAttachments[0].texture = texture
        pass.colorAttachments[0].loadAction = .clear
        pass.colorAttachments[0].storeAction = .store
        pass.colorAttachments[0].clearColor = MTLClearColor(red: 0, green: 0, blue: 0, alpha: 0)
        cb.makeRenderCommandEncoder(descriptor: pass)?.endEncoding()
    }
    private var turn = 0
    private struct Input {
        let els: [Any], rect: CGRect, w: Int, h: Int, k: CGFloat, program: [Float], dark: Bool
        let fonts: SvgText.Fonts?
        let clock: Double?
        /// Whether any animation in it is held at a local time.
        let held: Bool
        /// Whether it draws text (then drawn on the main thread).
        let text: Bool
        /// The sub-scene already holds this input: only render again (a
        /// frame of the animations inside it).
        var again = false
    }
    /// The last input drawn, for the frames of its animations.
    private var last: Input?
    private var pending: Input?
    private var scheduled = false
    /// A draw off the main thread not yet shown (main thread only).
    private var busy = false
    /// Draws asked for and not yet shown, over every picture: the agent's
    /// settle and screenshot wait for them.
    nonisolated(unsafe) static var inFlight = 0
    private static let queue = DispatchQueue(label: "exact.svg.filter", qos: .userInteractive)
    /// `kCARendererMetalCommandQueue`, read at run time (as `Shadow` does):
    /// the renderer then encodes on the chain's queue, so they are ordered.
    private static let queueOption: String = {
        if let sym = dlsym(UnsafeMutableRawPointer(bitPattern: -2), "kCARendererMetalCommandQueue") {
            return sym.assumingMemoryBound(to: Unmanaged<NSString>.self).pointee.takeUnretainedValue() as String
        }
        return "kCARendererMetalCommandQueue"
    }()

    init(layer: CALayer) {
        self.layer = layer
        scene.offscreen = CACurrentMediaTime()
        root.isGeometryFlipped = true
        root.anchorPoint = .zero
        root.addSublayer(top)
        top.addSublayer(scene.root)
    }

    /// Whether this picture can follow `spec` on the GPU: a chain the GPU
    /// runs, on a device with Metal.
    static func takes(_ spec: [String: Any]) -> Bool {
        SvgFilterGPU.metal != nil && SvgFilterGPU.runs(((spec["p"] as? [Any]) ?? []).map { Float(($0 as? NSNumber)?.doubleValue ?? 0) })
    }

    /// A new input: the layer takes its new place now, its pixels after this
    /// commit (the previous picture shows until then).
    func update(els: [Any], rect: CGRect, w: Int, h: Int, k: CGFloat, program: [Float], dark: Bool, fonts: SvgText.Fonts?,
                clock: Double?, now: Bool = false) {
        let input = Input(els: els, rect: rect, w: w, h: h, k: k, program: program, dark: dark, fonts: fonts, clock: clock,
                          held: SvgFilterLive.held(els, clock: clock), text: SvgFilterLive.hasText(els))
        last = input
        animate(SvgFilterLive.running(els, clock: clock))
        if now, pending == nil, !busy, gpu == 0 {
            // The first picture, in this commit: never shown without it.
            if let (surface, shown) = draw(input) { show(surface, shown) }
            return
        }
        pending = input
        schedule()
    }

    private func schedule() {
        guard !scheduled else { return }
        scheduled = true
        SvgFilterLive.inFlight += 1
        DispatchQueue.main.async { [weak self] in
            SvgFilterLive.inFlight -= 1
            self?.kick()
        }
    }

    /// Whether the elements hold an animation Core Animation is playing
    /// (one not held at a local time by an authored pause or the agent's
    /// clock).
    private static func running(_ v: Any, clock: Double?) -> Bool {
        if clock != nil { return false }
        if let d = v as? [String: Any] {
            if let specs = d["a"] as? [[String: Any]], specs.contains(where: { $0["h"] == nil || $0["h"] is NSNull }) { return true }
            return d.values.contains { running($0, clock: nil) }
        }
        if let a = v as? [Any] { return a.contains { running($0, clock: nil) } }
        return false
    }

    /// Whether the elements hold an animation held at a local time.
    private static func held(_ v: Any, clock: Double?) -> Bool {
        if let d = v as? [String: Any] {
            if let specs = d["a"] as? [[String: Any]], !specs.isEmpty,
               clock != nil || specs.contains(where: { !($0["h"] == nil || $0["h"] is NSNull) }) { return true }
            return d.values.contains { held($0, clock: clock) }
        }
        if let a = v as? [Any] { return a.contains { held($0, clock: clock) } }
        return false
    }

    /// Whether `els` hold any animation at all (the picture then follows
    /// them from its first frame).
    static func animated(_ v: Any) -> Bool {
        if let d = v as? [String: Any] {
            if let specs = d["a"] as? [Any], !specs.isEmpty { return true }
            return d.values.contains(where: animated)
        }
        if let a = v as? [Any] { return a.contains(where: animated) }
        return false
    }

    /// A draw each frame of the app's clock while the sub-scene holds a
    /// running animation (iOS: the picture follows its content there; macOS
    /// draws islands).
    private func animate(_ on: Bool) {
        #if os(iOS) || os(tvOS)
        // A nested filter is updated inside its outer one's encode, on the
        // queue; the app's clock is the main thread's.
        guard Thread.isMainThread else { DispatchQueue.main.async { [weak self] in self?.animate(on) }; return }
        if on, !FrameClock.shared.wants(self) {
            FrameClock.shared.want(self, .svgFilter, rate: FrameClock.full(on: nil)) { [weak self] _ in self?.tick() }
        } else if !on {
            FrameClock.shared.drop(self)
        }
        #endif
    }

    /// A frame of the running animations: the same input rendered again.
    fileprivate func tick() {
        guard pending == nil, var input = last else { return }
        input.again = true
        pending = input
        schedule()
    }

    /// The agent's clock moved: the sub-scene's animations seek to it.
    func seek(_ clock: Double?) {
        guard let l = last, l.clock != clock else { return }
        update(els: l.els, rect: l.rect, w: l.w, h: l.h, k: l.k, program: l.program, dark: l.dark, fonts: l.fonts, clock: clock)
    }

    /// Stop drawing (the element is gone).
    func stop() { animate(false); last = nil; pending = nil }

    /// Main thread: start the next draw when none is running.
    private func kick() {
        scheduled = false
        guard !busy, gpu < 2, let p = pending else { return }
        pending = nil
        if p.text {
            if let (surface, shown) = draw(p) { show(surface, shown) }
            return
        }
        // The queue encodes this draw and is free for the next one while
        // the GPU runs it (two in flight at most); its surface shows when
        // the GPU is done, in order.
        busy = true
        SvgFilterLive.inFlight += 1
        SvgFilterLive.queue.async { [weak self] in
            let started = self?.encode(p)
            DispatchQueue.main.async { [weak self] in
                guard let self else { return }
                self.busy = false
                self.gpu += 1
                if self.pending != nil, self.gpu < 2 { self.kick() }
            }
            SvgFilterLive.done.async { [weak self] in
                started?.wait()
                DispatchQueue.main.async { [weak self] in
                    SvgFilterLive.inFlight -= 1
                    guard let self else { return }
                    self.gpu -= 1
                    if let started { self.show(started.surface, started.shown) }
                    if self.pending != nil, !self.busy { self.kick() }
                }
            }
        }
    }

    /// Draws encoded and not yet done on the GPU (main thread).
    private var gpu = 0
    /// Where encoded draws are waited for, in order.
    private static let done = DispatchQueue(label: "exact.svg.filter.done", qos: .userInteractive)

    /// Where a drawn surface shows: the region, and the part of the surface
    /// the picture takes.
    struct Shown { let rect: CGRect; let unit: CGRect }

    private func show(_ surface: IOSurface, _ s: Shown) {
        CATransaction.begin()
        CATransaction.setDisableActions(true)
        layer.bounds = CGRect(origin: .zero, size: s.rect.size)
        layer.position = s.rect.origin
        layer.contentsRect = s.unit
        layer.contents = surface
        CATransaction.commit()
    }

    /// Whether a sub-scene draws text (the session's fonts are the main
    /// thread's).
    private static func hasText(_ v: Any) -> Bool {
        if let d = v as? [String: Any] { return d["tx"] != nil || d.values.contains(where: hasText) }
        if let a = v as? [Any] { return a.contains(where: hasText) }
        return false
    }

    /// Main thread: wait (at most `timeout`) for the draws asked for to show.
    static func waitForDraws(timeout: TimeInterval = 1) {
        let end = Date(timeIntervalSinceNow: timeout)
        while inFlight > 0 && Date() < end { RunLoop.main.run(until: Date(timeIntervalSinceNow: 0.002)) }
    }

    /// The picture of `p` in the next surface, drawn and waited for (the
    /// main thread's path, for a sub-scene with text).
    private func draw(_ p: Input) -> (IOSurface, Shown)? {
        guard let started = encode(p) else { return nil }
        started.wait()
        return (started.surface, started.shown)
    }

    /// The picture of `p` encoded into the next surface, and how to wait
    /// for the GPU before it shows.
    private func encode(_ p: Input) -> (surface: IOSurface, wait: () -> Void, shown: Shown)? {
        guard let metal = SvgFilterGPU.metal else { return nil }
        let (w, h) = (p.w, p.h)
        // The target and the surfaces are kept at a size that holds the
        // picture, growing only (by 64 px steps), so a region that changes
        // as its content moves (F3's orbit) allocates nothing per frame; the
        // picture takes their top-left `w` × `h` (`contentsRect`). A renderer
        // draws the tree as committed after it has it: made first.
        let cap = targets.first.map { (w: $0.texture.width, h: $0.texture.height) }
        if cap.map({ w > $0.w || h > $0.h || w * h * 4 < $0.w * $0.h }) ?? true {
            let (W, H) = (max(cap?.w ?? 0, (w + 63) / 64 * 64), max(cap?.h ?? 0, (h + 63) / 64 * 64))
            let (cw, ch) = cap.map { w * h * 4 < $0.w * $0.h ? ((w + 63) / 64 * 64, (h + 63) / 64 * 64) : (W, H) } ?? (W, H)
            targets = []
            surfaces = []
            let d = MTLTextureDescriptor.texture2DDescriptor(pixelFormat: .rgba8Unorm, width: cw, height: ch, mipmapped: false)
            d.usage = [.renderTarget, .shaderRead]
            d.storageMode = .private
            guard let t = metal.device.makeTexture(descriptor: d) else { return nil }
            let r = CARenderer(mtlTexture: t, options: [SvgFilterLive.queueOption: metal.queue])
            r.layer = root
            targets.append((t, r))
            cleared = false
        }
        let (tw, th) = (targets[0].texture.width, targets[0].texture.height)
        // One instant for the held animations' placement and the render.
        let at = CACurrentMediaTime()
        scene.offscreen = at
        CATransaction.begin()
        CATransaction.setDisableActions(true)
        scene.scale = p.k
        scene.fonts = p.fonts
        let m = CGAffineTransform(translationX: -p.rect.minX, y: -p.rect.minY)
        if !p.again {
            scene.apply(["box": [0, 0, p.rect.width, p.rect.height], "t": [m.a, m.b, m.c, m.d, m.tx, m.ty], "els": p.els], dark: p.dark, clock: p.clock)
        }
        // A held animation runs here from where it is held
        // (`CssAnimations.make`'s `offscreen`): set again for each render.
        if p.held { scene.seek(clock: p.clock, force: true) }
        root.bounds = CGRect(x: 0, y: 0, width: tw, height: th)
        top.bounds = CGRect(origin: .zero, size: p.rect.size)
        // The renderer's texture row is the root's height less its y: the
        // picture's centre goes where its rows are the texture's first `h`.
        top.position = CGPoint(x: CGFloat(w) / 2, y: CGFloat(th) - CGFloat(h) / 2)
        top.transform = CATransform3DMakeScale(CGFloat(w) / p.rect.width, -CGFloat(h) / p.rect.height, 1)
        CATransaction.commit()
        CATransaction.flush()
        let (texture, renderer) = targets[turn % targets.count]
        // The renderer composites over what the texture holds: it starts
        // clear (the last draw's chain cleared it after reading it, saving a
        // submission a frame), or is cleared here.
        if !cleared, let cb = metal.queue.makeCommandBuffer() {
            SvgFilterLive.clear(texture, on: cb)
            cb.commit()
        }
        cleared = false
        renderer.bounds = CGRect(x: 0, y: 0, width: tw, height: th)
        renderer.beginFrame(atTime: at, timeStamp: nil)
        renderer.addUpdate(renderer.bounds)
        renderer.render()
        renderer.endFrame()
        // Four surfaces: the one shown, one handed over, two drawn.
        if surfaces.count < 4 {
            guard let s = IOSurface(properties: [.width: tw, .height: th, .bytesPerElement: 4, .pixelFormat: 0x4247_5241 /* 'BGRA' */]) else { return nil }
            if let profile = CGColorSpace(name: CGColorSpace.sRGB)?.copyPropertyList() { IOSurfaceSetValue(s, kIOSurfaceColorSpace, profile) }
            surfaces.append(s)
        }
        let surface = surfaces[turn % surfaces.count]
        turn += 1
        let shown = Shown(rect: p.rect, unit: CGRect(x: 0, y: 0, width: CGFloat(w) / CGFloat(tw), height: CGFloat(h) / CGFloat(th)))
        // Metal when the chain is one it runs; Core Image otherwise.
        if let fm = SvgFilterMetal.shared, let steps = SvgFilterMetal.steps(p.program, scale: CGFloat(w) / p.rect.width),
           let cb = metal.queue.makeCommandBuffer(), fm.encode(steps, source: texture, into: surface, on: cb) {
            SvgFilterLive.clear(texture, on: cb)
            cleared = true
            cb.commit()
            return (surface, { cb.waitUntilCompleted() }, shown)
        }
        // The texture's first row is the top; CI's y runs up.
        guard let space = CGColorSpace(name: CGColorSpace.sRGB),
              let source = CIImage(mtlTexture: texture, options: [.colorSpace: space])?
                .transformed(by: CGAffineTransform(scaleX: 1, y: -1).translatedBy(x: 0, y: -CGFloat(h))),
              let (image, linear) = SvgFilterGPU.chain(p.program, source: source, w: w, h: h, origin: p.rect.origin,
                                                       scale: CGSize(width: CGFloat(w) / p.rect.width, height: CGFloat(h) / p.rect.height)).result
        else { return nil }
        guard let task = SvgFilterGPU.start(image, linear: linear, size: CGSize(width: w, height: h), into: surface) else { return nil }
        return (surface, { _ = try? task.waitUntilCompleted() }, shown)
    }
}
