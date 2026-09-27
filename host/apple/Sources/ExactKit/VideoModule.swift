// @ref LLP 1042. AVKit belongs to an on-demand artifact, never ExactKit's link graph.
import Foundation
#if os(macOS)
import AppKit
typealias MediaPlatformView = NSView
#else
import UIKit
typealias MediaPlatformView = UIView
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
        let path = Bundle.main.executableURL!.deletingLastPathComponent().path + "/libexact_video.dylib"
        #else
        let path = embeddedModule(framework: "ExactVideo", dylib: "libexact_video.dylib")
        #endif
        guard let library = dlopen(path, RTLD_NOW | RTLD_LOCAL) else {
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
    private var visibilityBlocked = false
    private var visibilityThreshold: CGFloat? {
        guard let owner, owner.props["paused"] != nil,
              let raw = owner.props["playbackVisibilityThreshold"],
              let value = Double(raw), value.isFinite, (0...1).contains(value) else { return nil }
        return CGFloat(value)
    }
    func refreshVisibility() {
        guard let threshold = visibilityThreshold, let owner else { return }
        let ratio = VideoVisibilityHost.fraction(owner)
        let blocked = ratio <= 0 || ratio < threshold
        if blocked != visibilityBlocked { update() }
    }

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
        owner?.presenter?.videoVisibility?.remove(self)
        guard let handle else { return }
        self.handle = nil
        VideoModule.shared?.destroy(handle)
        platformView?.removeFromSuperview()
        platformView = nil
    }
    func layout() {
        guard let owner else { return }
        platformView?.frame = owner.contentBox()
        let radius = owner.number("border_radius", owner.number("border_radius_top_left"))
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
        if let threshold = visibilityThreshold {
            if owner.presenter?.videoVisibility == nil { owner.presenter?.videoVisibility = VideoVisibilityHost() }
            owner.presenter?.videoVisibility?.track(self)
            let ratio = VideoVisibilityHost.fraction(owner)
            visibilityBlocked = ratio <= 0 || ratio < threshold
            if visibilityBlocked { props["paused"] = "true" }
        } else {
            visibilityBlocked = false
            owner.presenter?.videoVisibility?.remove(self)
        }
        props["objectFit"] = owner.style["object_fit"]?.string ?? "contain"
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
        var result = observed
        if visibilityThreshold != nil, let owner {
            result["intersectionRatio"] = VideoVisibilityHost.fraction(owner)
            result["visibilityPaused"] = visibilityBlocked
        }
        return result
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


/// Optional media policy. Scroll/layout notifications coalesce without app actions
/// or a frame clock; only a threshold crossing changes the player's paused request.
final class VideoVisibilityHost {
    private final class WeakVideo {
        weak var value: VideoView?
        init(_ value: VideoView) { self.value = value }
    }
    private var videos: [ObjectIdentifier: WeakVideo] = [:]
    private var queued = false
    func track(_ video: VideoView) {
        let key = ObjectIdentifier(video)
        if videos[key] == nil { videos[key] = WeakVideo(video) }
        changed()
    }
    func remove(_ video: VideoView) { videos.removeValue(forKey: ObjectIdentifier(video)) }
    func reset() { videos.removeAll() }
    func changed() {
        AnimatedRasters.shared.poke()
        guard !videos.isEmpty, !queued else { return }
        queued = true
        DispatchQueue.main.async { [weak self] in
            guard let self else { return }
            self.queued = false
            for (key, entry) in self.videos {
                if let video = entry.value { video.refreshVisibility() }
                else { self.videos.removeValue(forKey: key) }
            }
        }
    }
    /// Rectangular intersection, as IntersectionObserver without trackVisibility:
    /// ancestor clipping and the viewport count; sibling occlusion/opacity do not.
    static func fraction(_ view: MediaPlatformView) -> CGFloat {
        guard let window = view.window, view.bounds.width > 0, view.bounds.height > 0 else { return 0 }
        #if os(macOS)
        guard let root = window.contentView else { return 0 }
        let box = view.convert(view.bounds, to: root)
        var clipped = box.intersection(root.bounds)
        var ancestor: NSView? = view
        while let current = ancestor {
            if current.isHidden { return 0 }
            if current !== view && (current is NSClipView || current.clipsToBounds || current.layer?.masksToBounds == true) {
                clipped = clipped.intersection(current.convert(current.bounds, to: root))
            }
            ancestor = current.superview
        }
        #else
        let box = view.convert(view.bounds, to: window)
        var clipped = box.intersection(window.bounds)
        var ancestor: UIView? = view
        while let current = ancestor {
            if current.isHidden { return 0 }
            if current !== view && current.clipsToBounds {
                clipped = clipped.intersection(current.convert(current.bounds, to: window))
            }
            ancestor = current.superview
        }
        #endif
        guard !clipped.isNull, box.width > 0, box.height > 0 else { return 0 }
        return min(1, max(0, clipped.width * clipped.height / (box.width * box.height)))
    }
}

#if !os(macOS)
/// An optional module in the app's Frameworks: wrapped as `<Name>.framework`
/// when the bundle is built for distribution (the App Store refuses loose
/// dylibs, ITMS-90171), otherwise the loose `lib….dylib` a development build
/// places there.
func embeddedModule(framework: String, dylib: String) -> String {
    let directory = Bundle.main.privateFrameworksPath ?? Bundle.main.bundlePath
    let wrapped = directory + "/" + framework + ".framework/" + framework
    return FileManager.default.fileExists(atPath: wrapped) ? wrapped : directory + "/" + dylib
}
#endif
