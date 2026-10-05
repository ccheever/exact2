// @ref LLP 1098 D3, D5, D7, D10. The media session on Apple: one coordinator
// per process, in the video arm (one image per process, so every session's
// players register here), over `MPNowPlayingInfoCenter` and
// `MPRemoteCommandCenter`. A player whose props carry `mediaTitle` (the
// element's `metadata=`) is a claimant; the owner is the claimant that most
// recently reported `play`, kept after it pauses, else the latest to
// register (a commit registers its claimants in document order, and
// sessions in the order they register). Under the driver (`exactPublish`
// "false") the same info and command set are built and reported, never
// assigned: a drive must not take a developer's media keys.
import Foundation
import MediaPlayer
#if os(macOS)
import AppKit
#else
import UIKit
#endif

/// What the coordinator asks of a player: the arm's (VideoArm.swift), or a test's.
protocol NowPlayingPlayer: AnyObject {
    /// The element's props as ExactKit sent them: `mediaTitle` …, the
    /// offsets, `mediaArtworkURL` (resolved), `semanticTag`, `exactPublish`,
    /// `exactListeners` (the events the element handles).
    var sessionProps: [String: String] { get }
    /// The player is paused (`timeControlStatus`).
    var sessionPaused: Bool { get }
    /// Seconds; not finite while unknown, infinite for a live stream.
    var sessionDuration: Double { get }
    var sessionElapsed: Double { get }
    /// The element's `playbackRate`, never 0.
    var sessionRate: Double { get }
    /// The platform's play and pause act on the element (D3).
    func sessionPlay()
    func sessionPause()
    /// One of the six, as the element's event with `seekOffset seekTime fastSeek`.
    func sessionAction(_ name: String, payload: String)
    /// AVKit's own publication (`updatesNowPlayingInfoCenter`), off while a claimant owns the session.
    func sessionAVKitPublishes(_ on: Bool)
}

final class NowPlaying {
    static let shared = NowPlaying()
    static let actions = ["seekbackward", "seekforward", "seekto", "previoustrack", "nexttrack", "stop"]
    private final class Weak { weak var player: NowPlayingPlayer?; init(_ p: NowPlayingPlayer) { player = p } }
    /// Every player (AVKit's flag is every arm's) and the claimants' turns.
    private var players: [ObjectIdentifier: Weak] = [:]
    private var claims: [ObjectIdentifier: (mounted: Int, played: Int)] = [:]
    private var turn = 0
    private(set) weak var owner: NowPlayingPlayer?
    /// The session was published, so a clear has something to undo; a page
    /// of plain videos leaves AVKit's Now Playing alone.
    private var published = false
    private var targets: [Any] = []
    private var artwork: (source: String, image: MPMediaItemArtwork?, error: String?, generation: Int) = ("", nil, nil, 0)
    /// What was built for the owner, published or not: `state`'s readback.
    private(set) var info: [String: Any]?
    private(set) var enabled: [String] = []
    #if os(macOS)
    private(set) var playbackState = "stopped"
    #endif

    func joined(_ p: NowPlayingPlayer) {
        players[ObjectIdentifier(p)] = Weak(p)
        p.sessionAVKitPublishes(owner == nil)
    }
    /// The player's props changed: it claims while it carries `mediaTitle`.
    func updated(_ p: NowPlayingPlayer) {
        let key = ObjectIdentifier(p)
        if p.sessionProps["mediaTitle"] == nil { claims.removeValue(forKey: key) }
        else if claims[key] == nil { turn += 1; claims[key] = (turn, 0) }
        refresh()
    }
    /// HTML's `play`: `paused` became false (D5).
    func played(_ p: NowPlayingPlayer) {
        guard claims[ObjectIdentifier(p)] != nil else { return }
        turn += 1; claims[ObjectIdentifier(p)]?.played = turn
        refresh()
    }
    func left(_ p: NowPlayingPlayer) {
        let key = ObjectIdentifier(p)
        players.removeValue(forKey: key)
        if claims.removeValue(forKey: key) != nil || owner === p { refresh() }
    }
    /// The owner's position or state moved (`play`, `pause`, a rate, a seek,
    /// the end, a duration): published again, never at `timeupdate`.
    func moved(_ p: NowPlayingPlayer) { if owner === p { refresh() } }

