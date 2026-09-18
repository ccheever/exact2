// The GPU module's C ABI, loaded (LLP 1009 D2): the app's `<app>-gpu`
// dylib, `dlopen`ed by a presenter the first time a canvas is on screen —
// after the first painted frame. Shared by the AppKit and UIKit presenters;
// what each does with a surface (its `Canvases`) is its own.
import Foundation
import CryptoKit
import CExact

/// Why the module could not be loaded.
struct GpuLoadError: Error { let message: String }

/// The dylib's C ABI (gpu/src/native.rs). Loaded once per process (LLP 1031
/// D12): `gpu_load` creates the device and the module's instance table, and
/// a second load would replace them under the first session's surfaces.
final class GpuModule {
    nonisolated(unsafe) private static var shared: Result<GpuModule, GpuLoadError>?

    /// The process's one module: loaded on the first ask, the same answer after.
    static func loadShared(path: String) -> Result<GpuModule, GpuLoadError> {
        if let shared { return shared }
        let r = load(path: path)
        shared = r
        return r
    }

    static var bakedCompatibility: [String: Any] {
        let runtime = Runtime()
        defer { runtime.destroy() }
        let length = exact_baked_compat(runtime.rt)
        let bytes = Data(bytes: exact_out(runtime.rt), count: Int(length))
        return (try? JSONSerialization.jsonObject(with: bytes) as? [String: Any]) ?? [:]
    }
    static func modulePath(defaultPath: String, compat: [String: Any], environment: [String: String]) -> String {
        let gpu = (compat["embedded"] as? [String: Any])?["gpu"] as? [String: Any]
        return gpu?["trust"] as? String == "development" ? environment["EXACT_GPU_DYLIB"] ?? defaultPath : defaultPath
    }
    static func verify(path: String, compat: [String: Any]) -> GpuLoadError? {
        func refusal(_ reason: String) -> GpuLoadError { GpuLoadError(message:"GPU module \(path): \(reason)") }
        guard let card = (compat["embedded"] as? [String: Any])?["gpu"] as? [String: Any] else {
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
    typealias RenderFn = @convention(c) (UInt32, Float, Float, Float, Double) -> UInt32
    typealias DirtyFn = @convention(c) (UInt32) -> UInt32
    typealias DestroyFn = @convention(c) (UInt32) -> Void
    typealias TextureFn = @convention(c) (UInt32, UInt32, UInt32, UnsafePointer<UInt8>?, Int) -> UInt32
    typealias TextureMetalFn = @convention(c) (UInt32, UInt32, UInt32, UnsafeMutableRawPointer?) -> UInt32
    typealias SyncFn = @convention(c) () -> UInt32
    typealias WantsFn = @convention(c) (UInt32) -> UInt32
    typealias ReadbackFn = @convention(c) (UInt32, Float, Float, Float, Double, UnsafeMutablePointer<UInt8>?, Int) -> UInt32
    typealias ChildFn = @convention(c) (UInt32, UInt32, Float, Float, Float, Float, UInt32, UInt32, UnsafePointer<UInt8>?, Int) -> UInt32
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
    var seekable: SeekableFn?
    typealias PeriodFn = @convention(c) (Double) -> Void

    let wantsInput: WantsFn?
    let input: BindFn?
    typealias RestoreFn = @convention(c) (UInt32, UnsafePointer<UInt8>?, Int) -> Bool
    typealias AssetFn = @convention(c) (UInt32, UnsafePointer<UInt8>?, Int, UnsafePointer<UInt8>?, Int) -> Bool
    var assets: WantsFn?
    var asset: AssetFn?
    var carry: WantsFn?
    var restore: RestoreFn?
    let published: WantsFn?
    let messages: WantsFn?
    let agent: BindFn?
    private let outPtr: ErrorPtrFn?

    let create: CreateFn
    let bind: BindFn
    let render: RenderFn
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
    let wantsChildren: WantsFn
    /// A canvas's picture as pixels (LLP 1014, nested canvases).
    let readback: ReadbackFn
    /// Each child as its own texture, and where the surface put it (LLP 1014 D5).
    let wantsChildrenEach: WantsFn
    let child: ChildFn
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
    static func load(path: String) -> Result<GpuModule, GpuLoadError> {
        if let error = verify(path:path, compat:bakedCompatibility) { return .failure(error) }
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
              let readback = sym("gpu_readback", ReadbackFn.self),
              let wantsChildrenEach = sym("gpu_wants_children_each", WantsFn.self), let child = sym("gpu_child", ChildFn.self),
              let childrenCount = sym("gpu_children_count", CountFn.self), let placement = sym("gpu_placement", PlacementFn.self),
              let errorLen = sym("gpu_error", ErrorFn.self), let errorPtr = sym("gpu_error_ptr", ErrorPtrFn.self) else {
            return .failure(GpuLoadError(message: "\(path) is not an exact GPU module (missing exports)"))
        }
        let module = GpuModule(create: create, bind: bind, render: render, dirty: dirty, destroy: destroy, texture: texture, textureMetal: sym("gpu_texture_metal", TextureMetalFn.self), sync: sym("gpu_sync", SyncFn.self), wantsChildren: wantsChildren, readback: readback, wantsChildrenEach: wantsChildrenEach, child: child, childrenCount: childrenCount, placement: placement, shader: sym("gpu_shader", ShaderFn.self), validateShader: sym("gpu_shader_validate", ShaderFn.self), clearShaders: sym("gpu_shaders_clear", ClearShadersFn.self), errorLen: errorLen, errorPtr: errorPtr, wantsInput: sym("gpu_wants_input", WantsFn.self), input: sym("gpu_input", BindFn.self), messages: sym("gpu_messages", WantsFn.self), published: sym("gpu_published", WantsFn.self), agent: sym("gpu_agent", BindFn.self), outPtr: sym("gpu_out_ptr", ErrorPtrFn.self))
        if load() != 0 { return .failure(GpuLoadError(message: "gpu_load: \(module.error())")) }
        module.lifecycle = sym("gpu_lifecycle", LifecycleFn.self)
        module.period = sym("gpu_period", PeriodFn.self)
        module.assets = sym("gpu_assets", WantsFn.self); module.asset = sym("gpu_asset", AssetFn.self)
        module.carry = sym("gpu_carry", WantsFn.self); module.restore = sym("gpu_restore", RestoreFn.self)
        module.seekable = sym("gpu_seekable", SeekableFn.self)
        return .success(module)
    }

    private init(create: @escaping CreateFn, bind: @escaping BindFn, render: @escaping RenderFn, dirty: @escaping DirtyFn, destroy: @escaping DestroyFn, texture: @escaping TextureFn, textureMetal: TextureMetalFn?, sync: SyncFn?, wantsChildren: @escaping WantsFn, readback: @escaping ReadbackFn, wantsChildrenEach: @escaping WantsFn, child: @escaping ChildFn, childrenCount: @escaping CountFn, placement: @escaping PlacementFn, shader: ShaderFn?, validateShader: ShaderFn?, clearShaders: ClearShadersFn?, errorLen: @escaping ErrorFn, errorPtr: @escaping ErrorPtrFn, wantsInput: WantsFn?, input: BindFn?, messages: WantsFn?, published: WantsFn?, agent: BindFn?, outPtr: ErrorPtrFn?) {
        self.wantsInput = wantsInput; self.input = input; self.messages = messages; self.published = published; self.agent = agent; self.outPtr = outPtr
        self.create = create; self.bind = bind; self.render = render; self.dirty = dirty; self.destroy = destroy; self.texture = texture; self.textureMetal = textureMetal; self.sync = sync; self.wantsChildren = wantsChildren; self.readback = readback
        self.wantsChildrenEach = wantsChildrenEach; self.child = child; self.childrenCount = childrenCount; self.placement = placement; self.shader = shader; self.validateShader = validateShader; self.clearShaders = clearShaders; self.errorLen = errorLen; self.errorPtr = errorPtr
    }

    /// A loaded module validates candidate shaders without changing its registry.
    static var loaded: GpuModule? { if case .success(let module)? = shared { return module }; return nil }
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
