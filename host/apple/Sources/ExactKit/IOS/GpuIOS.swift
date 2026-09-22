// The GPU module on iOS (LLP 1009 D2/D4): the app's `<app>-gpu` dylib —
// built for the simulator, carried in the bundle's Frameworks — loaded
// with dlopen the first time a canvas is on screen, after the first painted
// frame, and driven from the presenter: a surface per canvas node on its
// CAMetalLayer, inputs bound as they arrive, frames rendered while a
// surface is dirty or wants more, from the same display link motion uses.
// The AppKit presenter's `Canvases` (host/apple/macos/…/Gpu.swift) on UIKit.
#if os(iOS)
import QuartzCore
import UIKit

/// A canvas node's backing view: a CAMetalLayer the module renders into.
final class MetalView: UIView {
    override class var layerClass: AnyClass { CAMetalLayer.self }
    override init(frame: CGRect) {
        super.init(frame: frame)
        isUserInteractionEnabled = false
        isOpaque = false
        // Composited, never direct-to-display: a translucent layer stays in
        // the compositor, where a drawable comes back as soon as it is read
        // (LLP 1014, the app inside the sky).
        (layer as? CAMetalLayer)?.isOpaque = false
        autoresizingMask = [.flexibleWidth, .flexibleHeight]
    }
    required init?(coder: NSCoder) { nil }
    override func didMoveToWindow() {
        super.didMoveToWindow()
        layer.contentsScale = window?.screen.scale ?? traitCollection.displayScale
    }
}

/// Every canvas on one session's page and its surface in the module — the
/// module itself loaded once per process (LLP 1031 D12).
final class Canvases {
    lazy var lifecycle = CanvasLifecycle(self)
    weak var session: ExactSession?
    final class Entry {
        let view: NodeView
        let name: String
        var values: Any
        var id: UInt32 = 0
        var presentable = true
        var wants = false
        var wantsInput = false
        var logCursor = 0
        var restoreAttempted = false
        var controls: [Int: SurfaceControl] = [:]
        var recoveryRedelivery = false
        var restorePending = false
        var restoreError: String?
        /// The surface samples the children (LLP 1014 D2): the overlay is
        /// captured into its texture and composited at alpha 0.
        var through = false
        /// The surface wants each direct child as its own texture and places
        /// it (LLP 1014 D5): captured one by one, hit-tested through the
        /// surface's placements.
        var each = false
        /// A children texture has been uploaded (so an emptied overlay is
        /// captured once more, to clear it).
        var uploaded = false
        var capturing = false
        /// The clock at the last readback of a nested canvas: one that wants
        /// frames is read back again only once the clock has moved, so a
        /// clock that stands still — the agent's — never spins it.
        var readAt: Double = -1
        /// The last readback's picture, kept while the module reports
        /// nothing dirty and the surface wants no frame at a new clock —
        /// so the line map, once drawn, costs the sky's capture nothing.
        var picture: CGImage?
        init(view: NodeView, name: String, values: Any) { self.view = view; self.name = name; self.values = values }
    }
    var entries: [UInt32: Entry] = [:]
    var publishers: [String: Entry] = [:]
    var module: GpuModule?
    var displayPeriod = DisplayPeriod()
    func period(_ ms: Double) {
        guard let m = module else { return }
        displayPeriod.publish(ms, maximum: Double(session?.presenter.viewport.window?.screen.maximumFramesPerSecond ?? 120)) { m.period?($0) }
    }
    var worldInput = WorldCarrier.read(ExactEnv.agentMode ? ProcessInfo.processInfo.environment["EXACT_WORLD"] : nil)
    var terminalRestoreReported = false
    var restoreJournal: [[String: Any]] = []
    var failed: String?
    var loadRequested = false
    var loadedMs: Double?
    var rendered = 0
    /// Captures since launch (LLP 1014 D3).
    var captures = 0
    /// The measure's window (EXACT_FPS): renders and captures since the last
    /// report, and the time they took (capture: paint and upload).
    var windowRenders = 0
    var windowRenderSeconds = 0.0
    var windowCaptures = 0
    var windowCaptureSeconds = 0.0
    private var captureScheduled = false
    var frameNow: Double?
    var settling = false