    private func ownerNow() -> NowPlayingPlayer? {
        var best: (NowPlayingPlayer, Int, Int)?
        for (key, claim) in claims {
            guard let p = players[key]?.player else { claims.removeValue(forKey: key); continue }
            let rank = claim.played > 0 ? (1, claim.played) : (0, claim.mounted)
            if best == nil || rank > (best!.1, best!.2) { best = (p, rank.0, rank.1) }
        }
        return best?.0
    }

    func refresh() {
        let next = ownerNow()
        if (next == nil) != (owner == nil) { for entry in players.values { entry.player?.sessionAVKitPublishes(next == nil) } }
        owner = next
        guard let p = next else { clear(); return }
        let props = p.sessionProps, publish = props["exactPublish"] != "false"
        loadArtwork(props["mediaArtwork"] ?? "", resolved: props["mediaArtworkURL"] ?? "")
        var info: [String: Any] = [
            MPMediaItemPropertyTitle: props["mediaTitle"] ?? "",
            MPMediaItemPropertyArtist: props["mediaArtist"] ?? "",
            MPMediaItemPropertyAlbumTitle: props["mediaAlbum"] ?? "",
            MPNowPlayingInfoPropertyPlaybackRate: p.sessionPaused ? 0.0 : p.sessionRate,
            MPNowPlayingInfoPropertyDefaultPlaybackRate: p.sessionRate,
            MPNowPlayingInfoPropertyMediaType: NSNumber(value: (props["semanticTag"] == "audio" ? MPNowPlayingInfoMediaType.audio : .video).rawValue),
        ]
        let duration = p.sessionDuration
        if duration.isFinite, duration > 0 {
            info[MPMediaItemPropertyPlaybackDuration] = duration
            info[MPNowPlayingInfoPropertyElapsedPlaybackTime] = min(max(p.sessionElapsed, 0), duration)
        } else if duration.isInfinite { info[MPNowPlayingInfoPropertyIsLiveStream] = true }
        if let image = artwork.image { info[MPMediaItemPropertyArtwork] = image }
        let handled = Set((props["exactListeners"] ?? "").split(separator: " ").map(String.init))
        enabled = ["pause", "play", "togglePlayPause"] + Self.actions.filter(handled.contains)
        self.info = info
        #if os(macOS)
        playbackState = p.sessionPaused ? "paused" : "playing"
        #endif
        guard publish else { return }
        published = true
        let center = MPNowPlayingInfoCenter.default()
        center.nowPlayingInfo = info
        #if os(macOS)
        // "Must be set every time the app begins or halts playback" (MPNowPlayingInfoCenter.h).
        center.playbackState = p.sessionPaused ? .paused : .playing
        #endif
        commands(enabled: Set(enabled), props: props)
    }

    private func clear() {
        info = nil; enabled = []
        #if os(macOS)
        playbackState = "stopped"
        #endif
        artwork = ("", nil, nil, artwork.generation + 1)
        guard published else { return }
        published = false
        MPNowPlayingInfoCenter.default().nowPlayingInfo = nil
        #if os(macOS)
        MPNowPlayingInfoCenter.default().playbackState = .stopped
        #endif
        commands(enabled: [], props: [:])
    }

    // MARK: - commands

    private static func offset(_ props: [String: String], _ name: String) -> Double {
        props[name].flatMap(Double.init).flatMap { $0.isFinite && $0 > 0 ? $0 : nil } ?? 10
    }

