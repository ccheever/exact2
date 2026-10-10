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
    let create: Create, view: View, update: Update, destroy: Handle, state: Handle, fullscreen: Handle, toggle: Handle
    private init(_ library: UnsafeMutableRawPointer) {
        func symbol<T>(_ name: String, _: T.Type) -> T { unsafeBitCast(dlsym(library, name)!, to: T.self) }
        create = symbol("exact_video_create", Create.self)
        view = symbol("exact_video_view", View.self)
        update = symbol("exact_video_update", Update.self)
        destroy = symbol("exact_video_destroy", Handle.self)
        state = symbol("exact_video_state", Handle.self)
        fullscreen = symbol("exact_video_fullscreen", Handle.self)
        toggle = symbol("exact_video_toggle", Handle.self)
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
        let exports = ["create", "view", "update", "destroy", "state", "fullscreen", "toggle"]
        guard exports.allSatisfy({ dlsym(library, "exact_video_" + $0) != nil }) else {
            dlclose(library); return nil
        }
        // Swift classes in a loaded image must remain mapped for the process lifetime.
        return VideoModule(library)
    }()
}

/// Chrome's rule for a muted video its `autoplay` attribute started, with
/// `paused` unbound (LLP 1042 §3, ruled 2026-09-28): it plays only while some
/// of it is in the viewport, and one that loads off screen starts when first
/// seen. A pause or play the rule did not ask for (native controls, the end
/// of a video without `loop`) ends the rule, as HTML's pause() and play()
/// clear the element's can-autoplay flag. Measured in Chrome 154.
struct OffscreenAutoplay {
    /// The playback is still the autoplay attribute's.
    private(set) var armed = false
    /// Paused by this rule.
    private(set) var holding = false
    /// The pauses (true) and plays the rule asked for and has not yet seen.
    private var expected: [Bool] = []
    /// A new source: armed when the autoplay attribute starts it.
    mutating func load(autoplay: Bool) { armed = autoplay; holding = false; expected = [] }
    mutating func disarm() { armed = false; holding = false; expected = [] }
    /// Whether the rule changes its request for this visibility: true to
    /// pause, false to resume, nil for no change.
    mutating func visible(_ visible: Bool) -> Bool? {
        guard armed, holding == visible else { return nil }
        holding = !visible
        expected.append(holding)
        if expected.count > 4 { expected.removeFirst() }
        return holding
    }
    /// The engine reported a pause (true) or a play (false).
    mutating func observed(paused: Bool) {
        guard armed else { return }
        if let i = expected.firstIndex(of: paused) { expected.removeFirst(i + 1); return }
        if paused != holding { disarm() }
    }
}

final class VideoView {
    weak var owner: NodeView?
    private var handle: UnsafeMutableRawPointer?
    private var platformView: MediaPlatformView?
    private var last: [String: String] = [:]
    private var observed: [String: Any] = ["unavailable": true]
    private var intrinsicSize: CGSize?
    private var visibilityBlocked = false
    private var autoplay = OffscreenAutoplay()
    private var autoplaySource: String?
    /// Whether its full-screen player shows (the arm's `fullscreen` state).
    private(set) var isFullscreen = false
    /// The rule applies: armed, muted, `paused` unbound, a `video` (an
    /// `audio` is never seen, so Chrome never holds it; LLP 1042 §8).
    private var autoplayRule: Bool {
        guard let owner else { return false }
        return autoplay.armed && owner.props["paused"] == nil && owner.props["muted"] == "true" && owner.props["semanticTag"] != "audio"
    }
    /// The last source resolved per name (`src`, `poster`): its authored text
    /// and what it resolved to. A resolution reads the file system.
    private var resolved: [String: (source: String, url: URL?)] = [:]
    /// `fastSeek` and `load` (podcast F8, F18), each a numbered request the
    /// arm runs once: a seek every time, even to the time it last sought.
    private var commands: (seek: Int, seconds: Double, load: Int) = (0, 0, 0)
    /// The media events the arm reports (LLP 1042 §3), and the media
    /// session's six (LLP 1098 D2), and fullscreenchange; others are not sent.
    static let events: Set<String> = ["loadedmetadata", "canplay", "play", "playing", "pause", "ended", "waiting", "seeking", "seeked", "ratechange", "volumechange", "timeupdate", "durationchange", "error", "fullscreenchange", "seekbackward", "seekforward", "seekto", "previoustrack", "nexttrack", "stop"]
    /// A remote play's latch over the visibility threshold (LLP 1098 D3):
    /// it holds across the `paused` bound when it was set, the app's stale
    /// `true` included, until a later commit writes `true` or the element
    /// rises above the threshold.
    private var latched = false
    private var latchedFrom: String?
    /// The arm's media session report (NowPlaying.swift), and the driver's
    /// last `tap … mediasession` request and answer (LLP 1098 D10).
    private(set) var sessionReport: [String: Any] = [:]
    private var remoteRequests = 0
    private var remoteRequest: String?
    private var remoteResult: (n: String, status: String)?
    private var visibilityThreshold: CGFloat? {
        guard let owner, owner.props["paused"] != nil,
              let raw = owner.props["playbackVisibilityThreshold"],
              let value = Double(raw), value.isFinite, (0...1).contains(value) else { return nil }
        return CGFloat(value)
    }
    func refreshVisibility() {
        if visibilityThreshold == nil, autoplayRule, let owner {
            if autoplay.holding == (VideoVisibilityHost.fraction(owner) > 0) { update() }
            return
        }
        guard let threshold = visibilityThreshold, let owner else { return }
        let ratio = VideoVisibilityHost.fraction(owner)
        let blocked = ratio <= 0 || ratio < threshold
        if blocked != visibilityBlocked { update() }
    }

