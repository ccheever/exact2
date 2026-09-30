// The GPU module's C ABI, loaded (LLP 1009 D2): the app's `<app>-gpu`
// dylib, `dlopen`ed by a presenter the first time a canvas is on screen —
// after the first painted frame. Shared by the AppKit and UIKit presenters;
// what each does with a surface (its `Canvases`) is its own. An app that
// declares GPU modules has one more dylib per module (LLP 1009 D6), each an
// instance of this class, loaded the first time a canvas of one of its
// surfaces is; a canvas keeps the instance that created it.
import Foundation
import CryptoKit
import CExact
#if os(macOS)
import Metal
#endif

/// Why the module could not be loaded.
struct GpuLoadError: Error { let message: String }

/// The dylib's C ABI (gpu/src/native.rs). Loaded once per process (LLP 1031
/// D12): `gpu_load` creates the device and the module's instance table, and
/// a second load would replace them under the first session's surfaces.
final class GpuModule {
    /// Each artifact's one load: "" is the primary, else a declared module's name.
    nonisolated(unsafe) private static var shared: [String: Result<GpuModule, GpuLoadError>] = [:]

    /// The process's module for `artifact`: loaded on the first ask, the same answer after.
    static func loadShared(path: String, artifact: String = "") -> Result<GpuModule, GpuLoadError> {
        if let loaded = shared[artifact] { return loaded }
        let r = load(path: path, artifact: artifact)
        shared[artifact] = r
        return r
    }

    /// The artifact that owns `surface` (LLP 1009 D6): the declared module
    /// that lists it (`compat.inputs.gpuModules`), else the primary, "".
    static func artifact(for surface: String, compat: [String: Any]) -> String {
        let modules = (compat["inputs"] as? [String: Any])?["gpuModules"] as? [String: [String]] ?? [:]
        return modules.first { $0.value.contains(surface) }?.key ?? ""
    }
    /// A module's dylib beside the primary's, under a name no app can collide with.
    static func loadName(artifact: String) -> String {
        artifact.isEmpty ? "libexact_gpu.dylib" : "libexact_gpu_\(artifact.replacingOccurrences(of: "-", with: "_")).dylib"
    }

    static var bakedCompatibility: [String: Any] {
        let bytes = Runtime.bakedCompat()
        return (try? JSONSerialization.jsonObject(with: bytes) as? [String: Any]) ?? [:]
    }
    static func modulePath(defaultPath: String, compat: [String: Any], environment: [String: String]) -> String {
        let gpu = (compat["embedded"] as? [String: Any])?["gpu"] as? [String: Any]
        return gpu?["trust"] as? String == "development" ? environment["EXACT_GPU_DYLIB"] ?? defaultPath : defaultPath
    }
    static func verify(path: String, compat: [String: Any], artifact: String = "") -> GpuLoadError? {
        func refusal(_ reason: String) -> GpuLoadError { GpuLoadError(message:"GPU module \(path): \(reason)") }
        let embedded = compat["embedded"] as? [String: Any]
        // A module's card is its own (LLP 1009 D6): one signed digest per artifact.
        let found = artifact.isEmpty ? embedded?["gpu"] : (embedded?["gpuModules"] as? [String: Any])?[artifact]
        guard let card = found as? [String: Any] else {
            return refusal("missing baked identity")
        }
        guard let app = (compat["inputs"] as? [String: Any])?["app"] as? String, card["app"] as? String == app else {
            return refusal("app identity mismatch")
        }
        guard let cohort = compat["id"] as? String, card["cohort"] as? String == cohort else {
            return refusal("cohort identity mismatch")
        }
        do {
            let bytes = try Data(contentsOf: URL(fileURLWithPath:path))
            let digest = SHA256.hash(data:bytes).map { String(format:"%02x", $0) }.joined()
            if card["sha256"] as? String != digest { return refusal("digest mismatch") }
        } catch { return refusal(error.localizedDescription) }
        return nil
    }

