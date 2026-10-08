#if os(iOS)
// @ref llp/1109.005-composer-and-transcript.decision.md#queued-command-construction
import Foundation

extension T3Transport {
    /// Actual pinned RpcAuthorization rejects these before the handler. A generic
    /// orchestration error can follow a commit, even inside an Effect typed Fail.
    static func outboxNotAccepted(_ exit: [String: Any]) -> Bool {
        guard let cause = exit["cause"] as? [[String: Any]], !cause.isEmpty else { return false }
        return cause.allSatisfy { entry in
            entry["_tag"] as? String == "Fail" &&
                (entry["error"] as? [String: Any])?["_tag"] as? String == "EnvironmentAuthorizationError"
        }
    }

    /// Runs on the transport queue. A saved command never borrows the foreground endpoint.
    func outboxDelivery(_ request: [String: Any], completion: @escaping Completion) throws {
        guard let queuedEdits else { throw arguments("Queued delivery is unavailable.") }
        guard T3OutboxDeliveryReceipt.integer(request["generation"]),
              let expectedGeneration = request["generation"] as? Int,
              let expectedOrigin = request["expectedOrigin"] as? String,
              let canonical = try? T3Endpoint.origin(expectedOrigin), canonical.absoluteString == expectedOrigin,
              let environment = request["expectedEnvironmentId"] as? String, !environment.isEmpty,
              environment == environment.trimmingCharacters(in: .whitespacesAndNewlines) else {
            throw arguments("Choose the captured connection for this queued command.")
        }
        let home = routes.home.isEmpty ? origin?.absoluteString ?? "" : routes.home
        guard state == "connected", generation == expectedGeneration,
              descriptor["environmentId"] as? String == environment,
              (try? T3Endpoint.origin(home)) == canonical else {
            throw T3Failure(kind: "stale", message: "The queued command connection changed before admission.")
        }
        switch request["action"] as? String {
        case "reserve":
            finish(completion, value: try queuedEdits.reserveOutboxDelivery(request, origin: expectedOrigin, environment: environment))
        case "send":
            let operation = try queuedEdits.beginOutboxDelivery(request, origin: expectedOrigin, environment: environment)
            if operation["state"] as? String == "acknowledged" {
                return finish(completion, value: ["operation": operation, "durable": true])
            }
            let id = operation["operationId"] as! String, revision = operation["revision"] as! Int
            do {
                try rpc(["method": operation["method"]!, "payload": operation["payload"]!], journal: id) { [self, queuedEdits] result in
                    do {
                        let saved = try queuedEdits.settleOutboxDelivery(id, issuedRevision: revision, result: result)
                        // Native completion owns settlement, independently of the screen's answer lifetime.
                        // RPC failure is a saved outcome; transport success does not mean command success.
                        finish(completion, value: ["operation": saved, "durable": true])
                    } catch {
                        finish(completion, failure: T3Failure(kind: "Persistence", message:
                            "The queued command outcome needs recovery before another attempt.", uncertain: true))
                    }
                }
            } catch {
                // rpc throws only before pending registration and frame enqueue. An older uncertain
                // attempt still belongs to the receipt; the coordinator preserves that distinction.
                do {
                    let saved = try queuedEdits.settleOutboxDelivery(id, issuedRevision: revision,
                        result: ["ok": false, "error": failure(error).json], knownUnsent: true)
                    finish(completion, value: ["operation": saved, "durable": true])
                } catch {
                    finish(completion, failure: T3Failure(kind: "Persistence", message:
                        "The queued command admission outcome needs recovery.", uncertain: true))
                }
            }
        default: throw arguments("Unknown queued delivery operation.")
        }
    }
}
#endif