    #if os(iOS) || os(tvOS)
    /// Whether it holds the app's audio session: while it has sound (a
    /// source, not muted), LLP 1096 D8.
    private var holdsSession = false
    private func holdSession(_ audible: Bool) {
        guard audible != holdsSession else { return }
        if audible {
            // Held even when activation fails: the session activates it when
            // the interruption ends, and a mute still releases it.
            holdsSession = true
            do { try AudioSession.hold(ObjectIdentifier(self)) } catch { fputs("exact audio session: \(error)\n", stderr) }
        } else {
            holdsSession = false
            AudioSession.release(ObjectIdentifier(self))
        }
    }
    #endif
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
        #if os(iOS) || os(tvOS)
        if handle != nil, !ExactEnv.agentMode { AudioSession.playerCame(ObjectIdentifier(self)) }
        #endif
        if let handle, let raw = module.view(handle) {
            let view = Unmanaged<MediaPlatformView>.fromOpaque(raw).takeUnretainedValue()
            platformView = view
            owner.addSubview(view)
        }
    }
    deinit { invalidate() }
    func invalidate() {
        owner?.presenter?.videoVisibility?.remove(self)
        guard let handle else {
            #if os(iOS) || os(tvOS)
            holdSession(false)
            #endif
            return
        }
        self.handle = nil
        VideoModule.shared?.destroy(handle)
        platformView?.removeFromSuperview()
        platformView = nil
        // The player is stopped and its claim gone before the category moves.
        #if os(iOS) || os(tvOS)
        holdSession(false)
        AudioSession.playerWent(ObjectIdentifier(self))
        #endif
    }
    func layout() {
        guard let owner, let platformView else { return }
        Self.layout(platformView, in: owner)
    }
    /// CSS `visibility` hides the player's own view, not the node's children.
    func applyCssHidden(_ hidden: Bool) { platformView?.isHidden = hidden }
    static func layout(_ view: MediaPlatformView, in owner: NodeView) {
        let content = owner.contentBox()
        view.frame = content
        #if os(macOS)
        view.wantsLayer = true
        guard let layer = view.layer else { return }
        #else
        let layer = view.layer
        #endif
        let radii = BorderPaint.contentRadii(owner.cornerSizes(in: owner.bounds), outer: owner.bounds, inner: content)
        BorderPaint.clip(layer, in: CGRect(origin: .zero, size: content.size), radii: radii)
    }
    func update() {
        // Radius, borders and padding are style, not player props. Refresh
        // their geometry even when the AVKit update below is deduplicated.
        layout()
        guard let owner, let module = VideoModule.shared, let handle else { return }
        var props = owner.props
        if props["src"] != autoplaySource {
            autoplaySource = props["src"]
            autoplay.load(autoplay: props["autoplay"] == "true")
        }
        if props["paused"] != nil { autoplay.disarm() }
        if latched, props["paused"] != latchedFrom { if props["paused"] == "true" { latched = false } else { latchedFrom = props["paused"] } }
        if let threshold = visibilityThreshold {
            if owner.presenter?.videoVisibility == nil { owner.presenter?.videoVisibility = VideoVisibilityHost() }
            owner.presenter?.videoVisibility?.track(self)
            let ratio = VideoVisibilityHost.fraction(owner)
            visibilityBlocked = ratio <= 0 || ratio < threshold
            if !visibilityBlocked { latched = false } // above it the authored value applies anyway
            if visibilityBlocked && !latched { props["paused"] = "true" }
        } else if autoplayRule {
            visibilityBlocked = false
            if owner.presenter?.videoVisibility == nil { owner.presenter?.videoVisibility = VideoVisibilityHost() }
            owner.presenter?.videoVisibility?.track(self)
            // The request crosses once: "true" while held, "false" in the
            // update that resumes, then nothing (a removed `paused` is no command).
            if let hold = autoplay.visible(VideoVisibilityHost.fraction(owner) > 0) { props["paused"] = hold ? "true" : "false" }
            else if autoplay.holding { props["paused"] = "true" }
        } else {
            visibilityBlocked = false
            owner.presenter?.videoVisibility?.remove(self)
        }
        props["objectFit"] = owner.style["object_fit"]?.string ?? "contain"
        // @ref LLP 1100 D11
        props["dynamicRangeLimit"] = owner.style["dynamic_range_limit"]?.string ?? "no-limit"
        for name in ["src", "poster", "mediaArtwork"] {
            if let source = props[name], !source.isEmpty {
                let url: URL?
                if let hit = resolved[name], hit.source == source { url = hit.url } else {
                    // The app's own file (LLP 1069.002 D7), as an `image`
                    // shows one: a download its data module kept (podcast F19).
                    url = source.hasPrefix("app:/") ? AppFiles.url(source) : NodeView.resolveSource(source, app: owner.presenter?.session?.app)
                    resolved[name] = (source, url)
                }
                // The artwork keeps its authored source for `state` (LLP 1098 D1, D7).
                props[name == "mediaArtwork" ? "mediaArtworkURL" : name] = url?.absoluteString ?? ""
                if name == "src" && url == nil { props["sourceError"] = "Unsupported media source" }
            } else if name == "src", props[name] == "" {
                // HTML fails an empty `src` (its resource selection's
                // "failed with attribute"), as the web reports it.
                props["sourceError"] = "Empty src attribute"
            }
        }
        if commands.seek > 0 { props["exactSeek"] = "\(commands.seek) \(commands.seconds)" }
        if commands.load > 0 { props["exactLoad"] = String(commands.load) }
        // Under the driver the session is built and reported, never published (LLP 1098 D10).
        if ExactEnv.agentMode { props["exactPublish"] = "false" }
        if let remoteRequest { props["exactRemote"] = remoteRequest }
        var listeners = owner.handlers.intersection(Self.events)
        if autoplayRule { listeners.formUnion(["pause", "play"]) }
        if !listeners.isEmpty { props["exactListeners"] = listeners.sorted().joined(separator: " ") }
        // A video with sound, or a media-session claimant (Now Playing needs
        // a non-mixable category), holds the app's session; muted (applied to
        // the player first) or gone, it gives it back (LLP 1096 D8).
        #if os(iOS) || os(tvOS)
        let audible = !ExactEnv.agentMode && (props["mediaTitle"] != nil || (props["muted"] != "true" && props["src"]?.isEmpty == false))
        #endif
        guard props != last else { return }
        last = props
        #if os(iOS) || os(tvOS)
        if audible { holdSession(true) }
        #endif
        guard let data = try? JSONSerialization.data(withJSONObject: props) else { return }
        data.withUnsafeBytes { module.update(handle, $0.bindMemory(to: UInt8.self).baseAddress, data.count) }
        #if os(iOS) || os(tvOS)
        if !audible { holdSession(false) }
        #endif
    }
    /// `fastSeek(id, seconds)`, `load(id)` or `requestFullscreen(id)`, by HTML's method names.
    func command(_ name: String, seconds: Double) {
        if name == "requestFullscreen" { return requestFullscreen() }
        if name == "fastSeek" { commands.seek += 1; commands.seconds = seconds } else { commands.load += 1 }
        update()
    }
    /// `requestFullscreen`: the arm presents it.
    func requestFullscreen() {
        if let handle { VideoModule.shared?.fullscreen(handle) }
    }
    /// The remote's Play/Pause (tvOS's `PlayPauseKey`).
    func togglePlayPause() {
        if let handle { VideoModule.shared?.toggle(handle) }
    }
    func state() -> [String: Any] {
        if let handle { VideoModule.shared?.state(handle) }
        var result = observed
        result.removeValue(forKey: "session")
        if visibilityThreshold != nil, let owner {
            result["intersectionRatio"] = VideoVisibilityHost.fraction(owner)
            result["visibilityPaused"] = visibilityBlocked
        }
        if autoplayRule { result["autoplayOffscreenPaused"] = autoplay.holding }
        return result
    }
    /// The driver's `tap … mediasession` (LLP 1098 D10): the arm runs the
    /// command target's own path; its status, or nil with no arm.
    func remote(_ action: String, seconds: Double?) -> String? {
        guard handle != nil else { return nil }
        remoteRequests += 1
        remoteRequest = "\(remoteRequests) \(action)" + (seconds.map { " \($0)" } ?? "")
        remoteResult = nil
        update()
        return remoteResult.flatMap { $0.n == String(remoteRequests) ? $0.status : nil }
    }
    private func receive(_ message: [String: Any]) {
        if let state = message["state"] as? [String: Any] { observed = state; sessionReport = state["session"] as? [String: Any] ?? sessionReport }
        if let result = message["remoteResult"] as? [String: String] { remoteResult = (result["n"] ?? "", result["status"] ?? "") }
        guard let owner else { return }
        if message["remote"] as? String == "play" { latched = true; latchedFrom = owner.props["paused"] }
        let fullscreen = observed["fullscreen"] as? Bool ?? false
        if fullscreen != isFullscreen {
            isFullscreen = fullscreen
            #if os(tvOS)
            DispatchQueue.main.async { [weak owner] in owner?.presenter?.remoteKeysChanged() }
            #endif
        }
        let w = observed["videoWidth"] as? Double ?? 0, h = observed["videoHeight"] as? Double ?? 0
        let size: CGSize? = w > 0 && h > 0 ? CGSize(width: w, height: h) : nil
        if size != intrinsicSize {
            intrinsicSize = size
            DispatchQueue.main.async { [weak self, weak owner] in
                guard let self, let owner, owner.video === self else { return }
                owner.presenter?.intrinsic(owner.id, size)
            }
        }
        if let event = message["event"] as? String, event == "pause" || event == "play" {
            let was = autoplay.armed
            autoplay.observed(paused: event == "pause")
            if was && !autoplay.armed { DispatchQueue.main.async { [weak self] in self?.update() } }
        }
        guard let event = message["event"] as? String, owner.handlers.contains(event) else { return }
        let payload = message["payload"] as? String ?? ""
        DispatchQueue.main.async { [weak self, weak owner] in
            guard let self, let owner, owner.video === self, let session = owner.presenter?.session else { return }
            session.apply(session.runtime.media(owner.id, event: event, payload: payload, now: session.now()))
        }
    }
}


extension Presenter {
    /// `fastSeek(id, seconds)` and `load(id)` on the video or audio with that
    /// HTML `id`, as `focus(id)` names one.
    func mediaCommand(_ name: String, _ args: [Any]) {
        let id = args.first as? String ?? ""
        let node = views.values.sorted(by: { $0.id < $1.id }).first(where: { $0.props["id"] == id })
        guard let video = node?.video else {
            session?.log("\(name) \"\(id)\" refused: \(node == nil ? "no live node with that id" : "not a video or audio")")
            return
        }
        video.command(name, seconds: (args.count > 1 ? args[1] as? Double : nil) ?? .nan)
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
package func embeddedModule(framework: String, dylib: String) -> String {
    let directory = Bundle.main.privateFrameworksPath ?? Bundle.main.bundlePath
    let wrapped = directory + "/" + framework + ".framework/" + framework
    return FileManager.default.fileExists(atPath: wrapped) ? wrapped : directory + "/" + dylib
}
#endif