    typealias LoadFn = @convention(c) () -> UInt32
    typealias CreateFn = @convention(c) (UnsafePointer<UInt8>?, Int, UnsafeMutableRawPointer?, UInt32, UInt32) -> UInt32
    typealias BindFn = @convention(c) (UInt32, UnsafePointer<UInt8>?, Int) -> UInt32
    typealias BindAtFn = @convention(c) (UInt32, UnsafePointer<UInt8>?, Int, Double) -> UInt32
    var bindAt: BindAtFn?
    typealias RenderFn = @convention(c) (UInt32, Float, Float, Float, Double) -> UInt32
    typealias DirtyFn = @convention(c) (UInt32) -> UInt32
    typealias DestroyFn = @convention(c) (UInt32) -> Void
    typealias TextureFn = @convention(c) (UInt32, UInt32, UInt32, UnsafePointer<UInt8>?, Int) -> UInt32
    typealias TextureMetalFn = @convention(c) (UInt32, UInt32, UInt32, UnsafeMutableRawPointer?) -> UInt32
    typealias SyncFn = @convention(c) () -> UInt32
    typealias WantsFn = @convention(c) (UInt32) -> UInt32
    typealias ReadbackFn = @convention(c) (UInt32, Float, Float, Float, Double, UnsafeMutablePointer<UInt8>?, Int) -> UInt32
    typealias ChildFn = @convention(c) (UInt32, UInt32, UnsafePointer<UInt8>?, Int, Float, Float, Float, Float, UInt32, UInt32, UnsafePointer<UInt8>?, Int) -> UInt32
    typealias CountFn = @convention(c) (UInt32, UInt32) -> UInt32
    typealias PlacementFn = @convention(c) (UInt32, UInt32, UnsafeMutablePointer<Float>?, Int) -> UInt32
    typealias ErrorFn = @convention(c) () -> UInt32
    typealias ErrorPtrFn = @convention(c) () -> UnsafePointer<UInt8>?
    typealias ClearShadersFn = @convention(c) () -> Void
    typealias ShaderFn = @convention(c) (UnsafePointer<UInt8>?, Int, UnsafePointer<UInt8>?, Int) -> UInt32

    typealias LifecycleFn = @convention(c) (UInt32, UInt32) -> Void
    var lifecycle: LifecycleFn?
    /// The display's frame period in milliseconds, for every frame after (0 = unknown).
    var period: PeriodFn?

    typealias SeekableFn = @convention(c) (Bool) -> Void
    typealias PeriodFn = @convention(c) (Double) -> Void

    let wantsInput: WantsFn?
    let input: BindFn?
    typealias RestoreFn = @convention(c) (UInt32, UnsafePointer<UInt8>?, Int, UInt32) -> Bool
    typealias AssetFn = @convention(c) (UInt32, UnsafePointer<UInt8>?, Int, UnsafePointer<UInt8>?, Int) -> Bool
    var assets: WantsFn?
    var asset: AssetFn?
    var assetFailed: AssetFn?
    var carry: WantsFn?
    var restore: RestoreFn?
    let published: WantsFn?
    let messages: WantsFn?
    let agent: BindFn?
    private let outPtr: ErrorPtrFn?

    var recover: LoadFn?
    /// Whether a canvas's last render went without a drawable (the module
    /// acquires off the main thread and says when one arrives).
    var starved: WantsFn?
    typealias AcquiredFn = @convention(c) () -> Void
    typealias OnAcquireFn = @convention(c) (AcquiredFn?) -> Void
    let canvases = NSHashTable<Canvases>.weakObjects()
    private var recovering = false
    private var deviceObserver: NSObjectProtocol?
    typealias DeviceIDFn = @convention(c) () -> UInt64
    typealias LostFn = @convention(c) () -> Bool
    var deviceID: DeviceIDFn?
    var deviceLost: LostFn?
    private(set) var lossGeneration: UInt64 = 0
    private var activeDeviceID: UInt64 = 0
    private var failures = 0

