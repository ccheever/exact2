// @ref LLP 1042. An AVPlayer and native presentation survive every layout/keyboard change.
// The media session (LLP 1098) is NowPlaying.swift's, compiled into this arm.
import Foundation
import AVFoundation
import AVKit
#if os(macOS)
import AppKit
private typealias PlatformView = NSView
private typealias PlatformImageView = NSImageView
#else
import UIKit
private typealias PlatformView = UIView
private typealias PlatformImageView = UIImageView
#endif
public typealias VideoCallback = @convention(c) (UnsafeMutableRawPointer?, UnsafePointer<UInt8>?, Int) -> Void

private final class VideoContainer: PlatformView {
    weak var arm: VideoArm?
    #if os(macOS)
    override var isFlipped: Bool { true }
    override func layout() { super.layout(); arm?.layout() }
    #else
    override func layoutSubviews() { super.layoutSubviews(); arm?.layout() }
    override func didMoveToWindow() { super.didMoveToWindow(); arm?.attach() }
    #endif
}

#if os(iOS) || os(tvOS)
private final class VideoLayerView: UIView {
    override class var layerClass: AnyClass { AVPlayerLayer.self }
    var playerLayer: AVPlayerLayer { layer as! AVPlayerLayer }
}
#endif

/// One parsed asset per unchanged local file (LLP 1042): a feed that repeats a
/// source makes each new item from it instead of opening and inspecting the
/// file again. A remote source is always its own asset; a failed item's asset
/// is dropped. The file's size and modification time are part of the key, so
/// a replaced file is a new asset.
private enum SharedAssets {
    static let cache: NSCache<NSString, AVURLAsset> = { let c = NSCache<NSString, AVURLAsset>(); c.countLimit = 16; return c }()
    static func key(_ url: URL) -> NSString? {
        guard url.isFileURL else { return nil }
        var info = stat()
        guard stat(url.path, &info) == 0 else { return nil }
        return "\(info.st_size) \(info.st_mtimespec.tv_sec).\(info.st_mtimespec.tv_nsec) \(url.path)" as NSString
    }
    static func asset(_ url: URL) -> AVURLAsset {
        guard let key = key(url) else { return AVURLAsset(url: url) }
        if let asset = cache.object(forKey: key) { return asset }
        let asset = AVURLAsset(url: url)
        cache.setObject(asset, forKey: key)
        return asset
    }
    static func forget(_ url: URL) { if let key = key(url) { cache.removeObject(forKey: key) } }
}

private final class VideoArm: NSObject, NowPlayingPlayer {
    let player = AVPlayer()
    let container = VideoContainer(frame: .zero)
    let poster = PlatformImageView(frame: .zero)
    #if os(macOS)
    let presentation = AVPlayerView(frame: .zero)
    #else
    var controller: AVPlayerViewController?
    var inline: VideoLayerView?
    /// The full-screen presentation `requestFullscreen` made, while it shows.
    var fullscreen: FullscreenPlayer?
    var presentation: UIView? { controller?.view ?? inline }
    #endif
    let context: UnsafeMutableRawPointer?
    let callback: VideoCallback
    var props: [String: String] = [:]
    var observations: [NSKeyValueObservation] = []
    var playerObservations: [NSKeyValueObservation] = []
    var notifications: [NSObjectProtocol] = []
    var tick: Any?
    var lastError: String?
    var invalidated = false
    var generation = 0
    var posterGeneration = 0
    var pendingSeek: Double?
    /// A seek in flight: HTML's official playback position, which `currentTime`
    /// reads at once, while AVPlayer still reports where it was (jukebox F20).
    var seekTarget: Double?
    var seeks = 0
    /// Whether the current item reached readyToPlay: a failure before it is
    /// HTML's "source not supported", one after it a network or decode error.
    var itemReady = false
    var naturalSize = CGSize.zero
    var wantsPlay = false
    /// The item played to its end (HTML's "ended playback"): the next play
    /// starts it over, as HTML's play() does (LLP 1042 §8: a sound effect
    /// replays by asking to play again).
    var atEnd = false
    var lastPaused = true
    var lastTimeStatus = AVPlayer.TimeControlStatus.paused
    /// The playback rate last reported by `ratechange` (HTML's playbackRate):
    /// starting and pausing change the player's rate, not this.
    var reportedRate: Float = 1
    /// The volume and muting last reported by `volumechange`: AVPlayer's KVO
    /// also fires when a value is set to what it was (the web's never does).
    var reportedVolume: (Float, Bool) = (1, false)
    /// The media events the node handles. Only these, and state snapshots,
    /// cross the ABI, as the web glue sends only handled events.
    var listeners: Set<String> = []
    /// AVKit's own Now Playing publication, off while a claimant owns the
    /// media session (LLP 1098 D7); a controller made later takes it.
    var avkitPublishes = true

