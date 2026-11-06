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
    var independentDraft: T3ComposerIndependentDraft? { try? JSONDecoder().decode(T3ComposerIndependentDraft.self, from: Data(owner.utf8)) }
    var independentNewTask: Bool { independentDraft != nil }
}
struct T3ComposerIndependentDraft: Decodable, Equatable {
    let origin: String
    let environment: String
    let key: String
    let incarnation: String
    init(from decoder: Decoder) throws {
        var fields = try decoder.unkeyedContainer()
        let kind = try fields.decode(String.self); origin = try fields.decode(String.self)
        environment = try fields.decode(String.self)
        let generation = try fields.decode(Int.self)
        key = try fields.decode(String.self); incarnation = try fields.decode(String.self)
        guard fields.isAtEnd, kind == "ordinary", !origin.isEmpty, !environment.isEmpty,
              generation >= 0, generation <= T3ComposerProtocolState.maxCount,
              key.range(of: "^new-task:[A-Za-z0-9_-]{1,128}$", options: .regularExpression) != nil,
              !key.hasPrefix("new-task:pending-"), !incarnation.isEmpty else {
            throw T3ComposerProtocolError.invalid("Invalid independent draft owner")
        }
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

struct T3ComposerHandoffRequest {
    struct Operation: Equatable { let id: String; let revision: Int }
    enum Action {
        case freeze(retireUndo: Bool)
        case resume(receiptId: String, document: T3ComposerDocument)
    }
    let generation: Int
    let identity: T3ComposerIdentity
    let operation: Operation
    let action: Action
    init(_ object: [String: Any]) throws {
        struct Wire: Decodable {
            let op: String; let generation: Int; let identity: T3ComposerIdentity
            let action: String; let operationId: String; let operationRevision: Int
            let retireUndo: Bool?; let receiptId: String?; let document: T3ComposerDocument?
        }
        do {
            let wire = try JSONDecoder().decode(Wire.self, from: JSONSerialization.data(withJSONObject: object))
            guard wire.op == "composerEditorHandoff", wire.generation >= 0, wire.generation <= T3ComposerProtocolState.maxCount,
                  wire.identity.admitted, !wire.identity.mountId.isEmpty, wire.identity.independentNewTask,
                  UUID(uuidString: wire.operationId) != nil, wire.operationRevision >= 0, wire.operationRevision <= T3ComposerProtocolState.maxCount else {
                throw T3ComposerProtocolError.invalid("Invalid handoff identity")
            }
            generation = wire.generation; identity = wire.identity
            operation = .init(id: wire.operationId, revision: wire.operationRevision)
            switch wire.action {
            case "freeze":
                guard let retireUndo = wire.retireUndo, wire.receiptId == nil, wire.document == nil else {
                    throw T3ComposerProtocolError.invalid("Invalid freeze request")
                }
                action = .freeze(retireUndo: retireUndo)
            case "resume":
                guard let receiptId = wire.receiptId, UUID(uuidString: receiptId) != nil, wire.retireUndo == nil,
                      let document = wire.document, document.selection != nil, !document.isNativeEcho else {
                    throw T3ComposerProtocolError.invalid("Invalid resume request")
                }
                try document.validate()
                action = .resume(receiptId: receiptId, document: document)
            default: throw T3ComposerProtocolError.invalid("Invalid handoff action")
            }
        } catch { throw T3ComposerOperationFailure("arguments", "The editor handoff request is invalid.") }
    }
}

final class T3ComposerFinalizations {
    struct Receipt {
        let id: String
        let identity: T3ComposerIdentity
        let event: [String: Any]
        var retired = false
        var json: [String: Any] { ["receiptId": id, "identity": identity.json, "event": event, "retired": retired] }
    }
    private enum Phase {
        case editing
        case frozen(Receipt)
        case saved(String)
        case retired(Receipt)
        var pending: Receipt? {
            switch self { case .frozen(let receipt), .retired(let receipt): return receipt; default: return nil }
        }
    }
    private struct Slot { let identity: T3ComposerIdentity; let draft: T3ComposerIndependentDraft; var phase: Phase = .editing }
    private var slots: [Slot] = []
    static let capacity = 8
    var pending: [Receipt] { slots.compactMap { $0.phase.pending } }
    var reserved: [T3ComposerIdentity] { slots.map(\.identity) }
    func reserve(_ identity: T3ComposerIdentity) throws {
        guard let draft = identity.independentDraft else { return }
        guard identity.admitted, !identity.mountId.isEmpty else {
            throw T3ComposerOperationFailure("arguments", "The independent editor identity is invalid.")
        }
        if slots.contains(where: { if case .editing = $0.phase { return $0.identity == identity }; return false }) { return }
        guard slots.count < Self.capacity, !slots.contains(where: { $0.draft == draft || $0.identity.mountId == identity.mountId }) else {
            throw T3ComposerOperationFailure("busy", "Save the previous editor source before opening another editor.")
        }
        slots.append(.init(identity: identity, draft: draft))
    }
    func freeze(_ identity: T3ComposerIdentity, event: [String: Any]) throws -> Receipt {
        guard let index = slots.firstIndex(where: { $0.identity == identity }), case .editing = slots[index].phase else {
            throw T3ComposerOperationFailure("busy", "The editor source is already held.")
        }
        let receipt = Receipt(id: UUID().uuidString, identity: identity, event: event)
        slots[index].phase = .frozen(receipt)
        return receipt
    }
    func resume(_ identity: T3ComposerIdentity, receiptId: String) throws {
        guard let index = slots.firstIndex(where: { $0.identity == identity }), case .saved(let savedId) = slots[index].phase, savedId == receiptId else {
            throw T3ComposerOperationFailure("busy", "Save the held editor source before resuming.")
        }
        slots[index].phase = .editing
    }
    @discardableResult func retire(_ identity: T3ComposerIdentity, event: [String: Any]?) -> Bool {
        guard let index = slots.firstIndex(where: { $0.identity == identity }) else { return false }
        switch slots[index].phase {
        case .editing:
            guard let event else { slots.remove(at: index); return false }
            slots[index].phase = .retired(.init(id: UUID().uuidString, identity: identity, event: event, retired: true))
        case .frozen(var receipt):
            receipt.retired = true; slots[index].phase = .retired(receipt)
        case .saved: slots.remove(at: index)
        case .retired: return false
        }
        return true
    }
    @discardableResult func acknowledge(_ id: String) -> Bool {
        guard let index = slots.firstIndex(where: { $0.phase.pending?.id == id }) else { return false }
        switch slots[index].phase {
        case .frozen: slots[index].phase = .saved(id)
        case .retired: slots.remove(at: index)
        default: return false
        }
        return true
    }
    func destroy() { slots.removeAll() }
}

#if os(iOS)
import UIKit

final class T3MobileComposerOperations {
    private struct Entry { weak var port: T3MobileComposerEditor? }
    private struct SourceCustody { let claim: T3ComposerFileClaim; let owner: T3MobileComposerFileHolds }
    private var entries: [ObjectIdentifier: Entry] = [:]
    private let finalizations = T3ComposerFinalizations()
    private var sourceCustody: [String: SourceCustody] = [:]
    private let changed: () -> Void
    private var alive = true
    init(changed: @escaping () -> Void = {}) { self.changed = changed }
    func register(_ port: T3MobileComposerEditor) throws {
        guard alive else { throw T3ComposerOperationFailure("superseded", "The editor session was closed.") }
        if let identity = port.composerIdentity { try finalizations.reserve(identity) }
        entries = entries.filter { $0.value.port != nil }
        entries[ObjectIdentifier(port)] = Entry(port: port)
    }
    func unregister(_ port: T3MobileComposerEditor, finalEvent: [String: Any]?, fileHolds: T3MobileComposerFileHolds? = nil) {
        defer { fileHolds?.unregister(port) }
        guard entries.removeValue(forKey: ObjectIdentifier(port)) != nil, let identity = port.composerIdentity else { return }
        let retired = finalizations.retire(identity, event: finalEvent)
        if let receipt = finalizations.pending.first(where: { $0.identity == identity && $0.retired }),
           let fileHolds, let claim = fileHolds.detach(port) {
            sourceCustody[receipt.id] = SourceCustody(claim: claim, owner: fileHolds)
        }
        if retired { changed() }
    }
    func freeze(_ port: T3MobileComposerEditor, event: [String: Any]) throws -> T3ComposerFinalizations.Receipt {
        guard alive, entries[ObjectIdentifier(port)]?.port === port, let identity = port.composerIdentity else {
            throw T3ComposerOperationFailure("superseded", "The captured editor is no longer mounted.")
        }
        let receipt = try finalizations.freeze(identity, event: event)
        changed()
        return receipt
    }
    func resume(_ port: T3MobileComposerEditor, receiptId: String) throws {
        guard alive, entries[ObjectIdentifier(port)]?.port === port, let identity = port.composerIdentity else {
            throw T3ComposerOperationFailure("superseded", "The captured editor is no longer mounted.")
        }
        try finalizations.resume(identity, receiptId: receiptId)
    }
    func performHandoff(_ object: [String: Any]) -> [String: Any] {
        let generation = (try? T3ComposerVoiceCapture.revision(object["generation"])) ?? 0
        do {
            let request = try T3ComposerHandoffRequest(object)
            guard alive else { throw T3ComposerOperationFailure("superseded", "The editor session was closed.") }
            let matches = entries.values.compactMap(\.port).filter { $0.composerIdentity == request.identity }
            guard matches.count == 1, let port = matches.first else {
                throw T3ComposerOperationFailure("superseded", "The captured editor is no longer mounted.")
            }
            return ["ok": true, "generation": request.generation, "value": try port.handoff(request)]
        } catch {
            let failure = error as? T3ComposerOperationFailure ?? .init("arguments", "The editor handoff request is invalid.")
            return ["ok": false, "generation": generation, "error": ["kind": failure.kind, "message": failure.message]]
        }
    }
    func performFinalizations(_ object: [String: Any]) -> [String: Any] {
        let generation = (try? T3ComposerVoiceCapture.revision(object["generation"])) ?? 0
        do {
            let request = try T3ComposerFinalizationRequest(object)
            guard alive else { throw T3ComposerOperationFailure("superseded", "The editor session was closed.") }
            let value: [String: Any]
            switch request.action {
            case .read: value = ["receipts": finalizations.pending.map(\.json), "reserved": finalizations.reserved.map(\.json)]
            case .acknowledge:
                let acknowledged = finalizations.acknowledge(request.receiptId!)
                value = ["acknowledged": acknowledged]
                if acknowledged {
                    if let custody = sourceCustody.removeValue(forKey: request.receiptId!) { custody.owner.retire(custody.claim) }
                    changed()
                }
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
    func destroy() {
        alive = false; entries.removeAll(); finalizations.destroy()
        let custody = sourceCustody.values; sourceCustody.removeAll()
        for source in custody { source.owner.retire(source.claim) }
    }
}
#endif