    func deliveryClock(_ entry: Canvases.Entry, now: Double) -> [String: Any] {
        let redelivery = entry.recoveryRedelivery
        entry.recoveryRedelivery = false
        return redelivery ? ["op": "clock"] : ["op": "clock", "now": now]
    }

    func removedDevice(_ registryID: UInt64, generation: UInt64) {
        guard registryID == activeDeviceID, generation == lossGeneration else { return }
        recoverDevice()
    }

    func recoverDevice() {
        guard !recovering, deviceLost?() == true else { return }
        recovering = true
        let generation = lossGeneration
        // Another artifact's canvases keep their device (LLP 1009 D6).
        let recoveryEntries = canvases.allObjects.flatMap { $0.entries.values.filter { $0.module == nil || $0.module === self } }
        let delay = failures == 0 ? 0 : min(5.0, 0.1 * pow(2.0, Double(failures - 1)))
        DispatchQueue.main.asyncAfter(deadline: .now() + delay) { [weak self] in
            guard let self else { return }
            defer { self.recovering = false }
            guard generation == self.lossGeneration, self.deviceLost?() == true else { return }
            let data = self.recover.flatMap { self.output($0()) }
            let outcome = data.flatMap { try? JSONSerialization.jsonObject(with: $0) as? [String: Any] }
            if outcome?["status"] as? String == "healthy" { return }
            let ok = outcome?["status"] as? String == "recovered" && self.deviceLost?() == false
            if ok {
                self.lossGeneration += 1; self.failures = 0
                self.activeDeviceID = self.deviceID?() ?? 0
                self.observeDevice()
            } else { self.failures += 1 }
            let reason = ok ? nil : "device recovery: \(self.error()) \(outcome ?? [:])"
            // Retaining the entries until here prevents address reuse during backoff.
            let entries = Set(recoveryEntries.map(ObjectIdentifier.init))
            for owner in self.canvases.allObjects { owner.recoveredDevice(ok, error: reason, recovering: entries) }
            if !ok { DispatchQueue.main.async { [weak self] in self?.recoverDevice() } }
        }
    }

    private func observeDevice() {
        #if os(macOS)
        if let deviceObserver { MTLRemoveDeviceObserver(deviceObserver) }
        let generation = lossGeneration
        let devices = MTLCopyAllDevicesWithObserver { [weak self] device, name in
            if name == .wasRemoved || name == .removalRequested {
                DispatchQueue.main.async { self?.removedDevice(device.registryID, generation: generation) }
            }
        }
        deviceObserver = devices.observer
        #endif
    }

    deinit {
        #if os(macOS)
        if let deviceObserver { MTLRemoveDeviceObserver(deviceObserver) }
        #endif
    }

    let create: CreateFn
    let bind: BindFn
    /// Records a canvas's frame; nothing is shown until `flush()`.
    let render: RenderFn
    private var flushFn: SyncFn?
    /// Submit the frame every canvas rendered since the last flush, once, and
    /// present them (LLP 1009 D7): after a tick's renders.
    func flush() {
        if flushFn?() == 1 { FileHandle.standardError.write(Data("exact gpu: \(error())\n".utf8)) }
    }
    let dirty: DirtyFn
    let destroy: DestroyFn
    /// The canvas's children as pixels; whether the surface wants them (LLP 1014).
    let texture: TextureFn
    /// The children as a Metal texture the presenter rendered, imported as it
    /// is (LLP 1008 §9) — when the module exports it (Apple targets).
    let textureMetal: TextureMetalFn?
    /// The module's GPU work complete — before a texture it read is drawn
    /// into again.
    let sync: SyncFn?
    let childrenMode: WantsFn
    /// A canvas's picture as pixels (LLP 1014, nested canvases).
    let readback: ReadbackFn
    /// Each child as its own texture, and where the surface put it (LLP 1014 D5).
    func wantsChildren(_ id: UInt32) -> UInt32 { (1...2).contains(childrenMode(id)) ? 1 : 0 }
    func wantsChildrenEach(_ id: UInt32) -> UInt32 { childrenMode(id) == 3 ? 1 : 0 }
    private let childView: ChildFn
    func child(_ id: UInt32, _ index: UInt32, _ name: String, _ x: Float, _ y: Float, _ w: Float, _ h: Float, _ width: UInt32, _ height: UInt32, _ pixels: UnsafePointer<UInt8>?, _ count: Int) -> UInt32 {
        name.utf8CString.withUnsafeBufferPointer { bytes in
            bytes.baseAddress!.withMemoryRebound(to: UInt8.self, capacity: bytes.count) {
                childView(id, index, $0, bytes.count - 1, x, y, w, h, width, height, pixels, count)
            }
        }
    }
    let childrenCount: CountFn
    let placement: PlacementFn
    /// A shader's text by name (LLP 1030 D8): validated, its interface
    /// checked against the module's; 0 on success, else `error()` says why.
    let shader: ShaderFn?
    let validateShader: ShaderFn?
    let clearShaders: ClearShadersFn?
    private let errorLen: ErrorFn
    private let errorPtr: ErrorPtrFn