    init(context: UnsafeMutableRawPointer?, callback: @escaping VideoCallback) {
        self.context = context; self.callback = callback
        super.init()
        container.arm = self
        #if os(macOS)
        container.wantsLayer = true; container.layer?.masksToBounds = true
        presentation.player = player
        poster.imageScaling = .scaleProportionallyUpOrDown
        #else
        container.clipsToBounds = true
        poster.contentMode = .scaleAspectFit
        poster.isUserInteractionEnabled = false
        #endif
        #if os(macOS)
        container.addSubview(presentation)
        #endif
        container.addSubview(poster)
        poster.isHidden = true
        playerObservations = [
            player.observe(\.timeControlStatus, options: [.new]) { [weak self] _, _ in self?.timeStatusChanged() },
            player.observe(\.rate, options: [.new]) { [weak self] _, _ in self?.rateChanged() },
            player.observe(\.volume, options: [.new]) { [weak self] _, _ in self?.volumeChanged() },
            player.observe(\.isMuted, options: [.new]) { [weak self] _, _ in self?.volumeChanged() }
        ]
        NowPlaying.shared.joined(self)
    }
    /// The periodic observer runs only while `timeupdate` is handled; `state`
    /// reads the time from the player when asked.
    func updateTick() {
        let wants = listeners.contains("timeupdate") && !invalidated
        if wants && tick == nil {
            tick = player.addPeriodicTimeObserver(forInterval: CMTime(seconds: 0.25, preferredTimescale: 600), queue: .main) { [weak self] _ in
                // A seek in flight reports its own time when it lands.
                guard let self, !self.invalidated, self.seekTarget == nil else { return }
                self.emit("timeupdate", payload: String(self.seconds))
            }
        } else if !wants, let tick {
            player.removeTimeObserver(tick); self.tick = nil
        }
    }
    func volumeChanged() {
        guard Thread.isMainThread else { DispatchQueue.main.async { [weak self] in self?.volumeChanged() }; return }
        let now = (player.volume, player.isMuted)
        guard !invalidated, now != reportedVolume else { return }
        reportedVolume = now
        emit("volumechange")
    }
    func rateChanged() {
        guard Thread.isMainThread else { DispatchQueue.main.async { [weak self] in self?.rateChanged() }; return }
        let rate = player.rate
        guard !invalidated, rate != 0, rate != reportedRate else { return }
        reportedRate = rate
        emit("ratechange")
    }
    var seconds: Double { if let seekTarget { return seekTarget }; let s = player.currentTime().seconds; return s.isFinite ? s : 0 }
    func bool(_ name: String, _ fallback: Bool = false) -> Bool { props[name].map { $0 == "true" } ?? fallback }
    func number(_ name: String, _ fallback: Double) -> Double { props[name].flatMap(Double.init) ?? fallback }
    var rate: Float { Float(number("playbackRate", 1)) }
    var renderer: String {
        #if os(macOS)
        return "AVKit"
        #else
        return controller != nil ? "AVKit" : inline != nil ? "AVPlayerLayer" : "AVPlayer"
        #endif
    }
    var snapshot: [String: Any] {
        let duration = player.currentItem?.duration.seconds ?? .nan
        return ["currentTime": seconds, "duration": duration.isFinite ? duration as Any : NSNull(),
                "paused": player.rate == 0, "muted": player.isMuted, "volume": player.volume,
                "playbackRate": player.rate, "readyState": player.currentItem?.status == .readyToPlay ? 4 : 0,
                "videoWidth": naturalSize.width, "videoHeight": naturalSize.height,
                "error": (lastError ?? player.currentItem?.error?.localizedDescription).map { $0 as Any } ?? NSNull(),
                "src": props["src"] ?? "", "renderer": renderer, "generation": generation,
                // @ref LLP 1100 D11
                "hdr": hdrItem, "eligibleForHDR": AVPlayer.eligibleForHDRPlayback,
                "dynamicRange": props["dynamicRangeLimit"] ?? "no-limit"]
    }
    func emit(_ event: String = "snapshot", payload: String = "") {
        guard !invalidated else { return }
        // KVO and AVFoundation completion delivery are serialized onto main before crossing the ABI.
        guard Thread.isMainThread else {
            DispatchQueue.main.async { [weak self] in self?.emit(event, payload: payload) }; return
        }
        if Self.moves.contains(event) { NowPlaying.shared.moved(self) }
        guard event == "snapshot" || listeners.contains(event) else { return }
        send(["event": event, "payload": payload])
    }
    /// One message to ExactKit, with the state snapshot and this player's media session.
    func send(_ message: [String: Any]) {
        var message = message
        var state = snapshot
        state["session"] = NowPlaying.shared.report(self)
        message["state"] = state
        guard let data = try? JSONSerialization.data(withJSONObject: message) else { return }
        data.withUnsafeBytes { callback(context, $0.bindMemory(to: UInt8.self).baseAddress, data.count) }
    }
    /// The reports that move the published position or state (LLP 1098 D4).
    static let moves: Set<String> = ["loadedmetadata", "durationchange", "play", "pause", "ratechange", "seeked", "ended"]

