// The GPU module on macOS (LLP 1009 D2/D4): the app's `<app>-gpu` dylib,
// loaded with dlopen the first time a canvas is on screen — after the first
// painted frame — and driven from the presenter: a surface per canvas node
// on its CAMetalLayer, inputs bound as they arrive, frames rendered while
// a surface is dirty or wants more, from the same display link motion uses.
#if os(macOS)
import AppKit
import QuartzCore

/// A canvas node's backing view: a CAMetalLayer the module renders into.
final class MetalView: NSView {
    override var isFlipped: Bool { true }
    override func hitTest(_ point: NSPoint) -> NSView? { nil }
    override func makeBackingLayer() -> CALayer {
        let layer = CAMetalLayer()
        // Composited, never direct-to-display: a layer that covers the whole
        // window and says it is opaque gets a presentation path whose
        // drawables come back only at the next vsync, and every
        // `nextDrawable` then waits a full frame on the main thread — a
        // full-window canvas rendered on the display link starved the app
        // (LLP 1014, the app inside the sky). A translucent layer stays in
        // the compositor, where a drawable comes back as soon as it is read.
        layer.isOpaque = false
        return layer
    }
    override init(frame: NSRect) {
        super.init(frame: frame)
        wantsLayer = true
        layerContentsRedrawPolicy = .never
        autoresizingMask = [.width, .height]
    }
    required init?(coder: NSCoder) { nil }
    override func viewDidChangeBackingProperties() {
        super.viewDidChangeBackingProperties()
        layer?.contentsScale = window?.backingScaleFactor ?? 2
    }
}

