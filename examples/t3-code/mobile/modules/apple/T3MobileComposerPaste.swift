#if os(iOS)
// @ref llp/1109.005-composer-and-transcript.decision.md#composer-command-foundation
// Native-issued temporary image ownership. Wire URIs never authorize filesystem IO.
import Foundation

struct T3ComposerPasteTarget: Codable, Equatable {
    let origin: String
    let environmentId: String
    let draftKey: String
    let incarnation: String
    let capturedRevision: Int
    var valid: Bool { !origin.isEmpty && !environmentId.isEmpty && !draftKey.isEmpty && !incarnation.isEmpty && capturedRevision >= 0 && capturedRevision <= T3ComposerProtocolState.maxCount }
}
struct T3ComposerPasteFile: Codable, Equatable {
    let leaseId: String
    let id: String
    let kind: String
    let name: String
    let mimeType: String
    let sizeBytes: Int
    let sha256: String
}
struct T3ComposerPasteProof: Codable, Equatable {
    let version: Int
    let operationId: String
    let identity: T3ComposerIdentity
    let richEventId: String
    let target: T3ComposerPasteTarget
    let files: [T3ComposerPasteFile]
    var valid: Bool {
        version == 1 && UUID(uuidString: operationId) != nil && operationId == operationId.lowercased()
            && identity.admitted && !identity.mountId.isEmpty && UUID(uuidString: richEventId) != nil && target.valid && files.count <= 100
            && Set(files.map(\.id)).count == files.count && Set(files.map(\.leaseId)).count == files.count
            && files.allSatisfy { f in
                UUID(uuidString: f.leaseId) != nil && UUID(uuidString: f.id) != nil && f.id == f.id.lowercased()
                    && f.kind == "image" && f.name == "pasted-image.png" && f.mimeType == "image/png"
                    && f.sizeBytes > 0 && f.sizeBytes <= T3MobileAttachments.maxImageBytes
                    && f.sha256.count == 64 && f.sha256.allSatisfy { "0123456789abcdef".contains($0) }
            }
    }
}
struct T3ComposerPasteRequest: Decodable {
    struct Source: Codable, Equatable { let kind: String; let leaseId: String }
    let op: String
    let action: String
    let generation: Int
    let operationId: String
    let identity: T3ComposerIdentity?
    let richEventId: String?
    let target: T3ComposerPasteTarget?
    let sources: [Source]?
    let remaining: Int?
    let proof: T3ComposerPasteProof?
    init(_ object: [String: Any]) throws {
        do {
            self = try JSONDecoder().decode(Self.self, from: JSONSerialization.data(withJSONObject: object))
            guard op == "composerEditorPasteFiles", ["stage", "adopt", "discard", "retire"].contains(action),
                  generation >= 0, generation <= T3ComposerProtocolState.maxCount,
                  UUID(uuidString: operationId) != nil, operationId == operationId.lowercased() else { throw T3ComposerPasteError.arguments }
            if action == "stage" {
                guard let identity, identity.admitted, !identity.mountId.isEmpty, let richEventId, UUID(uuidString: richEventId) != nil,
                      let target, target.valid, let sources, sources.count <= 1024,
                      Set(sources.map(\.leaseId)).count == sources.count,
                      sources.allSatisfy({ $0.kind == "editorImage" && UUID(uuidString: $0.leaseId) != nil }),
                      let remaining, (0...100).contains(remaining), proof == nil else { throw T3ComposerPasteError.arguments }
            } else { guard let proof, proof.valid, proof.operationId == operationId else { throw T3ComposerPasteError.arguments } }
        } catch { throw T3ComposerPasteError.arguments }
    }
}
enum T3ComposerPasteError: Error {
    case arguments, superseded, recovery, capacity
    var message: String {
        switch self {
        case .arguments: return "The pasted image request does not match its captured owner."
        case .superseded: return "Those pasted images are no longer available to this editor."
        case .recovery: return "Pasted image ownership needs recovery. The saved bytes have been retained."
        case .capacity: return "Finish recovering pasted images before importing more."
        }
    }
    var kind: String { self == .arguments ? "Arguments" : self == .superseded ? "superseded" : "Persistence" }
}
struct T3ComposerPasteInput { let leaseId: String; let file: URL }

/// Main-thread only. Claimed files belong to the operation, independently of a UITextView.
final class T3MobileComposerPaste {
    private struct Lease {
        let identity: T3ComposerIdentity
        let event: String
        let file: URL
        var operation: String?
    }
    private var leases: [String: Lease] = [:]
    private var alive = true
    func issue(identity: T3ComposerIdentity, event: String, uris: [String], take: ([String]) -> Bool) throws -> [[String: String]] {
        guard alive, leases.count + uris.count <= 1024 else { throw T3ComposerPasteError.capacity }
        let files = try uris.map { uri -> URL in
            guard let url = URL(string: uri), T3MobileComposerPasteStore.ownedTemporary(url) else { throw T3ComposerPasteError.arguments }
            return url
        }
        guard take(uris) else { throw T3ComposerPasteError.superseded }
        return files.map { file in
            let id = UUID().uuidString.lowercased()
            leases[id] = Lease(identity: identity, event: event, file: file)
            return ["leaseId": id, "uri": file.absoluteString]
        }
    }
    /// A missing lease can only be a durable replay; disk admission must prove that separately.
    func claim(_ request: T3ComposerPasteRequest) throws -> [T3ComposerPasteInput]? {
        guard alive, request.action == "stage", let sources = request.sources else { throw T3ComposerPasteError.superseded }
        var result: [T3ComposerPasteInput] = []
        for source in sources {
            guard let lease = leases[source.leaseId] else { return nil }
            guard lease.identity == request.identity, lease.event == request.richEventId,
                  lease.operation == nil || lease.operation == request.operationId else { throw T3ComposerPasteError.arguments }
            result.append(.init(leaseId: source.leaseId, file: lease.file))
        }
        for source in sources { leases[source.leaseId]?.operation = request.operationId }
        return result
    }
    func completed(_ request: T3ComposerPasteRequest) {
        let ids = leases.filter { $0.value.operation == request.operationId }.map(\.key)
        for id in ids { if let lease = leases.removeValue(forKey: id) { try? FileManager.default.removeItem(at: lease.file) } }
    }
    func retire(_ identity: T3ComposerIdentity) {
        let ids = leases.filter { $0.value.identity == identity && $0.value.operation == nil }.map(\.key)
        for id in ids { if let lease = leases.removeValue(forKey: id) { try? FileManager.default.removeItem(at: lease.file) } }
    }
    func destroy() {
        alive = false
        for lease in leases.values where lease.operation == nil { try? FileManager.default.removeItem(at: lease.file) }
        // Claimed operations are independently durable or in flight. Teardown is not cancellation proof.
        leases.removeAll()
    }
}
#endif