    // MARK: - the media session (LLP 1098; NowPlaying.swift)
    var sessionProps: [String: String] { props }
    var sessionPaused: Bool { player.timeControlStatus == .paused }
    /// HTML's duration: a live item's is infinite, an unknown one NaN.
    var sessionDuration: Double {
        guard let item = player.currentItem else { return .nan }
        return item.status == .readyToPlay && item.duration.isIndefinite ? .infinity : item.duration.seconds
    }
    var sessionElapsed: Double { seconds }
    var sessionRate: Double { Double(rate) }
    /// A remote play is a person's: ExactKit latches it over the visibility threshold (D3).
    func sessionPlay() {
        guard !invalidated else { return }
        send(["remote": "play"])
        wantsPlay = true
        if player.currentItem == nil { loadSource() }
        play()
    }
    func sessionPause() { guard !invalidated else { return }; wantsPlay = false; player.pause() }
    func sessionAction(_ name: String, payload: String) { emit(name, payload: payload) }
    func sessionAVKitPublishes(_ on: Bool) {
        avkitPublishes = on
        #if os(macOS)
        presentation.updatesNowPlayingInfoCenter = on
        #elseif os(iOS)
        controller?.updatesNowPlayingInfoCenter = on
        #endif
    }
    /// `error`'s payload is a stable code, never AVFoundation's text (jukebox
    /// F6): the web glue's (media-glue.js) MediaError words, `invalid-value`
    /// for a number out of range. The text stays in `state.media`.
    func fail(_ code: String, _ message: String) {
        guard !invalidated else { return }
        lastError = message
        emit("error", payload: code)
    }
    static func code(_ error: Error?, ready: Bool) -> String {
        guard ready else { return "src-not-supported" }
        var next = error as NSError?
        while let current = next {
            if current.domain == NSURLErrorDomain { return "network" }
            next = current.userInfo[NSUnderlyingErrorKey] as? NSError
        }
        return "decode"
    }
    func timeStatusChanged() {
        guard Thread.isMainThread else { DispatchQueue.main.async { [weak self] in self?.timeStatusChanged() }; return }
        guard !invalidated else { return }
        let status = player.timeControlStatus
        let paused = status == .paused
        if paused != lastPaused { lastPaused = paused; emit(paused ? "pause" : "play"); if !paused { NowPlaying.shared.played(self) } }
        if status != lastTimeStatus {
            lastTimeStatus = status
            if status == .playing { poster.isHidden = true; emit("playing") }
            if status == .waitingToPlayAtSpecifiedRate { emit("waiting") }
        }
    }
    func update(_ values: [String: String]) {
        let old = props
        props = values
        listeners = Set((values["exactListeners"] ?? "").split(separator: " ").map(String.init))
        updateTick()
        let changed = { (name: String) in old[name] != values[name] }
        for (name, min, max) in [("volume", 0.0, 1.0), ("playbackRate", 0.25, 4.0), ("currentTime", 0.0, Double.greatestFiniteMagnitude), ("preferredPeakBitRate", 0.0, Double.greatestFiniteMagnitude), ("preferredForwardBufferDuration", 0.0, Double.greatestFiniteMagnitude)] {
            if let text = props[name], let value = Double(text), value.isFinite, value >= min, value <= max { continue }
            if props[name] != nil { fail("invalid-value", "Invalid \(name)"); props.removeValue(forKey: name) }
        }
        #if os(macOS)
        presentation.controlsStyle = bool("controls") ? .inline : .none
        presentation.showsFullScreenToggleButton = !(props["controlslist"] ?? "").split(separator: " ").contains("nofullscreen")
        presentation.showsSharingServiceButton = false
        presentation.showsTimecodes = bool("showsTimecodes")
        presentation.allowsPictureInPicturePlayback = props["semanticTag"] != "audio" && !bool("disablepictureinpicture") && bool("allowsPictureInPicturePlayback", true)
        presentation.allowsVideoFrameAnalysis = bool("allowsVideoFrameAnalysis", true)
        #else
        configurePresentation()
        controller?.showsPlaybackControls = bool("controls")
        controller?.allowsPictureInPicturePlayback = props["semanticTag"] != "audio" && !bool("disablepictureinpicture") && bool("allowsPictureInPicturePlayback", true)
        // tvOS playback is always full screen and has no inline PiP or frame analysis.
        #if !os(tvOS)
        controller?.canStartPictureInPictureAutomaticallyFromInline = bool("canStartPictureInPictureAutomaticallyFromInline")
        controller?.entersFullScreenWhenPlaybackBegins = props["semanticTag"] != "audio" && bool("entersFullScreenWhenPlaybackBegins", !bool("playsinline"))
        controller?.exitsFullScreenWhenPlaybackEnds = bool("exitsFullScreenWhenPlaybackEnds")
        #endif
        controller?.requiresLinearPlayback = bool("requiresLinearPlayback")
        #if !os(tvOS)
        controller?.allowsVideoFrameAnalysis = bool("allowsVideoFrameAnalysis", true)
        #endif
        #endif
        // Each setter is a command to the media server and a KVO event; only a
        // changed value is sent.
        set(\.isMuted, bool("muted"))
        set(\.volume, Float(number("volume", 1)))
        set(\.allowsExternalPlayback, !bool("disableremoteplayback") && !(props["controlslist"] ?? "").split(separator: " ").contains("noremoteplayback"))
        set(\.automaticallyWaitsToMinimizeStalling, bool("automaticallyWaitsToMinimizeStalling", true))
        set(\.preventsDisplaySleepDuringVideoPlayback, bool("preventsDisplaySleepDuringVideoPlayback", true))
        // HTML's loop seeks to the start at the end without pausing (no pause, play or ended).
        set(\.actionAtItemEnd, bool("loop") ? .none : .pause)
        if changed("poster") { loadPoster() }
        if changed("dynamicRangeLimit") { applyDynamicRange() }
        if changed("src") {
            wantsPlay = props["paused"].map { $0 == "false" } ?? bool("autoplay")
            loadSource()
        }
        if changed("paused"), let value = props["paused"] {
            wantsPlay = value == "false"
            if wantsPlay { if player.currentItem == nil { loadSource() }; play() } else { player.pause() }
        }
        if changed("playbackRate") {
            if rate != reportedRate { reportedRate = rate; emit("ratechange") }
            if player.rate != 0 { player.rate = rate }
        }
        if changed("currentTime"), props["currentTime"] != nil { seek(number("currentTime", 0)) }
        // `load(id)`: the source again, as a changed `src` loads it (a retry
        // after an error, or a file written since); `fastSeek(id, seconds)`
        // seeks each time, to the exact time, which HTML's approximate-for-
        // speed allows (podcast F8, F18). Each is a numbered request.
        if changed("exactLoad"), props["exactLoad"] != nil {
            wantsPlay = props["paused"].map { $0 == "false" } ?? bool("autoplay")
            loadSource()
        }
        if changed("exactSeek"), let request = props["exactSeek"]?.split(separator: " ").last.flatMap({ Double($0) }) {
            if request.isFinite, request >= 0 { seek(request) } else { fail("invalid-value", "Invalid fastSeek") }
        }
        configureItem()
        if let error = props["sourceError"], error != old["sourceError"] { fail("src-not-supported", error) }
        layout()
        NowPlaying.shared.updated(self)
        // The driver's `tap … mediasession` (LLP 1098 D10): `<n> <action> [seconds]`,
        // the command target's own path, its status reported back at once.
        if changed("exactRemote"), let request = props["exactRemote"] {
            let parts = request.split(separator: " ").map(String.init)
            let status: String
            if parts.count < 2 { status = "unreadable" }
            else if NowPlaying.shared.owner !== self { status = "notOwner" }
            else {
                switch NowPlaying.shared.perform(parts[1], seconds: parts.count > 2 ? Double(parts[2]) : nil) {
                case .success: status = "success"
                case .noActionableNowPlayingItem: status = "noOwner"
                default: status = "commandFailed"
                }
            }
            send(["remoteResult": ["n": parts.first ?? "", "status": status]])
        }
        emit()
    }
    func set<Value: Equatable>(_ key: ReferenceWritableKeyPath<AVPlayer, Value>, _ value: Value) {
        if player[keyPath: key] != value { player[keyPath: key] = value }
    }
    func configureItem() {
        guard let item = player.currentItem else { return }
        let peak = number("preferredPeakBitRate", 0), buffer = number("preferredForwardBufferDuration", 0)
        let pitch: AVAudioTimePitchAlgorithm = bool("preservesPitch", true) ? .spectral : .varispeed
        if item.preferredPeakBitRate != peak { item.preferredPeakBitRate = peak }
        if item.preferredForwardBufferDuration != buffer { item.preferredForwardBufferDuration = buffer }
        if item.audioTimePitchAlgorithm != pitch { item.audioTimePitchAlgorithm = pitch }
    }
    func loadSource() {
        generation += 1
        lastError = nil
        observations.removeAll()
        notifications.forEach(NotificationCenter.default.removeObserver)
        notifications.removeAll()
        player.replaceCurrentItem(with: nil)
        naturalSize = .zero
        itemReady = false; seekTarget = nil; atEnd = false
        pendingSeek = props["currentTime"].flatMap(Double.init)
        guard let source = props["src"], !source.isEmpty, let url = URL(string: source) else { return }
        // preload is a hint: AVKit may prepare an item so its native Play control works.
        let item = AVPlayerItem(asset: SharedAssets.asset(url))
        player.replaceCurrentItem(with: item)
        configureItem()
        let token = generation
        observations = [item.observe(\.status, options: [.initial, .new]) { [weak self, weak item] _, _ in
            DispatchQueue.main.async {
                guard let self, let item, !self.invalidated, self.generation == token else { return }
                if item.status == .failed {
                    SharedAssets.forget(url)
                    self.fail(Self.code(item.error, ready: self.itemReady), item.error?.localizedDescription ?? "Media could not be loaded")
                }
                if item.status == .readyToPlay {
                    self.itemReady = true
                    self.naturalSize = item.presentationSize
                    self.emit(self.listeners.contains("loadedmetadata") ? "loadedmetadata" : "snapshot")
                    if item.duration.seconds.isFinite { self.emit("durationchange", payload: String(item.duration.seconds)) }
                    self.emit("canplay")
                    if let time = self.pendingSeek { self.pendingSeek = nil; self.seek(time) }
                    if self.wantsPlay { self.play() }
                    self.layout()
                }
            }
        }, item.observe(\.presentationSize, options: [.new]) { [weak self] item, _ in
            DispatchQueue.main.async { [weak self] in
                guard let self, !self.invalidated, self.generation == token else { return }
                self.naturalSize = item.presentationSize; self.layout(); self.emit()
            }
        }]
        notifications.append(NotificationCenter.default.addObserver(forName: .AVPlayerItemDidPlayToEndTime, object: item, queue: .main) { [weak self] _ in
            guard let self, !self.invalidated, self.generation == token else { return }
            // The player keeps its rate at the end (`actionAtItemEnd` is none);
            // the seek reports seeking, seeked and timeupdate, as Chrome does.
            // HTML's end without loop: `pause`, then `ended` (AVPlayer's
            // own pause arrives after this notification; it is the same one).
            guard !self.bool("loop") else { self.seek(0); return }
            self.wantsPlay = false; self.atEnd = true
            if !self.lastPaused { self.lastPaused = true; self.emit("pause") }
            self.emit("ended")
        })
        if wantsPlay { play() }
    }
    func play() {
        player.defaultRate = rate
        if atEnd { atEnd = false; seek(0) }
        player.play()
    }
    func seek(_ time: Double) {
        guard player.currentItem?.status == .readyToPlay else { pendingSeek = time; return }
        poster.isHidden = true
        seekTarget = time; seeks += 1; atEnd = false
        emit("seeking")
        let token = generation, mine = seeks
        player.seek(to: CMTime(seconds: time, preferredTimescale: 600), toleranceBefore: .zero, toleranceAfter: .zero) { [weak self] completed in
            DispatchQueue.main.async {
                // A seek another replaced completes unfinished; the last one lands.
                guard let self, !self.invalidated, self.generation == token, mine == self.seeks else { return }
                self.seekTarget = nil
                guard completed else { return }
                self.emit("seeked"); self.emit("timeupdate", payload: String(self.seconds))
            }
        }
    }
    /// An HDR poster is decoded with its gain map: UIKit's reader must be
    /// asked; `NSImage` keeps it (LLP 1100 D11).
    func loadPoster() {
        posterGeneration += 1
        let token = posterGeneration
        poster.image = nil; poster.isHidden = true
        guard let source = props["poster"], let url = URL(string: source) else { return }
        DispatchQueue.global(qos: .userInitiated).async { [weak self] in
            let data = try? Data(contentsOf: url)
            #if !os(macOS)
            let image = data.flatMap { data -> UIImage? in
                var configuration = UIImageReader.Configuration()
                configuration.prefersHighDynamicRange = true
                return UIImageReader(configuration: configuration).image(data: data)
            }
            #else
            let image = data.flatMap { NSImage(data: $0) }
            #endif
            DispatchQueue.main.async {
                guard let self, !self.invalidated, self.posterGeneration == token, let image else { return }
                self.poster.image = image
                self.applyDynamicRange()
                self.poster.isHidden = self.player.rate != 0
            }
        }
    }

