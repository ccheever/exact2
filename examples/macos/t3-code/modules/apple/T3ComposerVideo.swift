#if os(macOS)
import AppKit
import AVKit

/// r4-composer: the expanded preview of a video on the composer's shelf
/// (ExpandedImageDialog's ExpandedVideo, MIT reference; see LICENSE-T3). Hook
/// `t3-video` with `data-snapshot-id` naming the staged file: an AVPlayerView
/// with its own controls fills the box and starts playing, as the reference's
/// `<video controls autoplay>` does. It plays the typed link
/// T3ComposerAttach.swift made beside the staged copy (`<id>.<ext>`).
final class T3ComposerVideo {
    private let staged: URL
    private let muted: Bool
    private final class Entry {
        weak var host: NSView?
        let id: String
        let view: AVPlayerView
        init(host: NSView, id: String, view: AVPlayerView) { self.host = host; self.id = id; self.view = view }
    }
    private var entries: [ObjectIdentifier: Entry] = [:]

    /// Agent runs play silently.
    init(dataRoot: URL, muted: Bool) {
        staged = dataRoot.appendingPathComponent("composer-files", isDirectory: true)
        self.muted = muted
    }

    /// The staged file's typed link, if its id is a draft file's.
    func source(_ id: String) -> URL? {
        guard UUID(uuidString: id) != nil else { return nil }
        let prefix = "\(id.lowercased())."
        return ((try? FileManager.default.contentsOfDirectory(at: staged, includingPropertiesForKeys: nil)) ?? [])
            .first { $0.lastPathComponent.hasPrefix(prefix) }
    }

    var status: [String: Any] { ["videoPlayers": entries.count] }

    func install(_ element: ExactElement) {
        guard element.hook == .t3Video, let host = element.view else { return }
        let id = element.data[.snapshotId] ?? ""
        let key = ObjectIdentifier(host)
        if entries[key]?.id == id { return }
        detach(key)
        guard let url = source(id) else { return }
        let view = AVPlayerView(frame: host.bounds)
        view.autoresizingMask = [.width, .height]
        view.controlsStyle = .inline
        view.showsFullScreenToggleButton = true
        let player = AVPlayer(url: url)
        player.isMuted = muted
        view.player = player
        host.addSubview(view)
        entries[key] = Entry(host: host, id: id, view: view)
        player.play()
    }

    func remove(_ element: ExactElement) {
        if let host = element.view { detach(ObjectIdentifier(host)) }
        for (key, entry) in entries where entry.host == nil { entry.view.player?.pause(); entry.view.removeFromSuperview(); entries[key] = nil }
    }

    private func detach(_ key: ObjectIdentifier) {
        guard let entry = entries[key] else { return }
        entry.view.player?.pause()
        entry.view.player = nil
        entry.view.removeFromSuperview()
        entries[key] = nil
    }

    func destroy() { for key in Array(entries.keys) { detach(key) } }
}
#endif