    /// Where the module lives: EXACT_GPU_DYLIB, or the bundle's Frameworks.
    static func modulePath() -> String {
        return GpuModule.modulePath(defaultPath:(Bundle.main.privateFrameworksPath ?? Bundle.main.bundlePath) + "/libexact_gpu.dylib", compat:GpuModule.bakedCompatibility, environment:ProcessInfo.processInfo.environment)
    }

    /// A shader's text changed (the asset row, LLP 1030 D10): the module
    /// takes it if its interface is the one it binds — every surface then
    /// renders again through the new pipeline — or refuses it by name and
    /// keeps the old one; a module not loaded yet reads the file when it is.
    func shaderChanged(_ name: String, text: Data) {
        guard let m = module else { return }
        if m.register(shader: name, text: text) {
            print("exact gpu: shader \(name) swapped in")
        } else {
            FileHandle.standardError.write(Data("exact gpu: shader \(name) refused — \(m.error()) — rebuild the native host\n".utf8))
        }
    }

    /// A surface op: new inputs for a canvas node.
    func surface(view: NodeView, name: String, values: Any) {
        if let e = entries[view.id], e.view !== view || e.name != name { destroy(view: view.id) }
        if let e = entries[view.id] {
            e.values = values
            if e.id != 0, let m = module { bindNow(m, e) }
        } else {
            let e = Entry(view: view, name: name, values: values)
            entries[view.id] = e
            claimPublisher(e)
            if let m = module { create(m, e) }
        }
    }

    func destroy(view: UInt32) {
        if let e = entries.removeValue(forKey: view) {
            e.view.canvasInput = nil
            if e.id != 0 { module?.destroy(e.id) }
            releasePublisher(e)
        }
    }

    /// A restart: every surface goes with its view (a reload reuses ids).
    func reset() { for view in Array(entries.keys) { destroy(view: view) } }

    /// Load the module — once, after the first painted frame, only when a
    /// canvas exists.
    /// The smoke's line: loaded (with the load time and counts) or why not.
    var status: String {
        if let failed { return "failed: \(failed)" }
        if module != nil { return "module loaded in \(String(format: "%.1f", loadedMs ?? 0)) ms; \(entries.count) canvases; \(rendered) renders" }
        return "not loaded: \(failed ?? (entries.isEmpty ? "no canvas" : "not requested"))"
    }

    func loadIfNeeded() {
        guard !loadRequested, !entries.isEmpty, session?.firstDrawMs != nil else { return }
        loadRequested = true
        let t = CACurrentMediaTime()
        switch GpuModule.loadShared(path: Canvases.modulePath()) {
        case .failure(let e):
            failed = e.message
            FileHandle.standardError.write(Data("exact gpu: \(e.message)\n".utf8))
        case .success(let m):
            module = m
            m.canvases.add(self)
            loadedMs = (CACurrentMediaTime() - t) * 1000
            // The shaders as files (LLP 1030 D8), from the app's asset root
            // (LLP 1031 D1): the app's directory in dev, the bundle in a
            // release.
            if let resolver = session?.app.resolver { m.registerShaders(resolver: resolver) }
            for e in Array(entries.values) where entries[e.view.id] === e { create(m, e) }
        }
    }

    private func scale(of metal: MetalView) -> CGFloat {
        metal.window?.screen.scale ?? metal.traitCollection.displayScale
    }

    /// The scale the children are captured at: the screen's (LLP 1014 D3).
    /// Tried and measured 2026-08-30 (LLP 1008 §9): capturing at 2 on a 3×
    /// phone is slower, not faster — `layer.render(in:)` then resamples
    /// every view's 3× backing store instead of blitting it, 28–39 ms a
    /// capture against 20–25 — so the scale stays the screen's.
    private func captureScale(of metal: MetalView) -> CGFloat {
        scale(of: metal)
    }