    var hdrItem: Bool {
        player.currentItem?.asset.tracks(withMediaType: .video).contains { $0.hasMediaCharacteristic(.containsHDRVideo) } ?? false
    }

    /// `dynamic-range-limit` on the poster, AVKit's player and the inline
    /// layer (LLP 1100 D11). Before iOS/macOS 26 AVKit has no range control.
    func applyDynamicRange() {
        let limit = props["dynamicRangeLimit"] ?? "no-limit"
        #if !os(macOS)
        let image: UIImage.DynamicRange = limit == "standard" ? .standard : limit == "constrained" ? .constrainedHigh : .high
        #else
        let image: NSImage.DynamicRange = limit == "standard" ? .standard : limit == "constrained" ? .constrainedHigh : .high
        #endif
        if poster.preferredImageDynamicRange != image { poster.preferredImageDynamicRange = image }
        #if os(iOS) || os(tvOS)
        if let layer = inline?.playerLayer {
            if #available(iOS 26, tvOS 26, *) {
                let wanted: CALayer.DynamicRange = limit == "standard" ? .standard : limit == "constrained" ? .constrainedHigh : .high
                if layer.preferredDynamicRange != wanted { layer.preferredDynamicRange = wanted }
            } else {
                #if os(iOS)
                layer.wantsExtendedDynamicRangeContent = limit != "standard"
                #endif
            }
        }
        #endif
        // tvOS has no AVKit range preference.
        #if os(iOS) || os(macOS)
        if #available(iOS 26, macOS 26, *) {
            let range: AVDisplayDynamicRange = limit == "standard" ? .standard : limit == "constrained" ? .constrainedHigh : .high
            #if os(iOS)
            if let controller, controller.preferredDisplayDynamicRange != range { controller.preferredDisplayDynamicRange = range }
            #else
            if presentation.preferredDisplayDynamicRange != range { presentation.preferredDisplayDynamicRange = range }
            #endif
        }
        #endif
    }
    #if os(iOS) || os(tvOS)
    /// A video without `controls` uses the native player layer, as Chrome's
    /// `<video>` without controls draws no UI (LLP 1042 §7 A): PiP and frame
    /// analysis select AVKit only when set by name, since without controls or
    /// automatic start PiP cannot be reached. Enabling a controller feature
    /// promotes the same player; an existing controller remains its owner
    /// until this node is destroyed.
    private func configurePresentation() {
        guard props["src"] != nil || presentation != nil else { return }
        // An `audio` has no picture to take full screen or to picture in
        // picture: only its `controls` ask for AVKit (LLP 1042 §8).
        let audio = props["semanticTag"] == "audio"
        let needsController = bool("controls") || !audio && (
            (!bool("disablepictureinpicture") && props["allowsPictureInPicturePlayback"] == "true")
            || bool("canStartPictureInPictureAutomaticallyFromInline")
            || bool("entersFullScreenWhenPlaybackBegins", !bool("playsinline"))
            || bool("exitsFullScreenWhenPlaybackEnds") || bool("requiresLinearPlayback")
            || props["allowsVideoFrameAnalysis"] == "true")
        if controller == nil && needsController {
            let native = AVPlayerViewController()
            native.player = player
            // Its gravity before it is on screen: AVKit animates a later
            // change, and on iOS 17 that animation's mirrored
            // `sublayerTransform.scale.y` carries a CGSize, which
            // `renderInContext` throws on (object-fit does not animate in CSS).
            native.videoGravity = gravity
            controller = native
            #if os(iOS)
            native.updatesNowPlayingInfoCenter = avkitPublishes
            #endif
            applyDynamicRange()
            container.insertSubview(native.view, belowSubview: poster)
            inline?.playerLayer.player = nil
            inline?.removeFromSuperview()
            inline = nil
            attach()
        } else if controller == nil && inline == nil {
            let surface = VideoLayerView(frame: container.bounds)
            surface.isUserInteractionEnabled = false
            surface.playerLayer.player = player
            surface.playerLayer.videoGravity = gravity
            inline = surface
            applyDynamicRange()
            container.insertSubview(surface, belowSubview: poster)
        }
    }
    func attach() {
        guard let controller, container.window != nil, controller.parent == nil else { return }
        var responder: UIResponder? = container.next
        while let current = responder {
            if let parent = current as? UIViewController { parent.addChild(controller); controller.didMove(toParent: parent); break }
            responder = current.next
        }
    }
    #endif
    /// CSS `object-fit` as AVFoundation's gravity.
    var gravity: AVLayerVideoGravity {
        let fit = props["objectFit"] ?? "contain"
        return fit == "contain" || fit == "scale-down" ? .resizeAspect : fit == "cover" ? .resizeAspectFill : .resize
    }
    func layout() {
        guard !invalidated else { return }
        #if os(iOS) || os(tvOS)
        attach()
        #endif
        let fit = props["objectFit"] ?? "contain", gravity = self.gravity
        #if os(macOS)
        presentation.videoGravity = gravity
        #else
        if let controller, controller.videoGravity != gravity {
            UIView.performWithoutAnimation { controller.videoGravity = gravity }
        }
        if let layer = inline?.playerLayer, layer.videoGravity != gravity { layer.videoGravity = gravity }
        guard let presentation else { poster.frame = container.bounds; return }
        #endif
        var frame = container.bounds
        if (fit == "none" || fit == "scale-down"), naturalSize.width > 0, naturalSize.height > 0 {
            let scale = fit == "none" ? 1 : min(1, min(frame.width / naturalSize.width, frame.height / naturalSize.height))
            frame = CGRect(x: (frame.width - naturalSize.width * scale) / 2, y: (frame.height - naturalSize.height * scale) / 2, width: naturalSize.width * scale, height: naturalSize.height * scale)
        }
        if presentation.superview === container { presentation.frame = frame }
        poster.frame = container.bounds
    }
    func invalidate() {
        guard !invalidated else { return }
        invalidated = true; generation += 1; posterGeneration += 1
        NowPlaying.shared.left(self)
        player.pause()
        if let tick { player.removeTimeObserver(tick) }; tick = nil
        observations.removeAll(); playerObservations.removeAll()
        notifications.forEach(NotificationCenter.default.removeObserver); notifications.removeAll()
        player.replaceCurrentItem(with: nil)
        #if os(iOS) || os(tvOS)
        fullscreen?.left = nil; fullscreen?.dismiss(animated: false); fullscreen = nil
        controller?.willMove(toParent: nil); controller?.view.removeFromSuperview(); controller?.removeFromParent()
        inline?.playerLayer.player = nil
        inline?.removeFromSuperview()
        controller = nil; inline = nil
        #endif
        container.removeFromSuperview()
    }
}

