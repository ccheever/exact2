// @ref LLP 1098 D10. The media session under the driver: `state.mediaSession`
// from the video arm's coordinator (NowPlaying.swift) as each player reports
// it, and `tap <element> mediasession <action> [seconds]` through the
// command target's own path. Under the driver nothing is published
// (`published: "agent"`), so the readback is what would be.
import Foundation

extension Agent {
    /// This session's players, each with its arm's fresh report.
    private func mediaPlayers() -> [(id: UInt32, view: NodeView, report: [String: Any])] {
        presenter.views.compactMap { id, view in
            guard let video = view.video else { return nil }
            _ = video.state()
            return (id, view, video.sessionReport)
        }.sorted { $0.id < $1.id }
    }

    private static func named(_ view: NodeView) -> String {
        view.props["testId"].map { "\"\($0)\"" } ?? "view \(view.id)"
    }

    /// `state.mediaSession`: the owner when it is one of this session's
    /// players (another session's owns it otherwise, LLP 1031), the claimants
    /// by id, and the owner's report.
    func mediaSessionState() -> [String: Any] {
        let players = mediaPlayers()
        var out: [String: Any] = ["owner": NSNull(), "testId": NSNull(), "claimants": players.filter { $0.report["claimant"] as? Bool == true }.map { Int($0.id) },
                                  "metadata": NSNull(), "actions": [String](), "seekOffsets": NSNull(), "playbackState": "none", "position": NSNull(),
                                  "published": ExactEnv.agentMode ? "agent" : "MPNowPlayingInfoCenter", "readback": NSNull()]
        guard let owner = players.first(where: { $0.report["owner"] as? Bool == true }) else { return out }
        for (key, value) in owner.report where !["claimant", "owner"].contains(key) { out[key] = value }
        out["owner"] = Int(owner.id)
        out["testId"] = owner.view.props["testId"] ?? NSNull()
        return out
    }

    /// `tap <id> mediasession <action> [seconds]`: refused, with nothing
    /// dispatched, for a target that is not the owner or an action it does
    /// not offer; `delivery: substituted` (the platform did not press it).
    func mediaSessionTap(_ req: [String: Any], action: String) -> [String: Any] {
        guard let v = view(req) else { return ["error": "no view \(req["id"] ?? "?") on screen"] }
        let players = mediaPlayers()
        guard let owner = players.first(where: { $0.report["owner"] as? Bool == true }) else {
            return ["error": "mediasession: there is no media session: no `audio` or `video` with `metadata=` is mounted (tapped \(Self.named(v)))"]
        }
        guard owner.id == v.id, let video = v.video else {
            return ["error": "mediasession: \(Self.named(v)) does not own the media session; \(Self.named(owner.view)) does"]
        }
        guard (owner.report["actions"] as? [String] ?? []).contains(action) else {
            return ["error": "mediasession: \(Self.named(v)) has no \(action)"]
        }
        let seconds = (req["seconds"] as? NSNumber)?.doubleValue
        if action == "seekto", seconds == nil { return ["error": "mediasession: seekto needs the seconds to seek to"] }
        guard let status = video.remote(action, seconds: seconds) else { return ["error": "mediasession: the video arm is not loaded"] }
        guard status == "success" else { return ["error": "mediasession: \(Self.named(v)) did not take \(action) (\(status))"] }
        presenter.settlePump()
        return ["tapped": Int(v.id), "mediaSession": action, "delivery": "substituted"]
    }
}