    private func create(_ m: GpuModule, _ e: Entry) {
        guard let metal = e.view.metal, let layer = metal.layer as? CAMetalLayer else { return }
        layer.contentsScale = scale(of: metal)
        let scale = Float(layer.contentsScale)
        let w = UInt32(max(1, (Float(metal.bounds.width) * scale).rounded()))
        let h = UInt32(max(1, (Float(metal.bounds.height) * scale).rounded()))
        let ptr = Unmanaged.passUnretained(layer).toOpaque()
        let bytes = Array(e.name.utf8)
        e.id = bytes.withUnsafeBufferPointer { m.create($0.baseAddress, bytes.count, ptr, w, h) }
        e.presentable = e.id != 0
        if e.id == 0 { FileHandle.standardError.write(Data("exact gpu: \(m.error())\n".utf8)); return }
        lifecycle.deliver(e.id)
        bindNow(m, e)
        e.each = m.wantsChildrenEach(e.id) != 0
        e.through = e.each || m.wantsChildren(e.id) != 0
        e.wantsInput = m.wantsInput?(e.id) == 1 && m.input != nil
        if e.wantsInput { e.view.canvasInput = CanvasInput(view: e.view) }
        if e.through { capture(m, e) }
    }

    /// Each direct child of the overlay captured on its own and uploaded with
    /// its frame (LLP 1014 D5). Placed children composite through the surface;
    /// unplaced HUD children keep their ordinary overlay. Hit-testing follows
    /// the placements the surface reports after each frame.
    private func captureEach(_ m: GpuModule, _ e: Entry, overlay: UIView, scale: CGFloat) -> Bool {
        let children = overlay.subviews.compactMap { $0 as? NodeView }
        var uploaded = 0
        let t0 = CACurrentMediaTime()
        defer { windowCaptures += 1; windowCaptureSeconds += CACurrentMediaTime() - t0 }
        for (i, child) in children.enumerated() {
            if child.frame.width <= 0 || child.frame.height <= 0 || (child.style["display"] as? String) == "none" {
                let r = m.child(e.id, UInt32(i), child.props["testId"] ?? "", 0, 0, 0, 0, 0, 0, nil, 0)
                if r != 0 { return false }
                continue
            }
            let hidden = child.isHidden
            if child.placementHidden { child.isHidden = false }
            defer { child.isHidden = hidden }
            guard let bitmap = Capture.bitmap(of: child, scale: scale), let data = bitmap.bytes else { continue }
            guard live(e.view.id) === e else { return false }
            let f = child.frame
            let r = m.child(e.id, UInt32(i), child.props["testId"] ?? "", Float(f.origin.x), Float(f.origin.y), Float(f.width), Float(f.height), UInt32(bitmap.width), UInt32(bitmap.height), UnsafePointer(data.assumingMemoryBound(to: UInt8.self)), bitmap.height * bitmap.bytesPerRow)
            if r != 0 { FileHandle.standardError.write(Data("exact gpu: \(m.error())\n".utf8)); return false }
            uploaded += 1
        }
        _ = m.childrenCount(e.id, UInt32(children.count))
        if ExactEnv.agentMode {
            FileHandle.standardError.write(Data(String(format: "canvas %d: captured %d children in %.2f ms\n", Int(e.view.id), uploaded, (CACurrentMediaTime() - t0) * 1000).utf8))
        }
        return true
    }

