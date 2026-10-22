import Foundation

/// The core supplies this narrow projection only from a complete authoritative thread.
/// A missing card or incomplete request list cannot prove consumption.
enum T3QuestionConsumption {
    static func confirmed(_ proof: [String: Any], record: [String: Any]) throws -> Bool {
        try T3QuestionCommands.fields(proof, required: ["complete", "threadId", "requests", "latestItemId"])
        guard T3QuestionCommands.bool(proof["complete"]) == true,
              proof["threadId"] as? String == record["threadId"] as? String,
              let requests = proof["requests"] as? [[String: Any]], requests.count <= 10_000,
              let latest = proof["latestItemId"] as? String else { throw T3QuestionCommands.bad("A complete matching thread projection is required for request retirement.") }
        for request in requests {
            try T3QuestionCommands.fields(request, required: ["id", "status"])
            _ = try T3QuestionCommands.string(request["id"])
            guard ["pending", "resolved", "expired", "cancelled"].contains(request["status"] as? String ?? "") else { throw T3QuestionCommands.bad() }
        }
        guard Set(requests.compactMap { $0["id"] as? String }).count == requests.count else { throw T3QuestionCommands.bad() }
        guard let pending = requests.first(where: { $0["id"] as? String == record["requestId"] as? String }) else { return true }
        if pending["status"] as? String != "pending" { return true }
        // A complete catalog's later request item proves a reused request ID has a new incarnation.
        guard !latest.isEmpty, let signature = record["signature"] as? String,
              let identity = try? JSONSerialization.jsonObject(with: Data(signature.utf8)) as? [Any],
              let original = identity.first as? String, !original.isEmpty else { return false }
        return latest != original
    }
    static func stopped(_ proof: [String: Any], record: [String: Any]) throws -> Bool {
        try T3QuestionCommands.fields(proof, required: ["complete", "threadId", "runs"])
        guard T3QuestionCommands.bool(proof["complete"]) == true, proof["threadId"] as? String == record["threadId"] as? String,
              let runs = proof["runs"] as? [[String: Any]], runs.count <= 10_000 else { throw T3QuestionCommands.bad() }
        for run in runs {
            try T3QuestionCommands.fields(run, required: ["id", "status"]); _ = try T3QuestionCommands.string(run["id"])
            guard ["preparing", "queued", "starting", "running", "waiting", "completed", "interrupted", "failed", "cancelled", "rolled_back"].contains(run["status"] as? String ?? "") else { throw T3QuestionCommands.bad() }
        }
        guard Set(runs.compactMap { $0["id"] as? String }).count == runs.count else { throw T3QuestionCommands.bad() }
        let payload = try T3QuestionCommands.payload(record)
        return !runs.contains { $0["id"] as? String == payload["runId"] as? String && ["preparing", "starting", "running", "waiting"].contains($0["status"] as? String ?? "") }
    }
}