    /// dlopen the module and create its device; nil (with a reason) when
    /// the library is missing, incomplete, or has no device.
    static func load(path: String, artifact: String = "") -> Result<GpuModule, GpuLoadError> {
        if let error = verify(path:path, compat:bakedCompatibility, artifact:artifact) { return .failure(error) }
        guard let handle = dlopen(path, RTLD_NOW | RTLD_LOCAL) else {
            return .failure(GpuLoadError(message: "dlopen \(path): \(String(cString: dlerror()))"))
        }
        func sym<T>(_ name: String, _: T.Type) -> T? {
            guard let p = dlsym(handle, name) else { return nil }
            return unsafeBitCast(p, to: T.self)
        }
        guard let load = sym("gpu_load", LoadFn.self), let create = sym("gpu_create", CreateFn.self), let bind = sym("gpu_bind", BindFn.self),
              let render = sym("gpu_render", RenderFn.self), let flush = sym("gpu_flush", SyncFn.self), let dirty = sym("gpu_dirty", DirtyFn.self), let destroy = sym("gpu_destroy", DestroyFn.self),
              let texture = sym("gpu_texture", TextureFn.self), let childrenMode = sym("gpu_children_mode", WantsFn.self),
              let readback = sym("gpu_readback", ReadbackFn.self),
              let child = sym("gpu_child_view", ChildFn.self),
              let childrenCount = sym("gpu_children_count", CountFn.self), let placement = sym("gpu_placement", PlacementFn.self),
              let errorLen = sym("gpu_error", ErrorFn.self), let errorPtr = sym("gpu_error_ptr", ErrorPtrFn.self) else {
            return .failure(GpuLoadError(message: "\(path) is not an exact GPU module (missing exports)"))
        }
        let module = GpuModule(create: create, bind: bind, render: render, dirty: dirty, destroy: destroy, texture: texture, textureMetal: sym("gpu_texture_metal", TextureMetalFn.self), sync: sym("gpu_sync", SyncFn.self), childrenMode: childrenMode, readback: readback, child: child, childrenCount: childrenCount, placement: placement, shader: sym("gpu_shader", ShaderFn.self), validateShader: sym("gpu_shader_validate", ShaderFn.self), clearShaders: sym("gpu_shaders_clear", ClearShadersFn.self), errorLen: errorLen, errorPtr: errorPtr, wantsInput: sym("gpu_wants_input", WantsFn.self), input: sym("gpu_input", BindFn.self), messages: sym("gpu_messages", WantsFn.self), published: sym("gpu_published", WantsFn.self), agent: sym("gpu_agent", BindFn.self), outPtr: sym("gpu_out_ptr", ErrorPtrFn.self))
        module.flushFn = flush
        if load() != 0 { return .failure(GpuLoadError(message: "gpu_load: \(module.error())")) }
        module.recover = sym("gpu_recover", LoadFn.self)
        module.deviceID = sym("gpu_device_registry_id", DeviceIDFn.self)
        module.deviceLost = sym("gpu_device_is_lost", LostFn.self)
        module.activeDeviceID = module.deviceID?() ?? 0
        module.observeDevice()
        module.lifecycle = sym("gpu_lifecycle", LifecycleFn.self)
        module.period = sym("gpu_period", PeriodFn.self)
        module.bindAt = sym("gpu_bind_at", BindAtFn.self)
        module.assets = sym("gpu_assets", WantsFn.self); module.asset = sym("gpu_asset", AssetFn.self); module.assetFailed = sym("gpu_asset_failed", AssetFn.self)
        module.carry = sym("gpu_carry", WantsFn.self); module.restore = sym("gpu_restore", RestoreFn.self)
        sym("gpu_seekable", SeekableFn.self)?(ExactEnv.agentFreezes)
        module.starved = sym("gpu_starved", WantsFn.self)
        if module.starved != nil { sym("gpu_on_acquire", OnAcquireFn.self)?(gpuAcquired) }
        return .success(module)
    }