#if os(iOS) || os(tvOS)
/// The full-screen player: AVKit's own controller on the inline one's
/// AVPlayer, so the time, the play or pause, and the rate are the same ones
/// when it closes (Menu on tvOS, Done on iOS) as when it opened.
final class FullscreenPlayer: AVPlayerViewController {
    var left: (() -> Void)?
    override func viewDidDisappear(_ animated: Bool) {
        super.viewDidDisappear(animated)
        guard isBeingDismissed || presentingViewController == nil else { return }
        // Let go of the player first: AVKit pauses the one a closed controller holds.
        player = nil
        left?(); left = nil
    }
}

extension VideoArm {
    /// `requestFullscreen` (HTML's Element.requestFullscreen): the picture
    /// takes the screen; `fullscreenchange` reports true, then false when it
    /// closes. A second request while it shows, or an `audio`, does nothing.
    func enterFullscreen() {
        guard !invalidated, fullscreen == nil, props["semanticTag"] != "audio", player.currentItem != nil else { return }
        var responder: UIResponder? = container.next
        while let current = responder, !(current is UIViewController) { responder = current.next }
        guard var host = responder as? UIViewController else { return }
        while let shown = host.presentedViewController { host = shown }
        let full = FullscreenPlayer()
        full.player = player
        full.videoGravity = .resizeAspect
        full.modalPresentationStyle = .fullScreen
        // One picture at a time: the inline view lets go of the player while
        // the full-screen one shows it, and takes it back after.
        controller?.player = nil
        inline?.playerLayer.player = nil
        full.left = { [weak self] in self?.leftFullscreen() }
        fullscreen = full
        host.present(full, animated: true) { [weak self] in self?.emit("fullscreenchange", payload: "true") }
    }