    /// After a frame, where the surface put each child (LLP 1014 D5): the
    /// homography on the child, `nil` for the kernel's frame.
    private func readPlacements(_ m: GpuModule, _ e: Entry) {
        guard e.each, let overlay = e.view.overlay else { return }
        var h = [Float](repeating: 0, count: 10)
        for (i, child) in overlay.subviews.compactMap({ $0 as? NodeView }).enumerated() {
            let outcome = h.withUnsafeMutableBufferPointer { m.placement(e.id, UInt32(i), $0.baseAddress, $0.count) }
            let next: [Double]? = outcome == 1 ? h.map { Double($0) } : nil
            let changed = next != child.placement || child.placementHidden != (outcome == 2)
            child.placement = next
            child.placementHidden = outcome == 2
            child.alpha = outcome == 0 ? 1 : 0
            if changed { child.placementChanged() }
        }
        // Alpha suppresses duplicate composition only. VoiceOver traverses the
        // explicitly supplied children; hidden placements are excluded.
        overlay.accessibilityElements = overlay.subviews.compactMap { $0 as? NodeView }.filter { !$0.placementHidden && !$0.isHidden }
    }

    /// Every canvas whose children changed since its last capture is painted
    /// again (LLP 1014 D3): the overlay's subtree into a bitmap at the
    /// screen's scale, uploaded as the surface's children texture. Called at
    /// the end of every batch, and once per run-loop turn for changes that
    /// arrive outside one (D4 b, c).
    func captureIfNeeded() {
        guard let m = module else { return }
        for e in Array(entries.values) where live(e.view.id) === e && e.presentable && e.through && e.view.needsCapture { capture(m, e) }
    }

    /// A capture on this run-loop turn, coalesced.
    func scheduleCapture() {
        guard !captureScheduled else { return }
        captureScheduled = true
        DispatchQueue.main.async { self.captureScheduled = false; self.captureIfNeeded() }
    }

    /// Whether the input being edited is under the overlay (D4 d): carets
    /// and selection repaint without any batch.
    func editing(under overlay: UIView) -> Bool {
        session?.presenter.editing?.isDescendant(of: overlay) ?? false
    }

    private func capture(_ m: GpuModule, _ e: Entry) {
        // A nested readback can post a message and apply another batch here.
        guard live(e.view.id) === e, e.presentable, !e.capturing else { return }
        e.capturing = true
        defer { e.capturing = false }
        guard let overlay = e.view.overlay, e.view.window != nil, let metal = e.view.metal else { return }
        e.view.needsCapture = false
        // Nothing to paint and nothing painted before: no texture at all.
        guard !overlay.subviews.isEmpty || e.uploaded else { return }
        let scale = captureScale(of: metal)
        if e.each {
            if captureEach(m, e, overlay: overlay, scale: scale) { e.uploaded = true; captures += 1; overlay.alpha = 1; e.view.paintedThisTurn = true; DispatchQueue.main.async { e.view.paintedThisTurn = false } }
            return
        }
        let t0 = CACurrentMediaTime()
        let w: UInt32, h: UInt32, len: Int, r: UInt32, t1: Double
        // The texture about to be drawn into may be the one the module is
        // reading (its last frame; its copy into the previous children):
        // its work first.
        if m.lossGeneration == 0, !Capture.cpu, m.textureMetal != nil, Shadow.shared != nil, let sync = m.sync { _ = sync() }
        if m.lossGeneration == 0, !Capture.cpu, let hand = m.textureMetal, let shadow = Shadow.shared, let texture = shadow.renderTexture(overlay, scale: scale) {
            // Zero-copy (LLP 1008 §9): the module samples the texture the
            // renderer drew, as it is.
            guard live(e.view.id) === e else { return }
            t1 = CACurrentMediaTime()
            w = UInt32(texture.width); h = UInt32(texture.height); len = 0
            r = hand(e.id, w, h, Unmanaged.passUnretained(texture).toOpaque())
        } else {
            guard let bitmap = Capture.bitmap(of: overlay, scale: scale), let data = bitmap.bytes else { return }
            guard live(e.view.id) === e else { return }
            t1 = CACurrentMediaTime()
            w = UInt32(bitmap.width); h = UInt32(bitmap.height)
            len = bitmap.height * bitmap.bytesPerRow
            r = m.texture(e.id, w, h, UnsafePointer(data.assumingMemoryBound(to: UInt8.self)), len)
        }
        let t2 = CACurrentMediaTime()
        windowCaptures += 1
        windowCaptureSeconds += t2 - t0
        if r != 0 { FileHandle.standardError.write(Data("exact gpu: \(m.error())\n".utf8)); return }
        e.uploaded = true
        captures += 1
        // Painted through the surface from here on: the overlay stays laid
        // out — hit-testable, in the accessibility hierarchy — and is no
        // longer composited itself (D5: pixels may distort, boxes may not).
        overlay.alpha = 0
        e.view.paintedThisTurn = true
        DispatchQueue.main.async { e.view.paintedThisTurn = false }
        if ExactEnv.agentMode {
            FileHandle.standardError.write(Data(len == 0 ? String(format: "canvas %d: captured %dx%d in %.2f ms, handed over in %.2f ms\n", Int(e.view.id), Int(w), Int(h), (t1 - t0) * 1000, (t2 - t1) * 1000).utf8 : String(format: "canvas %d: captured %dx%d in %.2f ms, uploaded %d KiB in %.2f ms\n", Int(e.view.id), Int(w), Int(h), (t1 - t0) * 1000, len / 1024, (t2 - t1) * 1000).utf8))
        }
    }

