// @ref examples/t3-code/mobile/docs/1130.005-composer-and-transcript.decision.md#composer-focus-restoration
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
    var independentNewTask: Bool {
        struct Owner: Decodable {
            init(from decoder: Decoder) throws {
                var fields = try decoder.unkeyedContainer()
                let kind = try fields.decode(String.self), origin = try fields.decode(String.self)
                let environment = try fields.decode(String.self), generation = try fields.decode(Int.self)
                let key = try fields.decode(String.self), incarnation = try fields.decode(String.self)
                guard fields.isAtEnd, kind == "ordinary", !origin.isEmpty, !environment.isEmpty,
                      generation >= 0, generation <= T3ComposerProtocolState.maxCount,
                      key.range(of: "^new-task:[A-Za-z0-9_-]{1,128}$", options: .regularExpression) != nil,
                      !key.hasPrefix("new-task:pending-"), !incarnation.isEmpty else {
                    throw T3ComposerProtocolError.invalid("Invalid independent draft owner")
                }
            }
        }
        return (try? JSONDecoder().decode(Owner.self, from: Data(owner.utf8))) != nil
    }
}
extension T3ComposerReplace {
    func sameRequest(_ other: Self) -> Bool {
        commandId == other.commandId && commandRevision == other.commandRevision
            && expected.eventCount == other.expected.eventCount && expected.value == other.expected.value && expected.selection == other.expected.selection
            && next.value == other.next.value && next.selection == other.next.selection && next.tokensJson == other.next.tokensJson
    }
}

struct T3ComposerFinalizationRequest: Decodable {
    enum Action: String, Decodable { case read, acknowledge }
    let op: String
    let generation: Int
    let action: Action
    let receiptId: String?
    init(_ object: [String: Any]) throws {
        do {
            self = try JSONDecoder().decode(Self.self, from: JSONSerialization.data(withJSONObject: object))
            guard op == "composerEditorFinalizations", generation >= 0, generation <= T3ComposerProtocolState.maxCount,
                  action == .read ? receiptId == nil : UUID(uuidString: receiptId ?? "") != nil else {
                throw T3ComposerProtocolError.invalid("Invalid final source request")
            }
        } catch { throw T3ComposerOperationFailure("arguments", "The final editor source request is invalid.") }
    }
}

final class T3ComposerFinalizations {
    struct Receipt {
        let id: String
        let identity: T3ComposerIdentity
        let event: [String: Any]
        var json: [String: Any] { ["receiptId": id, "identity": identity.json, "event": event] }
    }
    private struct Slot { let identity: T3ComposerIdentity; var receipt: Receipt? }
    private var slots: [Slot] = []
    static let capacity = 8
    var pending: [Receipt] { slots.compactMap(\.receipt) }
    func reserve(_ identity: T3ComposerIdentity) throws {
        guard identity.independentNewTask else { return }
        guard identity.admitted, !identity.mountId.isEmpty else {
            throw T3ComposerOperationFailure("arguments", "The independent editor identity is invalid.")
        }
        if slots.contains(where: { $0.identity == identity && $0.receipt == nil }) { return }
        guard slots.count < Self.capacity, !slots.contains(where: { $0.identity.owner == identity.owner || $0.identity.mountId == identity.mountId }) else {
            throw T3ComposerOperationFailure("busy", "Save the previous editor source before opening another editor.")
        }
        slots.append(.init(identity: identity))
    }
    @discardableResult func retire(_ identity: T3ComposerIdentity, event: [String: Any]?) -> Bool {
        guard let index = slots.firstIndex(where: { $0.identity == identity }), slots[index].receipt == nil else { return false }
        guard let event else { slots.remove(at: index); return false }
        slots[index].receipt = .init(id: UUID().uuidString, identity: identity, event: event)
        return true
    }
    @discardableResult func acknowledge(_ id: String) -> Bool {
        guard let index = slots.firstIndex(where: { $0.receipt?.id == id }) else { return false }
        slots.remove(at: index)
        return true
    }
    func destroy() { slots.removeAll() }
}

#if os(iOS)
import UIKit

final class T3MobileComposerOperations {
    private struct Entry { weak var port: T3MobileComposerEditor? }
    private var entries: [ObjectIdentifier: Entry] = [:]
    private let finalizations = T3ComposerFinalizations()
    private let changed: () -> Void
    private var alive = true
    init(changed: @escaping () -> Void = {}) { self.changed = changed }
    func register(_ port: T3MobileComposerEditor) throws {
        guard alive else { throw T3ComposerOperationFailure("superseded", "The editor session was closed.") }
        if let identity = port.composerIdentity { try finalizations.reserve(identity) }
        entries = entries.filter { $0.value.port != nil }
        entries[ObjectIdentifier(port)] = Entry(port: port)
    }
    func unregister(_ port: T3MobileComposerEditor, finalEvent: [String: Any]?) {
        guard entries.removeValue(forKey: ObjectIdentifier(port)) != nil, let identity = port.composerIdentity else { return }
        if finalizations.retire(identity, event: finalEvent) { changed() }
    }
    func performFinalizations(_ object: [String: Any]) -> [String: Any] {
        let generation = (try? T3ComposerVoiceCapture.revision(object["generation"])) ?? 0
        do {
            let request = try T3ComposerFinalizationRequest(object)
            guard alive else { throw T3ComposerOperationFailure("superseded", "The editor session was closed.") }
            let value: [String: Any]
            switch request.action {
            case .read: value = ["receipts": finalizations.pending.map(\.json)]
            case .acknowledge:
                let acknowledged = finalizations.acknowledge(request.receiptId!)
                value = ["acknowledged": acknowledged]
                if acknowledged { changed() }
            }
            return ["ok": true, "generation": request.generation, "value": value]
        } catch {
            let failure = error as? T3ComposerOperationFailure ?? .init("arguments", "The final editor source request is invalid.")
            return ["ok": false, "generation": generation, "error": ["kind": failure.kind, "message": failure.message]]
        }
    }
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
    func destroy() { alive = false; entries.removeAll(); finalizations.destroy() }
}
#endif
