// App-owned invocation bridge; no reply, promise, timer or native work is retained.
// @ref llp/1109.005-composer-and-transcript.decision.md#composer-focus-restoration
import Foundation

struct T3ComposerOperationFailure: Error {
    let kind: String
    let message: String
    init(_ kind: String, _ message: String) { self.kind = kind; self.message = message }
}

struct T3ComposerApplyRequest: Decodable {
    let op: String
    let generation: Int
    let identity: T3ComposerIdentity
    let command: T3ComposerReplace
    init(_ object: [String: Any]) throws {
        do {
            self = try JSONDecoder().decode(Self.self, from: JSONSerialization.data(withJSONObject: object))
            guard op == "composerEditorApply", generation >= 0, generation <= T3ComposerProtocolState.maxCount,
                  identity.admitted, !identity.mountId.isEmpty else { throw T3ComposerProtocolError.invalid("Invalid editor invocation") }
            try command.validate()
        } catch { throw T3ComposerOperationFailure("arguments", "The editor replacement request is invalid.") }
    }
}

struct T3ComposerVoiceCapture: Codable, Equatable {
    let identity: T3ComposerIdentity
    let eventCount: Int
    let sourceRevision: Int
    var json: [String: Any] { ["identity": identity.json, "eventCount": eventCount, "sourceRevision": sourceRevision] }
    static func revision(_ input: Any?) throws -> Int? {
        guard let input else { return nil }
        struct Revision: Decodable { let value: Int }
        guard let bytes = try? JSONSerialization.data(withJSONObject: ["value": input]),
              let value = try? JSONDecoder().decode(Revision.self, from: bytes).value,
              value >= 0, value <= T3ComposerProtocolState.maxCount else {
            throw T3ComposerOperationFailure("arguments", "The source document revision is invalid.")
        }
        return value
    }
}

extension T3ComposerIdentity {
    var json: [String: Any] { ["owner": owner, "editorId": editorId, "routeVisit": routeVisit, "renderEpoch": renderEpoch, "mountId": mountId] }
}
extension T3ComposerReplace {
    func sameRequest(_ other: Self) -> Bool {
        commandId == other.commandId && commandRevision == other.commandRevision
            && expected.eventCount == other.expected.eventCount && expected.value == other.expected.value && expected.selection == other.expected.selection
            && next.value == other.next.value && next.selection == other.next.selection && next.tokensJson == other.next.tokensJson
    }
}

#if os(iOS)
import UIKit

final class T3MobileComposerOperations {
    private struct Entry { weak var port: T3MobileComposerEditor? }
    private var entries: [ObjectIdentifier: Entry] = [:]
    private var alive = true
    func register(_ port: T3MobileComposerEditor) {
        guard alive else { return }
        entries = entries.filter { $0.value.port != nil }
        entries[ObjectIdentifier(port)] = Entry(port: port)
    }
    func unregister(_ port: T3MobileComposerEditor) { entries.removeValue(forKey: ObjectIdentifier(port)) }
    func perform(_ object: [String: Any]) -> [String: Any] {
        let generation = (try? T3ComposerVoiceCapture.revision(object["generation"])) ?? 0
        do {
            let request = try T3ComposerApplyRequest(object)
            guard alive else { throw T3ComposerOperationFailure("superseded", "The editor session was closed.") }
            let matches = entries.values.compactMap(\.port).filter { $0.composerIdentity == request.identity }
            guard matches.count == 1, let port = matches.first else {
                throw T3ComposerOperationFailure("superseded", "The captured editor is no longer mounted.")
            }
            let event = try port.applyInvocation(identity: request.identity, command: request.command)
            return ["ok": true, "generation": request.generation, "value": ["event": event]]
        } catch {
            let failure = error as? T3ComposerOperationFailure ?? .init("arguments", "The editor replacement request is invalid.")
            return ["ok": false, "generation": generation, "error": ["kind": failure.kind, "message": failure.message]]
        }
    }
    func destroy() { alive = false; entries.removeAll() }
}
#endif