    private func leftFullscreen() {
        guard !invalidated else { return }
        fullscreen = nil
        controller?.player = player
        inline?.playerLayer.player = player
        emit("fullscreenchange", payload: "false")
    }
}
#else
extension VideoArm {
    func enterFullscreen() {
        FileHandle.standardError.write(Data("exact video: requestFullscreen: not on macOS yet\n".utf8))
    }
}
#endif

@_cdecl("exact_video_create")
public func videoCreate(_ context: UnsafeMutableRawPointer?, _ callback: VideoCallback?) -> UnsafeMutableRawPointer? {
    guard let callback else { return nil }
    return Unmanaged.passRetained(VideoArm(context: context, callback: callback)).toOpaque()
}
private func arm(_ raw: UnsafeMutableRawPointer?) -> VideoArm? { raw.map { Unmanaged<VideoArm>.fromOpaque($0).takeUnretainedValue() } }
@_cdecl("exact_video_view")
public func videoView(_ raw: UnsafeMutableRawPointer?) -> UnsafeMutableRawPointer? { arm(raw).map { Unmanaged.passUnretained($0.container).toOpaque() } }
@_cdecl("exact_video_update")
public func videoUpdate(_ raw: UnsafeMutableRawPointer?, _ bytes: UnsafePointer<UInt8>?, _ count: Int) {
    guard let object = arm(raw), let bytes, let values = try? JSONSerialization.jsonObject(with: Data(bytes: bytes, count: count)) as? [String: String] else { return }
    object.update(values)
}
@_cdecl("exact_video_fullscreen")
public func videoFullscreen(_ raw: UnsafeMutableRawPointer?) { arm(raw)?.enterFullscreen() }
@_cdecl("exact_video_state")
public func videoState(_ raw: UnsafeMutableRawPointer?) { arm(raw)?.emit() }
@_cdecl("exact_video_destroy")
public func videoDestroy(_ raw: UnsafeMutableRawPointer?) {
    guard let raw else { return }
    let object = Unmanaged<VideoArm>.fromOpaque(raw).takeRetainedValue(); object.invalidate()
}