    private func commands(enabled: Set<String>, props: [String: String]) {
        let c = MPRemoteCommandCenter.shared()
        if targets.isEmpty {
            targets = [
                c.playCommand.addTarget { [unowned self] _ in perform("play") },
                c.pauseCommand.addTarget { [unowned self] _ in perform("pause") },
                c.togglePlayPauseCommand.addTarget { [unowned self] _ in perform("togglePlayPause") },
                c.skipBackwardCommand.addTarget { [unowned self] e in perform("seekbackward", seconds: (e as? MPSkipIntervalCommandEvent)?.interval) },
                c.skipForwardCommand.addTarget { [unowned self] e in perform("seekforward", seconds: (e as? MPSkipIntervalCommandEvent)?.interval) },
                c.changePlaybackPositionCommand.addTarget { [unowned self] e in perform("seekto", seconds: (e as? MPChangePlaybackPositionCommandEvent)?.positionTime) },
                c.previousTrackCommand.addTarget { [unowned self] _ in perform("previoustrack") },
                c.nextTrackCommand.addTarget { [unowned self] _ in perform("nexttrack") },
                c.stopCommand.addTarget { [unowned self] _ in perform("stop") },
            ]
        }
        let all: [(String, MPRemoteCommand)] = [("play", c.playCommand), ("pause", c.pauseCommand), ("togglePlayPause", c.togglePlayPauseCommand),
            ("seekbackward", c.skipBackwardCommand), ("seekforward", c.skipForwardCommand), ("seekto", c.changePlaybackPositionCommand),
            ("previoustrack", c.previousTrackCommand), ("nexttrack", c.nextTrackCommand), ("stop", c.stopCommand)]
        for (name, command) in all where command.isEnabled != enabled.contains(name) { command.isEnabled = enabled.contains(name) }
        // The intervals the lock screen shows are the element's offsets (D2).
        let back = [NSNumber(value: Self.offset(props, "seekbackwardOffset"))], forward = [NSNumber(value: Self.offset(props, "seekforwardOffset"))]
        if c.skipBackwardCommand.preferredIntervals != back { c.skipBackwardCommand.preferredIntervals = back }
        if c.skipForwardCommand.preferredIntervals != forward { c.skipForwardCommand.preferredIntervals = forward }
    }

    /// A command's target, and the driver's `tap … mediasession` (D10): play
    /// and pause on the element, the six as its events, `seekOffset` the
    /// platform's (the driver's seconds), else the element's offset.
    func perform(_ action: String, seconds: Double? = nil) -> MPRemoteCommandHandlerStatus {
        guard let p = owner else { return .noActionableNowPlayingItem }
        switch action {
        case "play": p.sessionPlay(); return .success
        case "pause": p.sessionPause(); return .success
        case "togglePlayPause": if p.sessionPaused { p.sessionPlay() } else { p.sessionPause() }; return .success
        default: break
        }
        let props = p.sessionProps
        guard Self.actions.contains(action), (props["exactListeners"] ?? "").split(separator: " ").contains(Substring(action)) else { return .commandFailed }
        let ok = { (n: Double?) in n.flatMap { $0.isFinite && $0 >= 0 ? $0 : nil } }
        // A skip event of no interval (0) gives none: the element's offset.
        let offset = action == "seekbackward" || action == "seekforward" ? seconds.flatMap { $0.isFinite && $0 > 0 ? $0 : nil } ?? Self.offset(props, "\(action)Offset") : 0
        let time = action == "seekto" ? ok(seconds) : 0
        guard let time else { return .commandFailed }
        p.sessionAction(action, payload: "\(Self.number(offset)) \(Self.number(time)) 0")
        return .success
    }
    private static func number(_ n: Double) -> String { n == n.rounded() && abs(n) < 1e15 ? String(Int64(n)) : String(n) }

    // MARK: - artwork

    /// Its own loader and generation (the poster's drops a failure without a
    /// word): a resolved URL loads, a newer source cancels the load in
    /// flight, and an unresolved or failed one sets `artworkError` (D7).
    private func loadArtwork(_ source: String, resolved: String) {
        guard source != artwork.source else { return }
        artwork = (source, nil, nil, artwork.generation + 1)
        guard !source.isEmpty else { return }
        guard let url = URL(string: resolved), !resolved.isEmpty else {
            artwork.error = source.hasPrefix("app:/") ? "an app:/ artwork is not published until the media element takes an app:/ source (LLP 1098 §7)" : "the artwork's source did not resolve"
            return
        }
        let token = artwork.generation
        DispatchQueue.global(qos: .userInitiated).async { [weak self] in
            let data = try? Data(contentsOf: url)
            DispatchQueue.main.async {
                guard let self, self.artwork.generation == token else { return }
                #if os(macOS)
                let image = data.flatMap(NSImage.init(data:))
                #else
                let image = data.flatMap(UIImage.init(data:))
                #endif
                guard let image else { self.artwork.error = "the artwork did not load"; return }
                self.artwork.image = MPMediaItemArtwork(boundsSize: image.size) { _ in image }
                self.refresh()
            }
        }
    }

