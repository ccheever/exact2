// The GPU module's C ABI, loaded (LLP 1009 D2): the app's `<app>-gpu`
// dylib, `dlopen`ed by a presenter the first time a canvas is on screen —
// after the first painted frame. Shared by the AppKit and UIKit presenters;
// what each does with a surface (its `Canvases`) is its own.
import Foundation

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
    typealias TextureMetalFn = @convention(c) (UInt32, UInt32, UInt32, UnsafeMutableRawPointer?) -> UInt32
    typealias SyncFn = @convention(c) () -> UInt32
    typealias WantsFn = @convention(c) (UInt32) -> UInt32
    typealias ReadbackFn = @convention(c) (UInt32, Float, Float, Float, Double, UnsafeMutablePointer<UInt8>?, Int) -> UInt32
    typealias ChildFn = @convention(c) (UInt32, UInt32, Float, Float, Float, Float, UInt32, UInt32, UnsafePointer<UInt8>?, Int) -> UInt32
    typealias CountFn = @convention(c) (UInt32, UInt32) -> UInt32
    typealias PlacementFn = @convention(c) (UInt32, UInt32, UnsafeMutablePointer<Float>?, Int) -> UInt32
    typealias ErrorFn = @convention(c) () -> UInt32
    typealias ErrorPtrFn = @convention(c) () -> UnsafePointer<UInt8>?

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
              let readback = sym("gpu_readback", ReadbackFn.self),
              let wantsChildrenEach = sym("gpu_wants_children_each", WantsFn.self), let child = sym("gpu_child", ChildFn.self),
              let childrenCount = sym("gpu_children_count", CountFn.self), let placement = sym("gpu_placement", PlacementFn.self),
              let errorLen = sym("gpu_error", ErrorFn.self), let errorPtr = sym("gpu_error_ptr", ErrorPtrFn.self) else {
            return .failure(GpuLoadError(message: "\(path) is not an exact GPU module (missing exports)"))
        }
        let module = GpuModule(create: create, bind: bind, render: render, dirty: dirty, destroy: destroy, texture: texture, textureMetal: sym("gpu_texture_metal", TextureMetalFn.self), sync: sym("gpu_sync", SyncFn.self), wantsChildren: wantsChildren, readback: readback, wantsChildrenEach: wantsChildrenEach, child: child, childrenCount: childrenCount, placement: placement, errorLen: errorLen, errorPtr: errorPtr)
        if load() != 0 { return .failure(GpuLoadError(message: "gpu_load: \(module.error())")) }
        return .success(module)
    }

    private init(create: @escaping CreateFn, bind: @escaping BindFn, render: @escaping RenderFn, dirty: @escaping DirtyFn, destroy: @escaping DestroyFn, texture: @escaping TextureFn, textureMetal: TextureMetalFn?, sync: SyncFn?, wantsChildren: @escaping WantsFn, readback: @escaping ReadbackFn, wantsChildrenEach: @escaping WantsFn, child: @escaping ChildFn, childrenCount: @escaping CountFn, placement: @escaping PlacementFn, errorLen: @escaping ErrorFn, errorPtr: @escaping ErrorPtrFn) {
        self.create = create; self.bind = bind; self.render = render; self.dirty = dirty; self.destroy = destroy; self.texture = texture; self.textureMetal = textureMetal; self.sync = sync; self.wantsChildren = wantsChildren; self.readback = readback
        self.wantsChildrenEach = wantsChildrenEach; self.child = child; self.childrenCount = childrenCount; self.placement = placement; self.errorLen = errorLen; self.errorPtr = errorPtr
    }

    func error() -> String {
        let n = Int(errorLen())
        guard n > 0, let p = errorPtr() else { return "" }
        return String(decoding: UnsafeBufferPointer(start: p, count: n), as: UTF8.self)
    }
}
