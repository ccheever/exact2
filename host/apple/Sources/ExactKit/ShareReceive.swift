// A received share's claim (LLP 1133.024 D2–D4): the Rust host claims it,
// sets its tokens, delivers its location at the navigation root and lands
// its files in `app:/tmp/shared/`. Under the agent the driver's `share:`
// form arrives here (D9); a host never takes `share:` from outside it.
import Foundation

extension ExactSession {
    /// The agent's `share` op: `{"id": <navigation root>, "target", "share"}`.
    func receiveShare(_ req: [String: Any]) -> [String: Any] {
        guard booted, let root = navigationRoot else { return ["error": "share: the app has no navigation root to deliver it to"] }
        var request = req.filter { ["target", "share"].contains($0.key) }
        request["root"] = Int(root.id)
        guard let data = try? JSONSerialization.data(withJSONObject: request) else { return ["error": "share: the share is malformed"] }
        let batch = runtime.share(String(decoding: data, as: UTF8.self), now: now())
        apply(batch)
        if let error = batch.error { return ["error": error] }
        return ["typed": Int(root.id), "delivery": "recognized"]
    }

    /// The driver's cold share (`EXACT_AGENT_SHARE`), claimed before the
    /// boot: the boot takes its location and tokens.
    func claimColdShare() {
        guard !booted, ExactEnv.agentMode, let json = ExactEnv.environment["EXACT_AGENT_SHARE"] else { return }
        _ = runtime.share(json, now: 0)
    }

    /// Its files, once activation has configured storage.
    func landColdShare() {
        guard ExactEnv.agentMode, ExactEnv.environment["EXACT_AGENT_SHARE"] != nil else { return }
        apply(runtime.landShare())
    }
}
