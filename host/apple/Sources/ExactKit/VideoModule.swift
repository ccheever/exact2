// @ref LLP 1042. AVKit belongs to an on-demand artifact, never ExactKit's link graph.
import Foundation
#if os(macOS)
import AppKit
private typealias MediaPlatformView = NSView
#else
import UIKit
private typealias MediaPlatformView = UIView
#endif
private typealias MediaCallback = @convention(c) (UnsafeMutableRawPointer?, UnsafePointer<UInt8>?, Int) -> Void
private final class VideoModule {
    typealias Create = @convention(c) (UnsafeMutableRawPointer?, MediaCallback?) -> UnsafeMutableRawPointer?
    typealias View = @convention(c) (UnsafeMutableRawPointer?) -> UnsafeMutableRawPointer?
    typealias Update = @convention(c) (UnsafeMutableRawPointer?, UnsafePointer<UInt8>?, Int) -> Void
    typealias Handle = @convention(c) (UnsafeMutableRawPointer?) -> Void
    let create: Create, view: View, update: Update, destroy: Handle, state: Handle
    private init(_ library: UnsafeMutableRawPointer) {
        func symbol<T>(_ name: String, _: T.Type) -> T { unsafeBitCast(dlsym(library, name)!, to: T.self) }
        create = symbol("exact_video_create", Create.self)
        view = symbol("exact_video_view", View.self)
        update = symbol("exact_video_update", Update.self)
        destroy = symbol("exact_video_destroy", Handle.self)
        state = symbol("exact_video_state", Handle.self)
    }
    static let shared: VideoModule? = {
        #if os(macOS)
        let directory = Bundle.main.executableURL!.deletingLastPathComponent().path
        #else
        let directory = Bundle.main.privateFrameworksPath ?? Bundle.main.bundlePath
        #endif
        guard let library = dlopen(directory + "/libexact_video.dylib", RTLD_NOW | RTLD_LOCAL) else {
            FileHandle.standardError.write(Data("exact video: \(String(cString: dlerror()))\n".utf8)); return nil
        }
        let exports = ["create", "view", "update", "destroy", "state"]
        guard exports.allSatisfy({ dlsym(library, "exact_video_" + $0) != nil }) else {
            dlclose(library); return nil
        }
        // Swift classes in a loaded image must remain mapped for the process lifetime.
        return VideoModule(library)
    }()
}

final class VideoView {
    weak var owner: NodeView?
    private var handle: UnsafeMutableRawPointer?
    private var platformView: MediaPlatformView?
    private var last: [String: String] = [:]
    private var observed: [String: Any] = ["unavailable": true]
    private var intrinsicSize: CGSize?

    init(owner: NodeView) {
        self.owner = owner
        guard let module = VideoModule.shared else { return }
        let context = Unmanaged.passUnretained(self).toOpaque()
        handle = module.create(context, { context, bytes, length in
            guard let context, let bytes else { return }
            let video = Unmanaged<VideoView>.fromOpaque(context).takeUnretainedValue()
            guard let message = try? JSONSerialization.jsonObject(with: Data(bytes: bytes, count: length)) as? [String: Any] else { return }
            video.receive(message)
        })
        if let handle, let raw = module.view(handle) {
            let view = Unmanaged<MediaPlatformView>.fromOpaque(raw).takeUnretainedValue()
            platformView = view
            owner.addSubview(view)
        }
    }
    deinit { invalidate() }
    func invalidate() {
        guard let handle else { return }
        self.handle = nil
        VideoModule.shared?.destroy(handle)
        platformView?.removeFromSuperview()
        platformView = nil
    }
    func layout() {
        guard let owner else { return }
        platformView?.frame = owner.contentBox()
        let radius = owner.number("border_radius")
        #if os(macOS)
        platformView?.wantsLayer = true
        platformView?.layer?.cornerRadius = radius
        platformView?.layer?.masksToBounds = true
        #else
        platformView?.layer.cornerRadius = radius
        platformView?.layer.masksToBounds = true
        #endif
    }
    func update() {
        guard let owner, let module = VideoModule.shared, let handle else { return }
        var props = owner.props
        props["objectFit"] = owner.style["object_fit"] as? String ?? "contain"
        for name in ["src", "poster"] {
            if let source = props[name], !source.isEmpty {
                let resolved = NodeView.resolveSource(source, app: owner.presenter?.session?.app)
                props[name] = resolved?.absoluteString ?? ""
                if name == "src" && resolved == nil { props["sourceError"] = "Unsupported media source" }
            }
        }
        guard props != last else { return }
        last = props
        guard let data = try? JSONSerialization.data(withJSONObject: props) else { return }
        data.withUnsafeBytes { module.update(handle, $0.bindMemory(to: UInt8.self).baseAddress, data.count) }
        layout()
    }
    func state() -> [String: Any] {
        if let handle { VideoModule.shared?.state(handle) }
        return observed
    }
    private func receive(_ message: [String: Any]) {
        if let state = message["state"] as? [String: Any] { observed = state }
        guard let owner else { return }
        let w = observed["videoWidth"] as? Double ?? 0, h = observed["videoHeight"] as? Double ?? 0
        let size: CGSize? = w > 0 && h > 0 ? CGSize(width: w, height: h) : nil
        if size != intrinsicSize {
            intrinsicSize = size
            DispatchQueue.main.async { [weak self, weak owner] in
                guard let self, let owner, owner.video === self else { return }
                owner.presenter?.intrinsic(owner.id, size)
            }
        }
        guard let event = message["event"] as? String, owner.handlers.contains(event) else { return }
        let payload = message["payload"] as? String ?? ""
        DispatchQueue.main.async { [weak self, weak owner] in
            guard let self, let owner, owner.video === self, let session = owner.presenter?.session else { return }
            session.apply(session.runtime.media(owner.id, event: event, payload: payload, now: session.now()))
        }
    }
}