    private func refreshChildren(_ m: GpuModule, _ e: Entry) {
        let each = m.wantsChildrenEach(e.id) != 0
        if each != e.each {
            e.each = each
            e.through = each || m.wantsChildren(e.id) != 0
            e.view.needsCapture = e.through
            if !each { _ = m.childrenCount(e.id, 0) }
            if !each, let overlay = e.view.overlay {
                for case let child as NodeView in overlay.subviews {
                    child.placement = nil; child.placementHidden = false; child.alpha = 1
                }
                overlay.accessibilityElements = nil
                overlay.alpha = e.through ? 0 : 1
            }
        }
    }

    private func bindNow(_ m: GpuModule, _ e: Entry) {
        if bindSurface(m, e) != 0 {
            FileHandle.standardError.write(Data("exact gpu: \(m.error())\n".utf8))
            return
        }
        restoreWorld(m, e)
        refreshChildren(m, e)
        messages(e)
    }

    /// A nested canvas's picture for its ancestor's capture: the last
    /// readback while the module reports nothing dirty on it and its
    /// surface wants no frame at a new clock, else a fresh one.
    func picture(of view: NodeView) -> CGImage? {
        guard let m = module, let e = live(view.id), e.presentable, e.view === view else { return nil }
        if let p = e.picture, m.dirty(e.id) == 0, !(e.wants && (frameNow ?? session?.now() ?? 0) != e.readAt) { return p }
        e.picture = readback(view: view)?.cgImage
        return e.picture
    }

    /// A canvas's picture as pixels, rendered again by the module (LLP 1014):
    /// what a canvas nested under a canvas painted through its surface paints
    /// into its ancestor's capture, since its Metal layer is not seen there.
    func readback(view: NodeView) -> UIImage? {
        guard let m = module, let e = live(view.id), e.presentable, e.view === view, let metal = view.metal else { return nil }
        let scale = CGFloat(metal.layer.contentsScale)
        let w = Int((metal.bounds.width * scale).rounded()), h = Int((metal.bounds.height * scale).rounded())
        guard let bitmap = Bitmap.blank(width: w, height: h), let data = bitmap.bytes else { return nil }
        let at = frameNow ?? session?.now() ?? 0
        let r = m.readback(e.id, Float(metal.bounds.width), Float(metal.bounds.height), Float(scale), at, data.assumingMemoryBound(to: UInt8.self), w * h * 4)
        defer { messages(e) }
        e.readAt = at
        if r == 3 { rendered(e, 3); return nil }
        if r == 1 {
            FileHandle.standardError.write(Data("exact gpu: \(m.error())\n".utf8))
            return nil
        }
        // 2: the surface wants another frame — the ancestor will capture again.
        e.wants = r == 2
        readPlacements(m, e)
        return bitmap.image
    }

