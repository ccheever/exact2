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
            throw T3Failure(kind: "stale", message: "The queued image connection changed before reservation.")
        }
        return (home, environment)
    }

    func outboxInline(_ request: [String: Any], completion: @escaping Completion) throws {
        guard request["action"] as? String == "reserve", let queuedEdits else {
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
}
#endif
