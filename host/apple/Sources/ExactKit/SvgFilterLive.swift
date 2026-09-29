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
#if os(iOS)
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
    private var renderer: CARenderer?
    private var texture: MTLTexture?
    private var surfaces: [IOSurface] = []
    private var turn = 0
    private struct Input {
        let els: [Any], rect: CGRect, w: Int, h: Int, k: CGFloat, program: [Float], dark: Bool
        let fonts: SvgText.Fonts?
    }
    private var pending: Input?
    private var scheduled = false
    /// A draw off the main thread not yet shown (main thread only).
    private var busy = false
    /// Draws asked for and not yet shown, over every picture: the agent's
    /// settle and screenshot wait for them.
    nonisolated(unsafe) static var inFlight = 0
    private static let queue = DispatchQueue(label: "exact.svg.filter", qos: .userInteractive)

    init(layer: CALayer) {
        self.layer = layer
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
    func update(els: [Any], rect: CGRect, w: Int, h: Int, k: CGFloat, program: [Float], dark: Bool, fonts: SvgText.Fonts?) {
        pending = Input(els: els, rect: rect, w: w, h: h, k: k, program: program, dark: dark, fonts: fonts)
        layer.bounds = CGRect(origin: .zero, size: rect.size)
        layer.position = rect.origin
        guard !scheduled else { return }
        scheduled = true
        SvgFilterLive.inFlight += 1
        DispatchQueue.main.async { [weak self] in
            SvgFilterLive.inFlight -= 1
            self?.kick()
        }
    }

    /// Main thread: start the next draw when none is running.
    private func kick() {
        scheduled = false
        guard !busy, let p = pending else { return }
        pending = nil
        if SvgFilterLive.hasText(p.els) {
            if let surface = draw(p) { show(surface) }
            return
        }
        busy = true
        SvgFilterLive.inFlight += 1
        SvgFilterLive.queue.async { [weak self] in
            let surface = self?.draw(p)
            DispatchQueue.main.async { [weak self] in
                SvgFilterLive.inFlight -= 1
                guard let self else { return }
                self.busy = false
                if let surface { self.show(surface) }
                if self.pending != nil { self.kick() }
            }
        }
    }

    private func show(_ surface: IOSurface) {
        CATransaction.begin()
        CATransaction.setDisableActions(true)
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

    /// The picture of `p` in the next surface: on the queue, or on the main
    /// thread for a sub-scene with text.
    private func draw(_ p: Input) -> IOSurface? {
        guard let metal = SvgFilterGPU.metal else { return nil }
        let (w, h) = (p.w, p.h)
        // The renderer first: it draws the tree as committed after it has it.
        if texture?.width != w || texture?.height != h {
            let d = MTLTextureDescriptor.texture2DDescriptor(pixelFormat: .rgba8Unorm, width: w, height: h, mipmapped: false)
            d.usage = [.renderTarget, .shaderRead]
            d.storageMode = .private
            guard let t = metal.device.makeTexture(descriptor: d) else { return nil }
            let r = CARenderer(mtlTexture: t, options: [Shadow.queueOption: metal.queue])
            r.layer = root
            texture = t
            renderer = r
            surfaces = []
        }
        CATransaction.begin()
        CATransaction.setDisableActions(true)
        scene.scale = p.k
        scene.fonts = p.fonts
        let m = CGAffineTransform(translationX: -p.rect.minX, y: -p.rect.minY)
        scene.apply(["box": [0, 0, p.rect.width, p.rect.height], "t": [m.a, m.b, m.c, m.d, m.tx, m.ty], "els": p.els], dark: p.dark, clock: nil)
        root.bounds = CGRect(x: 0, y: 0, width: w, height: h)
        top.bounds = CGRect(origin: .zero, size: p.rect.size)
        top.position = CGPoint(x: CGFloat(w) / 2, y: CGFloat(h) / 2)
        top.transform = CATransform3DMakeScale(CGFloat(w) / p.rect.width, -CGFloat(h) / p.rect.height, 1)
        CATransaction.commit()
        CATransaction.flush()
        guard let texture, let renderer else { return nil }
        if let cb = metal.queue.makeCommandBuffer() {
            let pass = MTLRenderPassDescriptor()
            pass.colorAttachments[0].texture = texture
            pass.colorAttachments[0].loadAction = .clear
            pass.colorAttachments[0].storeAction = .store
            pass.colorAttachments[0].clearColor = MTLClearColor(red: 0, green: 0, blue: 0, alpha: 0)
            cb.makeRenderCommandEncoder(descriptor: pass)?.endEncoding()
            cb.commit()
        }
        renderer.bounds = CGRect(x: 0, y: 0, width: w, height: h)
        renderer.beginFrame(atTime: CACurrentMediaTime(), timeStamp: nil)
        renderer.addUpdate(renderer.bounds)
        renderer.render()
        renderer.endFrame()
        // The texture's first row is the top; CI's y runs up.
        guard let space = CGColorSpace(name: CGColorSpace.sRGB),
              let source = CIImage(mtlTexture: texture, options: [.colorSpace: space])?
                .transformed(by: CGAffineTransform(scaleX: 1, y: -1).translatedBy(x: 0, y: -CGFloat(h))),
              let (image, linear) = SvgFilterGPU.chain(p.program, source: source, w: w, h: h, origin: p.rect.origin,
                                                       scale: CGSize(width: CGFloat(w) / p.rect.width, height: CGFloat(h) / p.rect.height)).result
        else { return nil }
        // Three surfaces: the one shown, the one handed over, the one drawn.
        if surfaces.count < 3 {
            guard let s = IOSurface(properties: [.width: w, .height: h, .bytesPerElement: 4, .pixelFormat: 0x4247_5241 /* 'BGRA' */]) else { return nil }
            surfaces.append(s)
        }
        let surface = surfaces[turn % surfaces.count]
        turn += 1
        return SvgFilterGPU.render(image, linear: linear, into: surface) ? surface : nil
    }
}
#endif