    /// Whether the app can be seen (LLP 1009 D4: the host judges what is on
    /// screen). A backgrounded app's Metal layer hands out no drawables, and
    /// asking blocks the main thread for a frame nobody gets; so nothing
    /// renders and nothing wants a frame until the app is active again,
    /// when the scene delegate asks `Frames` to run.
    var visible: Bool {
        // A backgrounded app, or an unmounted view (LLP 1031 D3), wants no frames.
        UIApplication.shared.applicationState == .active && session?.presenter.viewport.window != nil
    }

    /// Whether any surface has something to render — or an edit is under a
    /// canvas painted through its surface, which captures every frame (D4 d).
    var wantsFrames: Bool {
        guard let m = module, visible else { return false }
        return entries.values.contains { e in
            e.needsFrame(dirty:m.dirty(e.id) != 0, editing:e.through && e.view.overlay.map { editing(under: $0) } == true)
        }
    }

    /// Under the agent's clock (LLP 1012's fixed point): the pending
    /// captures painted and every canvas rendered at `now` before a reply
    /// leaves — placements read, nested pictures read back — so what the
    /// next call sees is what this one left, not what the display link got
    /// to. Bounded: a nested canvas that wants a frame asks its ancestor to
    /// capture again, once more here, then the display link has it.
    func settle(now: Double) {
        guard module != nil, !settling, session?.clock != nil else { return }
        settling = true
        let previous = frameNow
        frameNow = now
        defer { frameNow = previous; settling = false }
        for _ in 0..<3 {
            captureIfNeeded()
            _ = tick(now: now)
            for e in Array(entries.values) { messages(e) }
            guard entries.values.contains(where: { $0.view.needsCapture }) else { return }
        }
    }

    /// Render every dirty or wanting surface at `now`; whether more is wanted.
    func tick(now: Double) -> Bool {
        guard let m = module, visible else { return false }
        let previous = frameNow
        frameNow = now
        defer { frameNow = previous }
        var more = false
        for e in Array(entries.values) where live(e.view.id) === e && e.presentable {
            refreshChildren(m, e)
            if e.through && e.view.needsCapture { capture(m, e) }
            // Nested under a canvas painted through its surface (LLP 1014):
            // its Metal layer is never composited, so presenting to it would
            // block on a drawable nobody takes. It is only ever read back into
            // the ancestor's capture — which a change asks for here — and it
            // never wants frames of its own.
            if let outer = e.view.canvasAbove, outer.overlay != nil, entries[outer.id]?.through == true {
                if m.dirty(e.id) != 0 || (e.wants && now != e.readAt) { outer.needsCapture = true }
                continue
            }
            // D4 (d): every frame while editing under the overlay — but not
            // twice on the turn a batch already captured.
            if e.through, !e.view.paintedThisTurn, let overlay = e.view.overlay, editing(under: overlay) { capture(m, e) }
            guard live(e.view.id) === e, e.wants || m.dirty(e.id) != 0, let metal = e.view.metal else { continue }
            // No starvation guard here (the AppKit presenter pauses a canvas
            // whose render took over 200 ms, a covered window's drawable
            // wait): iOS has no occlusion of that kind — a backgrounded app
            // is `visible == false` — and on the simulator the first render
            // of a surface, its pipelines compiling, honestly takes that long.
            let scale = Float(metal.layer.contentsScale)
            let t0 = CACurrentMediaTime()
            let r = m.render(e.id, Float(metal.bounds.width), Float(metal.bounds.height), scale, now)
            windowRenders += 1
            windowRenderSeconds += CACurrentMediaTime() - t0
            if r == 2 { FileHandle.standardError.write(Data("exact gpu: \(m.error())\n".utf8)) }
            rendered(e, r)
            rendered += 1
            more = more || e.wants
            readPlacements(m, e)
            messages(e)
        }
        captureIfNeeded()
        return more
    }
}
#endif