    init(create: @escaping CreateFn, bind: @escaping BindFn, render: @escaping RenderFn, dirty: @escaping DirtyFn, destroy: @escaping DestroyFn, texture: @escaping TextureFn, textureMetal: TextureMetalFn?, sync: SyncFn?, childrenMode: @escaping WantsFn, readback: @escaping ReadbackFn, child: @escaping ChildFn, childrenCount: @escaping CountFn, placement: @escaping PlacementFn, shader: ShaderFn?, validateShader: ShaderFn?, clearShaders: ClearShadersFn?, errorLen: @escaping ErrorFn, errorPtr: @escaping ErrorPtrFn, wantsInput: WantsFn?, input: BindFn?, messages: WantsFn?, published: WantsFn?, agent: BindFn?, outPtr: ErrorPtrFn?) {
        self.wantsInput = wantsInput; self.input = input; self.messages = messages; self.published = published; self.agent = agent; self.outPtr = outPtr
        self.create = create; self.bind = bind; self.render = render; self.dirty = dirty; self.destroy = destroy; self.texture = texture; self.textureMetal = textureMetal; self.sync = sync; self.childrenMode = childrenMode; self.readback = readback
        self.childView = child; self.childrenCount = childrenCount; self.placement = placement; self.shader = shader; self.validateShader = validateShader; self.clearShaders = clearShaders; self.errorLen = errorLen; self.errorPtr = errorPtr
    }

    /// A starved canvas's drawable arrived: every session's starved canvases render.
    static func acquired() {
        for case .success(let module) in shared.values {
            for owner in module.canvases.allObjects { owner.renderStarved() }
        }
    }

    /// A loaded module validates candidate shaders without changing its registry.
    /// Runtime shader assets (LLP 1030 D8) are the primary's; a declared
    /// module compiles the WGSL it carries (LLP 1009 D6).
    static var loaded: GpuModule? { if case .success(let module)? = shared[""] { return module }; return nil }
    func accepts(_ sources: [String: Data]) -> Bool {
        guard let validateShader, clearShaders != nil else { return false }
        for (name, text) in sources {
            let bytes = Array(name.utf8)
            let result = bytes.withUnsafeBufferPointer { n in text.withUnsafeBytes { t in validateShader(n.baseAddress, bytes.count, t.bindMemory(to: UInt8.self).baseAddress, text.count) } }
            if result != 0 { fputs("exact gpu: \(error())\n", stderr); return false }
        }
        return true
    }
    /// Called after acceptance, on the same thread, before new surfaces exist.
    func replaceShaders(_ sources: [String: Data]) {
        clearShaders?()
        for (name, text) in sources { _ = register(shader: name, text: text) }
    }
    func registerShaders(resolver: AssetResolver) {
        let sources = resolver.shaderSources()
        guard resolver.refusal == nil, accepts(sources) else { return }
        replaceShaders(sources)
    }

