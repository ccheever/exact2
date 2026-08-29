// The GPU module on macOS (LLP 1009 D2/D4): the app's `<app>-gpu` dylib,
// loaded with dlopen the first time a canvas is on screen — after the first
// painted frame — and driven from the presenter: a surface per canvas node
// on its CAMetalLayer, inputs bound as they arrive, frames rendered while
// a surface is dirty or wants more, from the same display link motion uses.
import AppKit
import QuartzCore

/// A canvas node's backing view: a CAMetalLayer the module renders into.
final class MetalView: NSView {
    override var isFlipped: Bool { true }
    override func makeBackingLayer() -> CALayer {
        let layer = CAMetalLayer()
        layer.isOpaque = true
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

/// Why the module could not be loaded.
struct GpuLoadError: Error { let message: String }

/// The dylib's C ABI (gpu/src/native.rs).
final class GpuModule {
    typealias LoadFn = @convention(c) () -> UInt32
    typealias CreateFn = @convention(c) (UnsafePointer<UInt8>?, Int, UnsafeMutableRawPointer?, UInt32, UInt32) -> UInt32
    typealias BindFn = @convention(c) (UInt32, UnsafePointer<UInt8>?, Int) -> UInt32
    typealias RenderFn = @convention(c) (UInt32, Float, Float, Float, Double) -> UInt32
    typealias DirtyFn = @convention(c) (UInt32) -> UInt32
    typealias DestroyFn = @convention(c) (UInt32) -> Void
    typealias TextureFn = @convention(c) (UInt32, UInt32, UInt32, UnsafePointer<UInt8>?, Int) -> UInt32
    typealias WantsFn = @convention(c) (UInt32) -> UInt32
    typealias ErrorFn = @convention(c) () -> UInt32
    typealias ErrorPtrFn = @convention(c) () -> UnsafePointer<UInt8>?

    let create: CreateFn
    let bind: BindFn
    let render: RenderFn
    let dirty: DirtyFn
    let destroy: DestroyFn
    /// The canvas's children as pixels; whether the surface wants them (LLP 1014).
    let texture: TextureFn
    let wantsChildren: WantsFn
    private let errorLen: ErrorFn
    private let errorPtr: ErrorPtrFn

    /// dlopen the module and create its device; nil (with a reason) when
    /// the library is missing, incomplete, or has no device.
    static func load(path: String) -> Result<GpuModule, GpuLoadError> {
        guard let handle = dlopen(path, RTLD_NOW | RTLD_LOCAL) else {
            return .failure(GpuLoadError(message: "dlopen \(path): \(String(cString: dlerror()))"))
        }
        func sym<T>(_ name: String, _: T.Type) -> T? {
            guard let p = dlsym(handle, name) else { return nil }
            return unsafeBitCast(p, to: T.self)
        }
        guard let load = sym("gpu_load", LoadFn.self), let create = sym("gpu_create", CreateFn.self), let bind = sym("gpu_bind", BindFn.self),
              let render = sym("gpu_render", RenderFn.self), let dirty = sym("gpu_dirty", DirtyFn.self), let destroy = sym("gpu_destroy", DestroyFn.self),
              let texture = sym("gpu_texture", TextureFn.self), let wantsChildren = sym("gpu_wants_children", WantsFn.self),
              let errorLen = sym("gpu_error", ErrorFn.self), let errorPtr = sym("gpu_error_ptr", ErrorPtrFn.self) else {
            return .failure(GpuLoadError(message: "\(path) is not an exact GPU module (missing exports)"))
        }
        let module = GpuModule(create: create, bind: bind, render: render, dirty: dirty, destroy: destroy, texture: texture, wantsChildren: wantsChildren, errorLen: errorLen, errorPtr: errorPtr)
        if load() != 0 { return .failure(GpuLoadError(message: "gpu_load: \(module.error())")) }
        return .success(module)
    }

    private init(create: @escaping CreateFn, bind: @escaping BindFn, render: @escaping RenderFn, dirty: @escaping DirtyFn, destroy: @escaping DestroyFn, texture: @escaping TextureFn, wantsChildren: @escaping WantsFn, errorLen: @escaping ErrorFn, errorPtr: @escaping ErrorPtrFn) {
        self.create = create; self.bind = bind; self.render = render; self.dirty = dirty; self.destroy = destroy; self.texture = texture; self.wantsChildren = wantsChildren; self.errorLen = errorLen; self.errorPtr = errorPtr
    }

    func error() -> String {
        let n = Int(errorLen())
        guard n > 0, let p = errorPtr() else { return "" }
        return String(decoding: UnsafeBufferPointer(start: p, count: n), as: UTF8.self)
    }
}

/// Every canvas on the page and its surface in the module.
final class Canvases {
    final class Entry {
        let view: NodeView
        let name: String
        var values: [Any]
        var id: UInt32 = 0
        var wants = false
        /// The surface samples the children (LLP 1014 D2): the overlay is
        /// captured into its texture and composited at alpha 0.
        var through = false
        /// A children texture has been uploaded (so an emptied overlay is
        /// captured once more, to clear it).
        var uploaded = false
        init(view: NodeView, name: String, values: [Any]) { self.view = view; self.name = name; self.values = values }
    }
    var entries: [UInt32: Entry] = [:]
    var module: GpuModule?
    var failed: String?
    var loadRequested = false
    var loadedMs: Double?
    var rendered = 0
    /// Captures since launch (LLP 1014 D3).
    var captures = 0
    private var captureScheduled = false

    /// Where the module lives: EXACT_GPU_DYLIB, or beside the executable.
    static func modulePath() -> String {
        if let p = ProcessInfo.processInfo.environment["EXACT_GPU_DYLIB"] { return p }
        let exe = URL(fileURLWithPath: CommandLine.arguments[0]).resolvingSymlinksInPath()
        return exe.deletingLastPathComponent().appendingPathComponent("libcaltrain_gpu.dylib").path
    }

    /// A surface op: new inputs for a canvas node.
    func surface(view: NodeView, name: String, values: [Any]) {
        if let e = entries[view.id], e.view !== view { destroy(view: view.id) }
        if let e = entries[view.id] {
            e.values = values
            if e.id != 0, let m = module { bindNow(m, e) }
        } else {
            let e = Entry(view: view, name: name, values: values)
            entries[view.id] = e
            if let m = module { create(m, e) }
        }
    }

    func destroy(view: UInt32) {
        if let e = entries.removeValue(forKey: view), e.id != 0 { module?.destroy(e.id) }
    }

    /// A restart: every surface goes with its view (a reload reuses ids).
    func reset() { for view in Array(entries.keys) { destroy(view: view) } }

    /// Load the module — once, after the first painted frame, only when a
    /// canvas exists. Returns whether a frame source is now wanted.
    func loadIfNeeded() {
        guard !loadRequested, !entries.isEmpty, firstDrawMs != nil else { return }
        loadRequested = true
        let t = CACurrentMediaTime()
        switch GpuModule.load(path: Canvases.modulePath()) {
        case .failure(let e):
            failed = e.message
            FileHandle.standardError.write(Data("exact gpu: \(e.message)\n".utf8))
        case .success(let m):
            module = m
            loadedMs = (CACurrentMediaTime() - t) * 1000
            for e in entries.values { create(m, e) }
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
        if e.id == 0 { FileHandle.standardError.write(Data("exact gpu: \(m.error())\n".utf8)); return }
        bindNow(m, e)
        e.through = m.wantsChildren(e.id) != 0
        if e.through { capture(m, e) }
    }

    /// Every canvas whose children changed since its last capture is painted
    /// again (LLP 1014 D3): the overlay's subtree into a bitmap at the
    /// window's scale, uploaded as the surface's children texture. Called at
    /// the end of every batch, and once per run-loop turn for changes that
    /// arrive outside one (D4 b, c).
    func captureIfNeeded() {
        guard let m = module else { return }
        for e in entries.values where e.id != 0 && e.through && e.view.needsCapture { capture(m, e) }
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
        guard let overlay = e.view.overlay, let win = e.view.window else { return }
        e.view.needsCapture = false
        // Nothing to paint and nothing painted before: no texture at all.
        guard !overlay.subviews.isEmpty || e.uploaded else { return }
        let scale = win.backingScaleFactor
        let t0 = CACurrentMediaTime()
        guard let rep = Capture.bitmap(of: overlay, scale: scale), let data = rep.bitmapData else { return }
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
        if agentMode {
            FileHandle.standardError.write(Data(String(format: "canvas %d: captured %dx%d in %.2f ms, uploaded %d KiB in %.2f ms\n", Int(e.view.id), Int(w), Int(h), (t1 - t0) * 1000, len / 1024, (t2 - t1) * 1000).utf8))
        }
    }

    private func bindNow(_ m: GpuModule, _ e: Entry) {
        guard let data = try? JSONSerialization.data(withJSONObject: e.values) else { return }
        let bytes = [UInt8](data)
        if bytes.withUnsafeBufferPointer({ m.bind(e.id, $0.baseAddress, bytes.count) }) != 0 {
            FileHandle.standardError.write(Data("exact gpu: \(m.error())\n".utf8))
        }
    }

    /// Whether any surface has something to render — or an edit is under a
    /// canvas painted through its surface, which captures every frame (D4 d).
    var wantsFrames: Bool {
        guard let m = module else { return false }
        return entries.values.contains { e in
            e.id != 0 && (e.wants || m.dirty(e.id) != 0 || (e.through && e.view.overlay.map { Canvases.editing(under: $0) } == true))
        }
    }

    /// Render every dirty or wanting surface at `now`; whether more is wanted.
    func tick(now: Double) -> Bool {
        guard let m = module else { return false }
        var more = false
        for e in entries.values where e.id != 0 {
            if e.through, let overlay = e.view.overlay, Canvases.editing(under: overlay) { capture(m, e) }
            guard e.wants || m.dirty(e.id) != 0, let metal = e.view.metal else { continue }
            let scale = Float(metal.layer?.contentsScale ?? 2)
            let r = m.render(e.id, Float(metal.bounds.width), Float(metal.bounds.height), scale, now)
            if r == 2 { FileHandle.standardError.write(Data("exact gpu: \(m.error())\n".utf8)) }
            e.wants = r == 1
            rendered += 1
            more = more || e.wants
        }
        return more
    }
}
