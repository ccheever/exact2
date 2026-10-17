#if os(iOS)
// @ref llp/1109.005-composer-and-transcript.decision.md#scratch-tasks-and-queue-boundaries
import Foundation

extension T3Transport {
    /// Called only on transport.queue. Local editor CAS never depends on selected transport.
    func queuedEdit(_ request: [String: Any], completion: @escaping Completion) throws {
        guard let queuedEdits else { throw arguments("Queued editing is unavailable.") }
        let environment = descriptor["environmentId"] as? String ?? ""
        let home = routes.home.isEmpty ? origin?.absoluteString ?? "" : routes.home
        guard state == "connected", let expected = request["generation"] as? Int, expected == generation,
              request["expectedEnvironmentId"] as? String == environment, !environment.isEmpty, !home.isEmpty else {
            throw T3Failure(kind: "stale", message: "The queued edit connection changed before admission.")
        }
        switch request["action"] as? String {
        case "reserve":
            finish(completion, value: try queuedEdits.reserve(request, origin: home, environment: environment, activeOrigin: origin?.absoluteString ?? ""))
        case "send":
            guard let id = request["operationId"] as? String, let revision = request["revision"] as? Int else { throw arguments("Choose a saved queued operation.") }
            let operation = try queuedEdits.beginSend(id, revision: revision, origin: home, environment: environment)
            if operation["state"] as? String == "acknowledged" {
                return finish(completion, value: ["operation": operation, "result": operation["result"] ?? NSNull()])
            }
            do {
                try rpc(["method": operation["method"]!, "payload": operation["payload"]!], journal: id) { [self] result in
                    do {
                        let saved = try queuedEdits.settle(id, result: result)
                        if result["ok"] as? Bool == true {
                            finish(completion, value: ["operation": saved, "result": result["value"] ?? NSNull()])
                        } else {
                            let error = result["error"] as? [String: Any] ?? [:]
                            finish(completion, failure: T3Failure(kind: error["kind"] as? String ?? "QueuedEdit",
                                message: error["message"] as? String ?? "The queued update was not confirmed.", uncertain: saved["state"] as? String == "uncertain"))
                        }
                    } catch {
                        finish(completion, failure: T3Failure(kind: "Persistence", message: "The queued update outcome could not be saved. Resolve the saved operation before retrying.", uncertain: true))
                    }
                }
            } catch {
                // rpc throws only before pending insertion/enqueue. Persist that known-unsent fact.
                do { _ = try queuedEdits.settle(id, result: ["ok": false, "error": failure(error).json], knownUnsent: true) }
                catch { return finish(completion, failure: T3Failure(kind: "Persistence", message: "The queued operation could not be restored after admission failed.", uncertain: true)) }
                throw error
            }
        default: throw arguments("Unknown queued transport operation.")
        }
    }
}
#endif
