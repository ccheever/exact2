// A received share's claim (LLP 1133.024 D2–D4): the Rust host claims it,
// sets its tokens, delivers its location at the navigation root and lands
// its files in `app:/tmp/shared/`. A share comes from the share extension's
// inbox in the App Group, claimed at launch, at each foreground and on the
// extension's Darwin notification; under the agent the driver's `share:`
// form arrives here too (D9). A host never takes `share:` from outside it.
#if canImport(UIKit)
import UIKit
#else
import AppKit
#endif
import Foundation

/// The share extension's inbox (D4): `<group>/shares/<uuid>/`, each complete
/// once its `share.json` is there.
struct ShareInbox {
    /// The app's `share_target`, which the bake wrote into the Info.plist.
    let target: Any
    let directory: URL

    /// The app's inbox, when it declares `share_target` and is entitled to
    /// the group (`ExactShareGroup` is written only then). Under the agent,
    /// `EXACT_AGENT_SHARE_INBOX` names another directory, for a drive whose
    /// build has no group (an ad-hoc Mac build).
    static var main: ShareInbox? {
        guard let json = Bundle.main.object(forInfoDictionaryKey: "ExactShareTarget") as? String,
              let target = try? JSONSerialization.jsonObject(with: Data(json.utf8)) else { return nil }
        if ExactEnv.agentMode, let path = ExactEnv.environment["EXACT_AGENT_SHARE_INBOX"] {
            return ShareInbox(target: target, directory: URL(fileURLWithPath: path, isDirectory: true))
        }
        guard let group = Bundle.main.object(forInfoDictionaryKey: "ExactShareGroup") as? String,
              let container = FileManager.default.containerURL(forSecurityApplicationGroupIdentifier: group) else { return nil }
        return ShareInbox(target: target, directory: container.appendingPathComponent("shares", isDirectory: true))
    }

    /// The complete shares, oldest first (D8's order); a directory without
    /// `share.json` is still being written, or was abandoned.
    func complete() -> [URL] {
        let dirs = (try? FileManager.default.contentsOfDirectory(at: directory, includingPropertiesForKeys: nil)) ?? []
        let dated: [(URL, Date)] = dirs.compactMap { dir in
            let record = dir.appendingPathComponent("share.json")
            guard let date = (try? record.resourceValues(forKeys: [.contentModificationDateKey]))?.contentModificationDate else { return nil }
            return (dir, date)
        }
        return dated.sorted { $0.1 < $1.1 }.map(\.0)
    }

    /// The bridge's request for the share in `dir`, or nil (the record is
    /// unreadable, and the directory is removed so it is not tried again).
    func request(_ dir: URL) -> [String: Any]? {
        guard let data = try? Data(contentsOf: dir.appendingPathComponent("share.json")),
              let record = try? JSONSerialization.jsonObject(with: data) else {
            try? FileManager.default.removeItem(at: dir)
            return nil
        }
        return ["target": target, "share": record, "inbox": dir.path]
    }
}

/// The inbox's Darwin notification, `<app id>.shares`, which the extension
/// posts after writing a share: while the app runs it claims at once.
private final class ShareInboxPing {
    static let shared = ShareInboxPing()
    weak var session: ExactSession?
    private var listening = false

    func listen(_ name: String, for session: ExactSession) {
        self.session = session
        guard !listening else { return }
        listening = true
        let center = CFNotificationCenterGetDarwinNotifyCenter()
        CFNotificationCenterAddObserver(center, Unmanaged.passUnretained(self).toOpaque(), { _, observer, _, _, _ in
            guard let observer else { return }
            let ping = Unmanaged<ShareInboxPing>.fromOpaque(observer).takeUnretainedValue()
            DispatchQueue.main.async { ping.session?.claimInbox() }
        }, name as CFString, nil, .deliverImmediately)
    }
}

/// The inbox share a cold claim took, removed once its files have moved.
nonisolated(unsafe) private var coldInboxShare: URL?

extension ExactSession {
    /// The sessions that claim at each foreground and on the ping, once each.
    nonisolated(unsafe) private static var claiming = Set<ObjectIdentifier>()

    /// The agent's `share` op: `{"id": <navigation root>, "target", "share"}`.
    func receiveShare(_ req: [String: Any]) -> [String: Any] {
        guard booted, let root = navigationRoot else { return ["error": "share: the app has no navigation root to deliver it to"] }
        var request = req.filter { ["target", "share"].contains($0.key) }
        request["root"] = Int(root.id)
        let batch = deliverShare(request)
        if let error = batch?.error { return ["error": error] }
        return batch == nil ? ["error": "share: the share is malformed"] : ["typed": Int(root.id), "delivery": "recognized"]
    }

    private func deliverShare(_ request: [String: Any]) -> Batch? {
        guard let data = try? JSONSerialization.data(withJSONObject: request) else { return nil }
        let batch = runtime.share(String(decoding: data, as: UTF8.self), now: now())
        apply(batch)
        return batch
    }

    /// Before the boot: the driver's cold share (`EXACT_AGENT_SHARE`), else
    /// the inbox's oldest complete share, becomes the launch location and
    /// its tokens. Only the record is read and the tokens drawn here; the
    /// files move once storage is configured (`landColdShare`).
    func claimColdShare() {
        guard !booted else { return }
        if ExactEnv.agentMode, let json = ExactEnv.environment["EXACT_AGENT_SHARE"] {
            _ = runtime.share(json, now: 0)
            return
        }
        guard let inbox = ShareInbox.main, let dir = inbox.complete().first, let request = inbox.request(dir),
              let data = try? JSONSerialization.data(withJSONObject: request) else { return }
        _ = runtime.share(String(decoding: data, as: UTF8.self), now: 0)
        coldInboxShare = dir
    }

    /// The cold share's files, once activation has configured storage; then
    /// the inbox's other shares, as `navigate` events after the first
    /// frame (LLP 1038 D8's order), and the foreground and the ping from
    /// now on.
    func landColdShare() {
        if (ExactEnv.agentMode && ExactEnv.environment["EXACT_AGENT_SHARE"] != nil) || coldInboxShare != nil {
            apply(runtime.landShare())
        }
        if let dir = coldInboxShare { try? FileManager.default.removeItem(at: dir); coldInboxShare = nil }
        guard ShareInbox.main != nil else { return }
        claimInbox()
        guard Self.claiming.insert(ObjectIdentifier(self)).inserted else { return }
        #if canImport(UIKit)
        let foreground = UIApplication.willEnterForegroundNotification
        #else
        let foreground = NSApplication.didBecomeActiveNotification
        #endif
        NotificationCenter.default.addObserver(forName: foreground, object: nil, queue: .main) { [weak self] _ in self?.claimInbox() }
        if let id = Bundle.main.object(forInfoDictionaryKey: "ExactAppIdentifier") as? String ?? Bundle.main.bundleIdentifier {
            ShareInboxPing.shared.listen("\(id).shares", for: self)
        }
    }

    /// Each complete share in the inbox, oldest first, delivered at the
    /// navigation root; its directory is removed once its files have moved.
    func claimInbox() {
        guard booted, state != .destroyed, let root = navigationRoot, let inbox = ShareInbox.main else { return }
        for dir in inbox.complete() {
            guard var request = inbox.request(dir) else { continue }
            request["root"] = Int(root.id)
            _ = deliverShare(request)
            try? FileManager.default.removeItem(at: dir)
        }
    }
}