/// Every canvas on one session's page and its surface in the module — the
/// module itself loaded once per process (LLP 1031 D12).
final class Canvases {
    lazy var lifecycle = CanvasLifecycle(self)
    weak var session: ExactSession?
    /// postMessage events waiting for a live canvas of their surface name.
    var pendingPosts: [(name: String, text: String, at: Double)] = []
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
        /// A render blocked in `nextDrawable`: the layer's drawables are not
        /// coming back (the window is covered and the occlusion notice has
        /// not landed yet). No presenting until the occlusion state changes
        /// or this much wall time has passed, so one lost second is not
        /// every frame lost.
        var starvedUntil: Double = 0
        /// The artifact whose surface this canvas is (LLP 1009 D6), once created.
        var module: GpuModule?
        init(view: NodeView, name: String, values: Any) { self.view = view; self.name = name; self.values = values }
    }
    var entries: [UInt32: Entry] = [:]
    var publishers: [String: Entry] = [:]
    /// The loaded artifacts by name, "" the primary (LLP 1009 D6); each is
    /// asked for once, the first time one of its canvases exists.
    var modules: [String: GpuModule] = [:]
    var attempted: Set<String> = []
    /// Module loads scheduled for the turn after their canvas mounted.
    var deferred: Set<String> = []
    private lazy var compat = GpuModule.bakedCompatibility
    /// Each surface's artifact, read from the baked card once: `ready` asks
    /// for every canvas's on every frame.
    private var artifacts: [String: String] = [:]
    func artifact(_ surface: String) -> String {
        if let known = artifacts[surface] { return known }
        let found = GpuModule.artifact(for: surface, compat: compat)
        artifacts[surface] = found
        return found
    }
    /// Every canvas's artifact loaded or refused: what an agent read waits for.
    var ready: Bool { deferred.isEmpty && entries.values.allSatisfy { attempted.contains(artifact($0.name)) } }
    var displayPeriod = DisplayPeriod()
    func period(_ ms: Double) {
        guard !modules.isEmpty else { return }
        let loaded = Array(modules.values)
        displayPeriod.publish(ms, maximum: Double(session?.presenter.viewport.window?.screen?.maximumFramesPerSecond ?? 120)) { ms in for m in loaded { m.period?(ms) } }
    }
    var worldInput = WorldCarrier.read(ExactEnv.agentMode ? ProcessInfo.processInfo.environment["EXACT_WORLD"] : nil)
    var terminalRestoreReported = false
    var restoreJournal: [[String: Any]] = []
    var failed: String?
    var loadedMs: Double?
    var rendered = 0
    /// Modules a render recorded into since their last flush (LLP 1009 D7).
    var unflushed: [GpuModule] = []
    /// Captures since launch (LLP 1014 D3).
    var captures = 0
    private var captureScheduled = false
    var frameNow: Double?
    var settling = false

    /// The smoke's line: loaded (with the load time and counts) or why not.
    var status: String {
        if let failed { return "failed: \(failed)" }
        if !modules.isEmpty { return "module loaded in \(String(format: "%.1f", loadedMs ?? 0)) ms; \(entries.count) canvases; \(rendered) renders" }
        return "not loaded: \(failed ?? (entries.isEmpty ? "no canvas" : "not requested"))"
    }

    /// Where an artifact lives: beside the executable; the primary's may be
    /// EXACT_GPU_DYLIB in a development build.
    static func modulePath(_ artifact: String = "") -> String {
        let exe = URL(fileURLWithPath: CommandLine.arguments[0]).resolvingSymlinksInPath()
        let path = exe.deletingLastPathComponent().appendingPathComponent(GpuModule.loadName(artifact: artifact)).path
        return artifact.isEmpty ? GpuModule.modulePath(defaultPath:path, compat:GpuModule.bakedCompatibility, environment:ProcessInfo.processInfo.environment) : path
    }

    /// A shader's text changed (the asset row, LLP 1030 D10): the module
    /// takes it if its interface is the one it binds — every surface then
    /// renders again through the new pipeline — or refuses it by name and
    /// keeps the old one; a module not loaded yet reads the file when it is.
    func shaderChanged(_ name: String, text: Data) {
        guard let m = modules[""] else { return }
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
            if e.id != 0, let m = e.module { bindNow(m, e) }
        } else {
            let e = Entry(view: view, name: name, values: values)
            entries[view.id] = e
            claimPublisher(e)
            if let m = modules[artifact(name)] { create(m, e) }
        }
    }

    func destroy(view: UInt32) {
        if let e = entries.removeValue(forKey: view) {
            e.view.canvasInput = nil
            if e.id != 0 { e.module?.destroy(e.id) }
            releasePublisher(e)
        }
    }

    /// A restart: every surface goes with its view (a reload reuses ids).
    func reset() { for view in Array(entries.keys) { destroy(view: view) } }

    /// Load the module — once, after the first painted frame, only when a
    /// canvas exists. Returns whether a frame source is now wanted.
    func loadIfNeeded() {
        guard !entries.isEmpty, session?.firstDrawMs != nil else { return }
        // Each artifact the first time one of its canvases exists (LLP 1009
        // D6). A declared module loads on the next turn, after the frame that
        // mounted its canvas (D4); the primary as it always has.
        for key in Set(entries.values.map { artifact($0.name) }).sorted() where !attempted.contains(key) {
            attempted.insert(key)
            if key.isEmpty { load(key); continue }
            deferred.insert(key)
            DispatchQueue.main.async { [weak self] in
                guard let self, deferred.remove(key) != nil else { return }
                load(key)
                session?.frames.requestCanvas()
                // Work queued while this wave loaded settles when the wave's
                // last module has loaded or failed: no later batch is owed to
                // drain it (an idle tick is skipped). Not before, or a module
                // still loading would answer its tickets "unavailable".
                if deferred.isEmpty { session?.drainSurfaceWorkNow() }
            }
        }
    }

    private func load(_ key: String) {
        let t = CACurrentMediaTime()
        switch GpuModule.loadShared(path: Canvases.modulePath(key), artifact: key) {
        case .failure(let e):
            failed = e.message
            FileHandle.standardError.write(Data("exact gpu: \(e.message)\n".utf8))
        case .success(let m):
            modules[key] = m
            m.canvases.add(self)
            loadedMs = loadedMs ?? (CACurrentMediaTime() - t) * 1000
            // The shaders as files (LLP 1030 D8), from the app's asset root
            // (LLP 1031 D1): the app's directory in dev, the bundle in a
            // release. They are the primary's (LLP 1009 D6).
            if key.isEmpty, let resolver = session?.app.resolver { m.registerShaders(resolver: resolver) }
            for e in Array(entries.values) where entries[e.view.id] === e && e.module == nil && artifact(e.name) == key { create(m, e) }
        }
    }

    private func create(_ m: GpuModule, _ e: Entry) {
        guard let metal = e.view.metal, let layer = metal.layer else { return }
        layer.contentsScale = metal.window?.backingScaleFactor ?? 2
        let scale = Float(layer.contentsScale)
        let w = UInt32(max(1, (Float(metal.bounds.width) * scale).rounded()))
        let h = UInt32(max(1, (Float(metal.bounds.height) * scale).rounded()))
        let ptr = Unmanaged.passUnretained(layer).toOpaque()
        let bytes = Array(e.name.utf8)
        e.id = bytes.withUnsafeBufferPointer { m.create($0.baseAddress, bytes.count, ptr, w, h) }
        e.presentable = e.id != 0
        if e.id == 0 { FileHandle.standardError.write(Data("exact gpu: \(m.error())\n".utf8)); return }
        e.module = m
        lifecycle.deliver(e.id, module: m)
        bindNow(m, e)
        e.each = m.wantsChildrenEach(e.id) != 0
        e.through = e.each || m.wantsChildren(e.id) != 0
        e.wantsInput = m.wantsInput?(e.id) == 1 && m.input != nil
        deliverPosts() // a post held for this surface, if the canvas is live already
        if e.wantsInput { e.view.canvasInput = CanvasInput(view: e.view) }
        if e.through { capture(m, e) }
    }

    /// Each direct child of the overlay captured on its own and uploaded with
    /// its frame (LLP 1014 D5). Placed children composite through the surface;
    /// unplaced HUD children keep their ordinary overlay. Hit-testing follows
    /// the placements the surface reports after each frame.
    private func captureEach(_ m: GpuModule, _ e: Entry, overlay: NSView, scale: CGFloat) -> Bool {
        let children = overlay.subviews.compactMap { $0 as? NodeView }
        var uploaded = 0
        let t0 = CACurrentMediaTime()
        for (i, child) in children.enumerated() {
            if child.frame.width <= 0 || child.frame.height <= 0 || child.style["display"]?.string == "none" {
                let r = m.child(e.id, UInt32(i), child.props["testId"] ?? "", 0, 0, 0, 0, 0, 0, nil, 0)
                if r != 0 { return false }
                continue
            }
            let hidden = child.hiddenByHost
            if child.placementHidden { child.isHidden = false }
            defer { child.isHidden = hidden }
            guard let rep = Capture.bitmap(of: child, scale: scale), let data = rep.bitmapData else { continue }
            guard live(e.view.id) === e else { return false }
            let f = child.frame
            let r = m.child(e.id, UInt32(i), child.props["testId"] ?? "", Float(f.origin.x), Float(f.origin.y), Float(f.width), Float(f.height), UInt32(rep.pixelsWide), UInt32(rep.pixelsHigh), UnsafePointer(data), rep.pixelsHigh * rep.bytesPerRow)
            if r != 0 { FileHandle.standardError.write(Data("exact gpu: \(m.error())\n".utf8)); return false }
            uploaded += 1
        }
        _ = m.childrenCount(e.id, UInt32(children.count))
        if ExactEnv.agentMode {
            FileHandle.standardError.write(Data(String(format: "canvas %d: captured %d children in %.2f ms\n", Int(e.view.id), uploaded, (CACurrentMediaTime() - t0) * 1000).utf8))
        }
        return true
    }

    /// The window's occlusion changed: whatever was starved may try again.
    func occlusionChanged() {
        lifecycle.refresh()
        for e in entries.values { e.starvedUntil = 0 }
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
            child.alphaValue = outcome == 0 ? 1 : 0
            if changed { child.placementChanged() }
        }
    }

    /// Every canvas whose children changed since its last capture is painted
    /// again (LLP 1014 D3): the overlay's subtree into a bitmap at the
    /// window's scale, uploaded as the surface's children texture. Called at
    /// the end of every batch, and once per run-loop turn for changes that
    /// arrive outside one (D4 b, c).
    func captureIfNeeded() {
        for e in Array(entries.values) where live(e.view.id) === e && e.presentable && e.through && e.view.needsCapture {
            if let m = e.module { capture(m, e) }
        }
    }

    /// A capture on this run-loop turn, coalesced.
    func scheduleCapture() {
        guard !captureScheduled else { return }
        captureScheduled = true
        DispatchQueue.main.async { self.captureScheduled = false; self.captureIfNeeded() }
    }

    /// Whether the first responder — a field's editor — is under the overlay
    /// (D4 d): carets and selection repaint without any batch.
    static func editing(under overlay: NSView) -> Bool {
        guard let r = overlay.window?.firstResponder else { return false }
        let v = (r as? NSView) ?? ((r as? NSText)?.delegate as? NSView)
        return v?.isDescendant(of: overlay) ?? false
    }

    private func capture(_ m: GpuModule, _ e: Entry) {
        // A nested readback can post a message and apply another batch here.
        guard live(e.view.id) === e, e.presentable, !e.capturing else { return }
        e.capturing = true
        defer { e.capturing = false }
        guard let overlay = e.view.overlay, let win = e.view.window else { return }
        e.view.needsCapture = false
        // Nothing to paint and nothing painted before: no texture at all.
        guard !overlay.subviews.isEmpty || e.uploaded else { return }
        let scale = win.backingScaleFactor
        if e.each {
            if captureEach(m, e, overlay: overlay, scale: scale) { e.uploaded = true; captures += 1; overlay.alphaValue = 1; e.view.paintedThisTurn = true; DispatchQueue.main.async { e.view.paintedThisTurn = false } }
            return
        }
        let t0 = CACurrentMediaTime()
        guard let rep = Capture.bitmap(of: overlay, scale: scale), let data = rep.bitmapData else { return }
        guard live(e.view.id) === e else { return }
        let t1 = CACurrentMediaTime()
        let w = UInt32(rep.pixelsWide), h = UInt32(rep.pixelsHigh)
        let len = rep.pixelsHigh * rep.bytesPerRow
        let r = m.texture(e.id, w, h, UnsafePointer(data), len)
        let t2 = CACurrentMediaTime()
        if r != 0 { FileHandle.standardError.write(Data("exact gpu: \(m.error())\n".utf8)); return }
        e.uploaded = true
        captures += 1
        // Painted through the surface from here on: the overlay stays laid
        // out — hit-testable, in the accessibility hierarchy — and is no
        // longer composited itself (D5: pixels may distort, boxes may not).
        overlay.alphaValue = 0
        e.view.paintedThisTurn = true
        DispatchQueue.main.async { e.view.paintedThisTurn = false }
        if ExactEnv.agentMode {
            FileHandle.standardError.write(Data(String(format: "canvas %d: captured %dx%d in %.2f ms, uploaded %d KiB in %.2f ms\n", Int(e.view.id), Int(w), Int(h), (t1 - t0) * 1000, len / 1024, (t2 - t1) * 1000).utf8))
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
                    child.placement = nil; child.placementHidden = false; child.alphaValue = 1
                }
                overlay.alphaValue = e.through ? 0 : 1
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

    /// A canvas's picture as pixels, rendered again by the module (LLP 1014):
    /// what a canvas nested under a canvas painted through its surface paints
    /// into its ancestor's capture, since its Metal layer is not seen there.
    func readback(view: NodeView) -> NSBitmapImageRep? {
        guard let e = live(view.id), let m = e.module, e.presentable, e.view === view, let metal = view.metal else { return nil }
        let scale = CGFloat(metal.layer?.contentsScale ?? 2)
        let w = Int((metal.bounds.width * scale).rounded()), h = Int((metal.bounds.height * scale).rounded())
        guard w > 0, h > 0,
              let rep = NSBitmapImageRep(bitmapDataPlanes: nil, pixelsWide: w, pixelsHigh: h, bitsPerSample: 8, samplesPerPixel: 4, hasAlpha: true, isPlanar: false, colorSpaceName: .deviceRGB, bytesPerRow: w * 4, bitsPerPixel: 32),
              let data = rep.bitmapData
        else { return nil }
        rep.size = metal.bounds.size
        let at = frameNow ?? session?.now() ?? 0
        let r = m.readback(e.id, Float(metal.bounds.width), Float(metal.bounds.height), Float(scale), at, data, w * h * 4)
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
        return rep
    }

    /// Whether the window can be seen (LLP 1009 D4: the host judges what is
    /// on screen). An occluded window's Metal layer hands out no drawables,
    /// and asking blocks the main thread — up to a second — for a frame
    /// nobody gets (measured closing LLP 1014 §4.1); so nothing renders
    /// and nothing wants a frame until the window is seen again, when the
    /// delegate asks `Frames` to run.
    var visible: Bool {
        guard let viewport = session?.presenter.viewport else { return false }
        // An unmounted view wants no frames (LLP 1031 D3).
        return !NSApplication.shared.isHidden && (viewport.window?.occlusionState.contains(.visible) ?? false)
    }

    /// Whether any surface has something to render — or an edit is under a
    /// canvas painted through its surface, which captures every frame (D4 d).
    var wantsFrames: Bool {
        guard !modules.isEmpty, visible else { return false }
        return entries.values.contains { e in
            e.needsFrame(dirty:(e.module?.dirty(e.id) ?? 0) != 0, editing:e.through && e.view.overlay.map { Canvases.editing(under: $0) } == true)
        }
    }

    /// Under the agent's clock (LLP 1012's fixed point): the pending
    /// captures painted and every canvas rendered at `now` before a reply
    /// leaves — placements read, nested pictures read back — so what the
    /// next call sees is what this one left, not what the display link got
    /// to. Bounded: a nested canvas that wants a frame asks its ancestor to
    /// capture again, once more here, then the display link has it.
    func settle(now: Double) {
        guard !modules.isEmpty, !settling, session?.clock != nil else { return }
        settling = true
        let previous = frameNow
        frameNow = now
        defer { frameNow = previous; settling = false }
        for _ in 0..<3 {
            captureIfNeeded()
            _ = tick(now: now)
            // The agent owns presentation time even behind another window. An
            // offscreen target cannot starve and refreshes the same placements.
            if ExactEnv.agentMode {
                for e in Array(entries.values) where !visible || e.starvedUntil > CACurrentMediaTime() {
                    _ = readback(view: e.view)
                }
            }
            for e in Array(entries.values) { messages(e) }
            guard entries.values.contains(where: { $0.view.needsCapture }) else { return }
        }
    }

    /// Render every dirty or wanting surface at `now`; whether more is wanted.
    func tick(now: Double) -> Bool {
        guard !modules.isEmpty, visible else { return false }
        let previous = frameNow
        frameNow = now
        defer { frameNow = previous }
        var more = false
        for e in Array(entries.values) where live(e.view.id) === e && e.presentable {
            guard let m = e.module else { continue }
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
            if e.through, !e.view.paintedThisTurn, let overlay = e.view.overlay, Canvases.editing(under: overlay) { capture(m, e) }
            m.syncDynamicRange(e.id, view: e.view, layer: e.view.metal?.layer)
            guard live(e.view.id) === e, e.wants || m.dirty(e.id) != 0, let metal = e.view.metal else { continue }
            let wall = CACurrentMediaTime()
            if wall < e.starvedUntil { more = true; continue }
            let scale = Float(metal.layer?.contentsScale ?? 2)
            let r = m.render(e.id, Float(metal.bounds.width), Float(metal.bounds.height), scale, now)
            recorded(m)
            if CACurrentMediaTime() - wall > 0.2 {
                e.starvedUntil = CACurrentMediaTime() + 1.0
                if ExactEnv.agentMode { FileHandle.standardError.write(Data("exact gpu: canvas \(e.view.id) waited \(Int((CACurrentMediaTime() - wall) * 1000)) ms for a drawable; not presenting for a second\n".utf8)) }
            }
            if r == 2 { FileHandle.standardError.write(Data("exact gpu: \(m.error())\n".utf8)) }
            rendered(e, r)
            rendered += 1
            more = more || e.wants
            readPlacements(m, e)
            messages(e)
        }
        flushRecorded()
        lastTickNow = now
        captureIfNeeded()
        return more
    }

    /// The frame time of the last tick: a starved canvas's late render draws that frame.
    private var lastTickNow: Double?

    /// A starved canvas's drawable arrived (gpu/src/acquire.rs): the canvases
    /// whose render at the tick went without one draw the tick's frame now,
    /// instead of the main thread waiting in `nextDrawable` at the tick.
    func renderStarved() {
        guard !modules.isEmpty, visible, !settling, let now = lastTickNow else { return }
        let previous = frameNow
        frameNow = now
        defer { frameNow = previous }
        for e in Array(entries.values) where live(e.view.id) === e && e.presentable {
            guard let m = e.module, m.starved?(e.id) == 1, let metal = e.view.metal else { continue }
            let scale = Float(metal.layer?.contentsScale ?? 2)
            let r = m.render(e.id, Float(metal.bounds.width), Float(metal.bounds.height), scale, now)
            recorded(m)
            if r == 2 { FileHandle.standardError.write(Data("exact gpu: \(m.error())\n".utf8)) }
            rendered(e, r)
            rendered += 1
            readPlacements(m, e)
            messages(e)
        }
        flushRecorded()
    }
}
#endif