    /// One shader's text into the module (LLP 1030 D8): validated, its
    /// interface checked against the one the module binds; false, with the
    /// reason at `error()`, when refused.
    func register(shader name: String, text: Data) -> Bool {
        guard let shader else { return false }
        let bytes = Array(name.utf8)
        let r = bytes.withUnsafeBufferPointer { n in text.withUnsafeBytes { t in shader(n.baseAddress, bytes.count, t.bindMemory(to: UInt8.self).baseAddress, text.count) } }
        return r == 0
    }

    /// Copy before any other module call can reuse its output buffer.
    func output(_ length: UInt32) -> Data? {
        guard length > 0, let p = outPtr?() else { return nil }
        return Data(bytes: p, count: Int(length))
    }

    func error() -> String {
        let n = Int(errorLen())
        guard n > 0, let p = errorPtr() else { return "" }
        return String(decoding: UnsafeBufferPointer(start: p, count: n), as: UTF8.self)
    }
}

/// Display-link cadence policy shared by AppKit and UIKit.
struct DisplayPeriod {
    private static let rates: [Double] = [10, 12, 15, 16, 20, 24, 30, 40, 48, 60, 80, 120]
    private(set) var value = 0.0
    private var candidate = 0.0
    private var samples = 0
    private var maximumHeld = 0.0
    private var lower = 0.0, upper = Double.infinity
    mutating func publish(_ ms: Double, maximum: Double, send: (Double) -> Void) {
        // gpu_period belongs to the shared module, not this session. Even zero
        // must replace another session's known cadence before this one's render.
        defer { send(value) }
        guard ms.isFinite, ms > 0, maximum.isFinite, maximum > 0 else { return }
        if value > 0, maximum == maximumHeld, ms >= lower, ms <= upper {
            candidate = 0; samples = 0
            return
        }
        var next = 1000 / maximum
        func consider(_ period: Double) { if abs(period - ms) < abs(next - ms) { next = period } }
        for rate in Self.rates where rate <= maximum { consider(1000 / rate) }
        for divisor in 1...12 { consider(1000 * Double(divisor) / maximum) }
        // Cross the midpoint by 1% of the held class before considering a change.
        guard next != value,
              value == 0 || abs(ms - next) + value * 0.01 < abs(ms - value) else {
            candidate = 0; samples = 0
            return
        }
        if next == candidate { samples += 1 } else { candidate = next; samples = 1 }
        // The first class needs the same persistence as subsequent classes.
        if samples >= 3 {
            value = next; candidate = 0; samples = 0
            maximumHeld = maximum; lower = 0; upper = .infinity
            func boundary(_ period: Double) {
                if period < value { lower = max(lower, (period + value) / 2 - value * 0.005) }
                if period > value { upper = min(upper, (period + value) / 2 + value * 0.005) }
            }
            for rate in Self.rates where rate <= maximum { boundary(1000 / rate) }
            for divisor in 1...12 { boundary(1000 * Double(divisor) / maximum) }
        }
    }
}

/// The module's callback on the thread that acquired a starved canvas's
/// drawable (gpu/src/acquire.rs): the render belongs on the main thread.
/// Drawables that land before the main thread gets to the first are served
/// by that one pass: one render walk and one flush for them all (LLP 1009 D7).
private let gpuAcquired: GpuModule.AcquiredFn = {
    guard AcquiredPass.schedule() else { return }
    DispatchQueue.main.async {
        AcquiredPass.begin()
        GpuModule.acquired()
    }
}

/// Whether a starved-canvas pass is already queued on the main thread.
private enum AcquiredPass {
    nonisolated(unsafe) static var queued = false
    static let lock = NSLock()
    /// True when this call queued the pass.
    static func schedule() -> Bool {
        lock.lock(); defer { lock.unlock() }
        if queued { return false }
        queued = true
        return true
    }
    static func begin() {
        lock.lock(); queued = false; lock.unlock()
    }
}