    // MARK: - state

    /// `state.mediaSession`'s part this player knows (D10): whether it
    /// claims and owns; for the owner, what the platform is offered, the
    /// position, and the readback (the center's own when published).
    func report(_ p: NowPlayingPlayer) -> [String: Any] {
        let key = ObjectIdentifier(p)
        var out: [String: Any] = ["claimant": claims[key] != nil, "owner": owner === p]
        guard owner === p, let info else { return out }
        let props = p.sessionProps
        let publish = props["exactPublish"] != "false"
        out["metadata"] = ["title": props["mediaTitle"] ?? "", "artist": props["mediaArtist"] ?? "", "album": props["mediaAlbum"] ?? "", "artwork": props["mediaArtwork"] ?? ""]
        if let error = artwork.error { out["artworkError"] = error }
        out["actions"] = (["play", "pause"] + Self.actions.filter(enabled.contains)).sorted()
        out["seekOffsets"] = ["seekbackward": Self.offset(props, "seekbackwardOffset"), "seekforward": Self.offset(props, "seekforwardOffset")]
        out["playbackState"] = p.sessionPaused ? "paused" : "playing"
        let duration = p.sessionDuration
        out["position"] = duration.isFinite && duration > 0 ? ["duration": duration, "position": min(max(p.sessionElapsed, 0), duration), "playbackRate": p.sessionRate] as [String: Any] : NSNull()
        out["published"] = publish ? "MPNowPlayingInfoCenter" : "agent"
        let shown = publish ? MPNowPlayingInfoCenter.default().nowPlayingInfo ?? [:] : info
        let c = MPRemoteCommandCenter.shared()
        var readback: [String: Any] = [
            "title": shown[MPMediaItemPropertyTitle] ?? NSNull(), "artist": shown[MPMediaItemPropertyArtist] ?? NSNull(),
            "album": shown[MPMediaItemPropertyAlbumTitle] ?? NSNull(), "artwork": shown[MPMediaItemPropertyArtwork] != nil,
            "duration": shown[MPMediaItemPropertyPlaybackDuration] ?? NSNull(), "elapsed": shown[MPNowPlayingInfoPropertyElapsedPlaybackTime] ?? NSNull(),
            "rate": shown[MPNowPlayingInfoPropertyPlaybackRate] ?? NSNull(), "defaultRate": shown[MPNowPlayingInfoPropertyDefaultPlaybackRate] ?? NSNull(),
            "mediaType": (shown[MPNowPlayingInfoPropertyMediaType] as? NSNumber)?.uintValue == MPNowPlayingInfoMediaType.audio.rawValue ? "audio" : "video",
            "liveStream": shown[MPNowPlayingInfoPropertyIsLiveStream] as? Bool ?? false,
            "commands": publish ? ["play": c.playCommand, "pause": c.pauseCommand, "togglePlayPause": c.togglePlayPauseCommand, "seekbackward": c.skipBackwardCommand, "seekforward": c.skipForwardCommand, "seekto": c.changePlaybackPositionCommand, "previoustrack": c.previousTrackCommand, "nexttrack": c.nextTrackCommand, "stop": c.stopCommand].filter { $0.value.isEnabled }.keys.sorted() : enabled.sorted(),
            "preferredIntervals": ["seekbackward": publish ? c.skipBackwardCommand.preferredIntervals.map(\.doubleValue) : [Self.offset(props, "seekbackwardOffset")],
                                   "seekforward": publish ? c.skipForwardCommand.preferredIntervals.map(\.doubleValue) : [Self.offset(props, "seekforwardOffset")]],
        ]
        #if os(macOS)
        readback["playbackState"] = publish ? ["unknown", "playing", "paused", "stopped", "interrupted"][Int(MPNowPlayingInfoCenter.default().playbackState.rawValue)] : playbackState
        #endif
        out["readback"] = readback
        return out
    }
}
