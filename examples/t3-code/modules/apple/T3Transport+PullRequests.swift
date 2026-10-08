// T3Transport's pull request ops (T3Transport.perform routes them; 20261005-pr-code-tab): the one
// HTTP read the client sends with a body, `POST /api/pull-requests/diff` (PullRequestDiffInput;
// the reference's client gives it 60 s, packages/client-runtime/src/state/pullRequestDiffHttp.ts).
// The client HTTP interface stays read-only (`http`): this op takes the request's fields, never a
// path or a body, rebuilds the body from the allow-listed ones, and the bearer stays here. A read
// identical to one still out joins it (an answer Exact asked again before the reply sends nothing
// new, as a shared RPC read does). Runs on the transport's queue.
import Foundation

extension T3Transport {
    /// Pull request ops; false for an op that is not one of them.
    func pullRequestOps(_ request: [String: Any], completion: @escaping Completion) throws -> Bool {
        switch request["op"] as? String {
        case "prDiff": try pullRequestDiff(request, completion: completion)
        default: return false
        }
        return true
    }

    /// PullRequestDiffInput's fields and nothing else: the reference (projectId, host, repository,
    /// number), and where to carry on from (`cursor`) or which commit (`commit`).
    static func pullRequestDiffBody(_ payload: Any?) throws -> Data {
        let refused = T3Failure(kind: "Arguments", message: "prDiff takes projectId, host, repository, number, cursor and commit.")
        guard let input = payload as? [String: Any], Set(input.keys).isSubset(of: ["projectId", "host", "repository", "number", "cursor", "commit"]) else { throw refused }
        func text(_ key: String, required: Bool) throws -> String? {
            guard let value = input[key] else { if required { throw refused }; return nil }
            guard let string = value as? String, !string.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty, string.utf8.count <= 4096 else { throw refused }
            return string
        }
        guard let number = input["number"] as? Int, !(input["number"] is Bool), number > 0 else { throw refused }
        var body: [String: Any] = ["projectId": try text("projectId", required: true)!, "repository": try text("repository", required: true)!, "number": number]
        for key in ["host", "cursor", "commit"] { if let value = try text(key, required: false) { body[key] = value } }
        return try JSONSerialization.data(withJSONObject: body, options: [.sortedKeys])
    }

    func pullRequestDiff(_ request: [String: Any], completion: @escaping Completion) throws {
        guard state == "connected" else { throw T3Failure(kind: "Disconnected", message: "The server is not connected.") }
        if let expected = request["generation"] as? Int, expected != generation {
            throw T3Failure(kind: "stale", message: "The connection changed before this operation was sent.")
        }
        let body = try Self.pullRequestDiffBody(request["payload"])
        let key = "\(generation)\n" + (String(data: body, encoding: .utf8) ?? "")
        if pendingPullRequestDiffs[key] != nil { pendingPullRequestDiffs[key]?.append(completion); return }
        pendingPullRequestDiffs[key] = [completion]
        http(path: "/api/pull-requests/diff", method: "POST", raw: body, epoch: generation, long: true) { [self] result in
            for waiter in pendingPullRequestDiffs.removeValue(forKey: key) ?? [] {
                switch result {
                case .success(let value): finish(waiter, value: value)
                case .failure(let error): finish(waiter, failure: error)
                }
            }
        }
    }

    /// The diff's session: this transport's configuration (its protocol classes, no cookies, no cache)
    /// with the reference's 60 s in place of the 15 s / 30 s every other request keeps.
    func longLived() -> URLSession {
        if let made = longSession { return made }
        let config = session.configuration
        config.timeoutIntervalForRequest = 60
        config.timeoutIntervalForResource = 60
        let made = URLSession(configuration: config, delegate: self, delegateQueue: nil)
        longSession = made
        return made
    }
}
