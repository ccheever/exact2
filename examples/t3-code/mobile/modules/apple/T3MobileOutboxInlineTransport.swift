#if os(iOS)
// @ref llp/1109.005-composer-and-transcript.decision.md#queued-command-construction
import Foundation

extension T3Transport {
    /// Validate on this transport's serial queue before capture and again before publication.
    private func inlineEndpoint(_ request: [String: Any]) throws -> (String, String) {
        guard T3OutboxDeliveryReceipt.integer(request["generation"]),
              let expected = request["generation"] as? Int,
              let home = request["expectedOrigin"] as? String,
              let canonical = try? T3Endpoint.origin(home), canonical.absoluteString == home,
              let environment = request["expectedEnvironmentId"] as? String, !environment.isEmpty,
              environment == environment.trimmingCharacters(in: .whitespacesAndNewlines) else {
            throw arguments("Choose the captured connection for these queued images.")
        }
        let current = routes.home.isEmpty ? origin?.absoluteString ?? "" : routes.home
        guard alive, state == "connected", generation == expected,
              descriptor["environmentId"] as? String == environment,
              (try? T3Endpoint.origin(current)) == canonical else {
            throw T3Failure(kind: "stale", message: "The queued image connection changed before admission.")
        }
        return (home, environment)
    }

    func outboxInline(_ request: [String: Any], completion: @escaping Completion) throws {
        guard let queuedEdits else { throw arguments("Queued images are unavailable.") }
        if request["action"] as? String == "send" { return try sendOutboxInline(request, completion: completion) }
        guard request["action"] as? String == "reserve" else {
            throw arguments("Unknown queued image operation.")
        }
        let (home, environment) = try inlineEndpoint(request)
        switch try queuedEdits.beginOutboxInlineReservation(request, origin: home, environment: environment) {
        case .existing(let value): finish(completion, value: value)
        case .preparing(let token):
            // The coordinator retains identities and byte ownership, never this completion.
            DispatchQueue.global(qos: .userInitiated).async { [self, queuedEdits] in
                let captured = Result { try queuedEdits.captureOutboxInlineReservation(token) }
                queue.async { [self, queuedEdits] in
                    defer { queuedEdits.cancelOutboxInlineReservation(token) }
                    do {
                        try captured.get()
                        _ = try inlineEndpoint(request)
                        let value = try queuedEdits.finishOutboxInlineReservation(token)
                        finish(completion, value: value)
                    } catch { finish(completion, failure: failure(error)) }
                }
            }
        }
    }

    private func sendOutboxInline(_ request: [String: Any], completion: @escaping Completion) throws {
        guard let queuedEdits else { throw arguments("Queued images are unavailable.") }
        let (home, environment) = try inlineEndpoint(request)
        switch try queuedEdits.beginOutboxInlineSend(request, origin: home, environment: environment) {
        case .acknowledged(let value): finish(completion, value: value)
        case .preparing(let token):
            let memory: T3InlineMemoryLease
            do { memory = try reserveInlineMemory() }
            catch { queuedEdits.cancelOutboxInlineSend(token); throw error }
            let id = nextID(), epoch = generation
            DispatchQueue.global(qos: .userInitiated).async { [self, queuedEdits] in
                defer { withExtendedLifetime(memory) {} }
                let captured = Result {
                    let prepared = try queuedEdits.expandOutboxInlineSend(token)
                    return try Self.encodeInlineRPC(prepared, memory: memory, id: id, epoch: epoch)
                }
                queue.async { [self, queuedEdits] in
                    defer { queuedEdits.cancelOutboxInlineSend(token) }
                    do {
                        let frame = try captured.get()
                        _ = try inlineEndpoint(request)
                        try checkInlineRPC(frame)
                        let operation = try queuedEdits.issueOutboxInlineSend(frame.prepared)
                        let operationID = operation["operationId"] as! String
                        let revision = operation["revision"] as! Int
                        do {
                            // No callback holds the expanded payload. The FIFO owns only the encoded frame.
                            try rpc(["method": "assets.persistChatAttachments"], journal: operationID, inline: frame) { [self, queuedEdits] result in
                                do {
                                    let saved = try queuedEdits.settleOutboxInline(operationID, issuedRevision: revision, result: result)
                                    finish(completion, value: saved)
                                } catch {
                                    finish(completion, failure: T3Failure(kind: "Persistence", message:
                                        "The queued image outcome needs recovery before another attempt.", uncertain: true))
                                }
                            }
                        } catch {
                            do {
                                let saved = try queuedEdits.settleOutboxInline(operationID, issuedRevision: revision,
                                    result: ["ok": false, "error": failure(error).json], knownUnsent: true)
                                finish(completion, value: saved)
                            } catch {
                                finish(completion, failure: T3Failure(kind: "Persistence", message:
                                    "The queued image admission outcome needs recovery.", uncertain: true))
                            }
                        }
                    } catch { finish(completion, failure: failure(error)) }
                }
            }
        }
    }
}
#endif
