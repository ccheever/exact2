// Native question intake and local byte pins. Attachment dispatch requires the separate upload owner.
// @ref llp/1109.005-composer-and-transcript.decision.md#scratch-tasks-and-queue-boundaries
import Foundation

private struct QuestionKey: CodingKey {
    let stringValue: String
    var intValue: Int? { nil }
    init?(stringValue: String) { self.stringValue = stringValue }
    init?(intValue: Int) { return nil }
}
private func questionKeys(_ decoder: Decoder, _ keys: Set<String>) throws {
    let values = try decoder.container(keyedBy: QuestionKey.self)
    guard Set(values.allKeys.map(\.stringValue)) == keys else { throw T3QuestionAttachmentFailure("Invalid question attachment record fields.") }
}

struct T3QuestionScope: Codable, Hashable {
    let origin: String
    let environment: String
    let thread: String
    let request: String
    init(origin: String, environment: String, thread: String, request: String) throws {
        self.origin = origin; self.environment = environment; self.thread = thread; self.request = request
        try validate()
    }
    init(from decoder: Decoder) throws {
        try questionKeys(decoder, ["origin", "environment", "thread", "request"])
        let c = try decoder.container(keyedBy: CodingKeys.self)
        origin = try c.decode(String.self, forKey: .origin); environment = try c.decode(String.self, forKey: .environment)
        thread = try c.decode(String.self, forKey: .thread); request = try c.decode(String.self, forKey: .request)
        try validate()
    }
    func validate() throws {
        guard var url = URLComponents(string: origin), let scheme = url.scheme?.lowercased(), ["http", "https"].contains(scheme),
              let host = url.host, !host.isEmpty, url.user == nil, url.password == nil else {
            throw T3QuestionAttachmentFailure("Invalid question attachment owner.")
        }
        url.scheme = scheme; url.host = host.lowercased(); url.path = ""; url.query = nil; url.fragment = nil
        guard url.url?.absoluteString == origin, origin == origin.trimmingCharacters(in: .whitespacesAndNewlines),
              [origin, environment, thread, request].allSatisfy({ !$0.isEmpty && $0.count <= 2048 && !$0.contains("\n") }) else {
            throw T3QuestionAttachmentFailure("Invalid question attachment owner.")
        }
    }
}

struct T3QuestionAttachmentMetadata: Codable, Equatable {
    enum Kind: String, Codable { case image, file }
    let kind: Kind
    let name: String
    let mimeType: String
    init(kind: Kind, name: String, mimeType: String) { self.kind = kind; self.name = name; self.mimeType = mimeType }
    init(from decoder: Decoder) throws {
        try questionKeys(decoder, ["kind", "name", "mimeType"])
        let c = try decoder.container(keyedBy: CodingKeys.self)
        kind = try c.decode(Kind.self, forKey: .kind); name = try c.decode(String.self, forKey: .name); mimeType = try c.decode(String.self, forKey: .mimeType)
    }
    var wireMimeType: String {
        let type = mimeType.lowercased()
        if Self.images.contains(type) { return type }
        if ["application/octet-stream", "binary/octet-stream", "application/unknown"].contains(type) {
            let extensionName = (name as NSString).pathExtension.lowercased()
            let types = ["png": "image/png", "jpg": "image/jpeg", "jpeg": "image/jpeg", "gif": "image/gif", "webp": "image/webp"]
            return types[extensionName] ?? mimeType
        }
        return mimeType
    }
    var wireKind: Kind { Self.images.contains(wireMimeType) ? .image : kind }
    static let images = ["image/png", "image/jpeg", "image/gif", "image/webp"]
    func validate(size: Int, fileLimit: Int) throws {
        guard !name.isEmpty, name == name.trimmingCharacters(in: .whitespacesAndNewlines), name.count <= 255,
              !name.contains("\0"), !mimeType.isEmpty, mimeType.count <= 100,
              mimeType.range(of: #"^[A-Za-z0-9!#$&^_.+-]+/[A-Za-z0-9!#$&^_.+-]+$"#, options: .regularExpression) != nil,
              size > 0, size <= 50 * 1024 * 1024,
              kind != .image || Self.images.contains(mimeType),
              kind != .file || fileLimit > 0 && size <= fileLimit,
              wireKind != .image || size <= 10 * 1024 * 1024 else {
            throw T3QuestionAttachmentFailure("The attachment type or size is unsupported.")
        }
    }
}

struct T3QuestionAttachment: Codable, Equatable {
    let id: String
    let request: String
    let question: String
    let metadata: T3QuestionAttachmentMetadata
    let size: Int
    let digest: String
    init(id: String, request: String, question: String, metadata: T3QuestionAttachmentMetadata, size: Int, digest: String) {
        self.id = id; self.request = request; self.question = question; self.metadata = metadata; self.size = size; self.digest = digest
    }
    init(from decoder: Decoder) throws {
        try questionKeys(decoder, ["id", "request", "question", "metadata", "size", "digest"])
        let c = try decoder.container(keyedBy: CodingKeys.self)
        id = try c.decode(String.self, forKey: .id); request = try c.decode(String.self, forKey: .request)
        question = try c.decode(String.self, forKey: .question); metadata = try c.decode(T3QuestionAttachmentMetadata.self, forKey: .metadata)
        size = try c.decode(Int.self, forKey: .size); digest = try c.decode(String.self, forKey: .digest)
    }
}

/// Issued by this owner; callers cannot manufacture admission from a serialized dictionary.
final class T3QuestionAdmission {
    fileprivate let owner: UUID
    let request: String
    let question: String
    let signature: String
    let generation: Int
    let route: String
    let mount: UUID
    let fileLimit: Int
    fileprivate let id = UUID().uuidString.lowercased()
    private let lock = NSLock()
    private var ended = false
    fileprivate init(owner: UUID, request: String, question: String, signature: String, generation: Int, route: String, mount: UUID, fileLimit: Int) {
        self.owner = owner; self.request = request; self.question = question; self.signature = signature
        self.generation = generation; self.route = route; self.mount = mount; self.fileLimit = fileLimit
    }
    func end() { lock.lock(); ended = true; lock.unlock() }
    var live: Bool { lock.lock(); defer { lock.unlock() }; return !ended }
}

final class T3QuestionOperation {
    let id: String
    let fingerprint: String
    let admission: T3QuestionAdmission
    fileprivate init(id: String, fingerprint: String, admission: T3QuestionAdmission) {
        self.id = id; self.fingerprint = fingerprint; self.admission = admission
    }
}
struct T3QuestionReservation {
    let id: String
    fileprivate let operation: T3QuestionOperation
    fileprivate let ordinal: Int
}
final class T3QuestionAttachmentLease {
    let item: T3QuestionAttachment
    private let readBytes: () throws -> Data
    private let releaseBytes: () -> Void
    private let lock = NSLock()
    private var released = false
    fileprivate init(item: T3QuestionAttachment, read: @escaping () throws -> Data, release: @escaping () -> Void) {
        self.item = item; readBytes = read; releaseBytes = release
    }
    func read() throws -> Data {
        lock.lock(); defer { lock.unlock() }
        guard !released else { throw T3QuestionAttachmentFailure("The attachment reader has ended.") }
        return try readBytes()
    }
    func release() { lock.lock(); let first = !released; released = true; lock.unlock(); if first { releaseBytes() } }
    deinit { release() }
}

private struct QuestionPin: Codable {
    enum Phase: String, Codable { case prepared, bound, issued, released, quarantined }
    let id: String; let request: String; let command: String; let epoch: String; let process: String
    let inventoryRevision: Int; let payloadDigest: String; let recordDigest: String; let pinDigest: String
    let questions: [String: String]; let items: [T3QuestionAttachment]; let requireEmpty: Bool
    var phase: Phase; var releaseToken: String; var acknowledged: Bool
    init(id: String, request: String, command: String, epoch: String, process: String, inventoryRevision: Int,
         payloadDigest: String, recordDigest: String, questions: [String: String], items: [T3QuestionAttachment], requireEmpty: Bool) throws {
        self.id = id; self.request = request; self.command = command; self.epoch = epoch; self.process = process
        self.inventoryRevision = inventoryRevision; self.payloadDigest = payloadDigest; self.recordDigest = recordDigest; self.questions = questions
        self.items = items; self.requireEmpty = requireEmpty; phase = .prepared; releaseToken = ""; acknowledged = false
        pinDigest = try Self.digest(request: request, command: command, epoch: epoch, process: process, revision: inventoryRevision,
            payload: payloadDigest, record: recordDigest, questions: questions, items: items, empty: requireEmpty)
    }
    init(from decoder: Decoder) throws {
        try questionKeys(decoder, ["id", "request", "command", "epoch", "process", "inventoryRevision", "payloadDigest", "recordDigest", "pinDigest", "questions", "items", "requireEmpty", "phase", "releaseToken", "acknowledged"])
        let c = try decoder.container(keyedBy: CodingKeys.self)
        id = try c.decode(String.self, forKey: .id); request = try c.decode(String.self, forKey: .request)
        command = try c.decode(String.self, forKey: .command); epoch = try c.decode(String.self, forKey: .epoch); process = try c.decode(String.self, forKey: .process)
        inventoryRevision = try c.decode(Int.self, forKey: .inventoryRevision); payloadDigest = try c.decode(String.self, forKey: .payloadDigest)
        pinDigest = try c.decode(String.self, forKey: .pinDigest); questions = try c.decode([String: String].self, forKey: .questions)
        recordDigest = try c.decode(String.self, forKey: .recordDigest)
        items = try c.decode([T3QuestionAttachment].self, forKey: .items); requireEmpty = try c.decode(Bool.self, forKey: .requireEmpty)
        phase = try c.decode(Phase.self, forKey: .phase); releaseToken = try c.decode(String.self, forKey: .releaseToken)
        acknowledged = try c.decode(Bool.self, forKey: .acknowledged)
    }
    private struct Binding: Encodable {
        let request: String; let command: String; let epoch: String; let process: String; let revision: Int
        let payload: String; let record: String; let questions: [String: String]; let items: [T3QuestionAttachment]; let empty: Bool
    }
    static func digest(request: String, command: String, epoch: String, process: String, revision: Int, payload: String,
                       record: String, questions: [String: String], items: [T3QuestionAttachment], empty: Bool) throws -> String {
        let encoder = JSONEncoder(); encoder.outputFormatting = [.sortedKeys, .withoutEscapingSlashes]
        return t3QuestionDigest(try encoder.encode(Binding(request: request, command: command, epoch: epoch, process: process,
            revision: revision, payload: payload, record: record, questions: questions, items: items, empty: empty)))
    }
    var evidence: [String: Any] { ["pinId": id, "pinDigest": pinDigest, "requestIncarnation": request, "inventoryRevision": inventoryRevision] }
}

/// All durable transitions are serialized. Byte producers run outside this lock, with a live writer hold.
final class T3QuestionAttachments {
    private struct Request: Codable {
        let id: String; let scope: T3QuestionScope; var questions: [String: String]; var active: Bool; var order: [String]
        init(id: String, scope: T3QuestionScope, questions: [String: String]) { self.id = id; self.scope = scope; self.questions = questions; active = true; order = [] }
        init(from decoder: Decoder) throws {
            try questionKeys(decoder, ["id", "scope", "questions", "active", "order"])
            let c = try decoder.container(keyedBy: CodingKeys.self)
            id = try c.decode(String.self, forKey: .id); scope = try c.decode(T3QuestionScope.self, forKey: .scope)
            questions = try c.decode([String: String].self, forKey: .questions); active = try c.decode(Bool.self, forKey: .active); order = try c.decode([String].self, forKey: .order)
        }
    }
    private struct Slot: Codable {
        enum Phase: String, Codable { case reserved, staged, adopted, cancelled }
        private enum CodingKeys: String, CodingKey { case id, ordinal, phase, item }
        let id: String; let ordinal: Int; var phase: Phase; var item: T3QuestionAttachment?
        init(id: String, ordinal: Int) { self.id = id; self.ordinal = ordinal; phase = .reserved; item = nil }
        init(from decoder: Decoder) throws {
            try questionKeys(decoder, ["id", "ordinal", "phase", "item"])
            let c = try decoder.container(keyedBy: CodingKeys.self)
            id = try c.decode(String.self, forKey: .id); ordinal = try c.decode(Int.self, forKey: .ordinal)
            phase = try c.decode(Phase.self, forKey: .phase); item = try c.decodeIfPresent(T3QuestionAttachment.self, forKey: .item)
        }
        func encode(to encoder: Encoder) throws {
            var c = encoder.container(keyedBy: CodingKeys.self)
            try c.encode(id, forKey: .id); try c.encode(ordinal, forKey: .ordinal); try c.encode(phase, forKey: .phase); try c.encode(item, forKey: .item)
        }
    }
    private struct Operation: Codable {
        enum Phase: String, Codable { case opened, adopted, cancelled }
        let id: String; let fingerprint: String; let request: String; let question: String; let admission: String; let process: String; var phase: Phase; var slots: [Slot]
        init(id: String, fingerprint: String, admission: T3QuestionAdmission, process: String) {
            self.id = id; self.fingerprint = fingerprint; request = admission.request; question = admission.question
            self.admission = admission.id; self.process = process; phase = .opened; slots = []
        }
        init(from decoder: Decoder) throws {
            try questionKeys(decoder, ["id", "fingerprint", "request", "question", "admission", "process", "phase", "slots"])
            let c = try decoder.container(keyedBy: CodingKeys.self)
            id = try c.decode(String.self, forKey: .id); fingerprint = try c.decode(String.self, forKey: .fingerprint)
            request = try c.decode(String.self, forKey: .request); question = try c.decode(String.self, forKey: .question)
            admission = try c.decode(String.self, forKey: .admission); process = try c.decode(String.self, forKey: .process)
            phase = try c.decode(Phase.self, forKey: .phase); slots = try c.decode([Slot].self, forKey: .slots)
        }
    }
    private struct State: Codable {
        let version: Int; var requests: [Request]; var operations: [Operation]; var items: [T3QuestionAttachment]; var garbage: [String]; var revision: Int; var pins: [QuestionPin]
        init() { version = 1; requests = []; operations = []; items = []; garbage = []; revision = 0; pins = [] }
        init(from decoder: Decoder) throws {
            let keys = try decoder.container(keyedBy: QuestionKey.self)
            let required: Set<String> = ["version", "requests", "operations", "items", "garbage"]
            let actual = Set(keys.allKeys.map(\.stringValue))
            guard required.isSubset(of: actual), actual.isSubset(of: required.union(["revision", "pins"])) else { throw T3QuestionAttachmentFailure("Invalid question inventory fields.") }
            let c = try decoder.container(keyedBy: CodingKeys.self)
            version = try c.decode(Int.self, forKey: .version); requests = try c.decode([Request].self, forKey: .requests)
            operations = try c.decode([Operation].self, forKey: .operations); items = try c.decode([T3QuestionAttachment].self, forKey: .items); garbage = try c.decode([String].self, forKey: .garbage)
            revision = c.contains(.revision) ? try c.decode(Int.self, forKey: .revision) : 0
            pins = c.contains(.pins) ? try c.decode([QuestionPin].self, forKey: .pins) : []
        }
    }
    private static let process = UUID().uuidString.lowercased()
    private static let ownersLock = NSLock()
    private static var owners: [String: T3QuestionAttachments] = [:]
    static func shared(dataRoot: URL) -> T3QuestionAttachments {
        ownersLock.lock(); defer { ownersLock.unlock() }
        let key = dataRoot.standardizedFileURL.path
        if let existing = owners[key] { return existing }
        let owner = T3QuestionAttachments(dataRoot: dataRoot); owners[key] = owner; return owner
    }
    private let identity = UUID()
    private let disk: T3QuestionAttachmentDisk
    private let lock = NSRecursiveLock()
    private var state: State?
    private var holds: [String: Int] = [:]
    private var admissions: [String: T3QuestionAdmission] = [:]
    private init(dataRoot: URL) { disk = T3QuestionAttachmentFiles(dataRoot: dataRoot) }
    init(disk: T3QuestionAttachmentDisk) { self.disk = disk }
    private func locked<T>(_ body: () throws -> T) rethrows -> T { lock.lock(); defer { lock.unlock() }; return try body() }
    private func fail(_ message: String = "The question attachment operation changed.") -> T3QuestionAttachmentFailure { .init(message) }
    private static func uuid(_ value: String) -> Bool { UUID(uuidString: value)?.uuidString.lowercased() == value }
    private func validate(_ value: State) throws {
        guard value.version == 1, (0...9_007_199_254_740_991).contains(value.revision), value.pins.count <= 100_000, value.requests.count <= 10_000, value.operations.count <= 100_000, value.items.count <= 1_000_000,
              Set(value.requests.map(\.id)).count == value.requests.count, Set(value.operations.map(\.id)).count == value.operations.count,
              Set(value.items.map(\.id)).count == value.items.count, Set(value.garbage).count == value.garbage.count,
              value.garbage.allSatisfy(Self.uuid) else { throw fail("Question attachment inventory is corrupt.") }
        guard Set(value.pins.map(\.id)).count == value.pins.count, Set(value.pins.map(\.command)).count == value.pins.count else { throw fail() }
        for pin in value.pins {
            guard Self.uuid(pin.id), Self.uuid(pin.epoch), Self.uuid(pin.process), Self.uuid(pin.request),
                  !pin.command.isEmpty, pin.command.utf8.count <= 65_536, (0...9_007_199_254_740_991).contains(pin.inventoryRevision),
                  value.requests.contains(where: { $0.id == pin.request && $0.questions == pin.questions }), pin.inventoryRevision <= value.revision, pin.items.count <= 100,
                  Set(pin.items.map(\.id)).count == pin.items.count, pin.items.allSatisfy({ $0.request == pin.request && pin.questions[$0.question] != nil }),
                  pin.payloadDigest.count == 64, pin.payloadDigest.allSatisfy({ $0.isHexDigit && !$0.isUppercase }),
                  pin.recordDigest.count == 64, pin.recordDigest.allSatisfy({ $0.isHexDigit && !$0.isUppercase }),
                  !pin.requireEmpty || pin.items.isEmpty, !pin.acknowledged || pin.phase == .released,
                  pin.phase == .released ? Self.uuid(pin.releaseToken) : pin.releaseToken.isEmpty,
                  try QuestionPin.digest(request: pin.request, command: pin.command, epoch: pin.epoch, process: pin.process,
                    revision: pin.inventoryRevision, payload: pin.payloadDigest, record: pin.recordDigest, questions: pin.questions, items: pin.items, empty: pin.requireEmpty) == pin.pinDigest else { throw fail("Question pin inventory is corrupt.") }
            for item in pin.items { try item.metadata.validate(size: item.size, fileLimit: 50 * 1024 * 1024) }
        }
        var ids = Set<String>()
        for request in value.requests {
            try request.scope.validate()
            guard Self.uuid(request.id), !request.questions.isEmpty, request.questions.count <= 100,
                  request.questions.allSatisfy({
                      !$0.key.isEmpty && $0.key.count <= 2048 && !$0.value.isEmpty && $0.value.count <= 65_536
                  }),
                  Set(request.order).count == request.order.count,
                  request.order.allSatisfy({ id in value.items.contains { $0.id == id && $0.request == request.id } }), request.order.count <= 100 else { throw fail() }
        }
        let active = value.requests.filter(\.active).map(\.scope)
        guard Set(active).count == active.count else { throw fail() }
        for item in value.items {
            guard Self.uuid(item.id), Self.uuid(item.request), item.digest.count == 64,
                  item.digest.allSatisfy({ $0.isHexDigit && !$0.isUppercase }),
                  let request = value.requests.first(where: { $0.id == item.request }), request.order.contains(item.id), request.questions[item.question] != nil,
                  !value.garbage.contains(item.id) else { throw fail() }
            try item.metadata.validate(size: item.size, fileLimit: 50 * 1024 * 1024)
        }
        for operation in value.operations {
            guard Self.uuid(operation.id), Self.uuid(operation.admission), Self.uuid(operation.process),
                  operation.fingerprint.count == 64, operation.fingerprint.allSatisfy({ $0.isHexDigit && !$0.isUppercase }),
                  let request = value.requests.first(where: { $0.id == operation.request }), request.questions[operation.question] != nil,
                  operation.slots.count <= 100, Set(operation.slots.map(\.ordinal)).count == operation.slots.count else { throw fail() }
            for slot in operation.slots {
                guard Self.uuid(slot.id), ids.insert(slot.id).inserted, (0..<100).contains(slot.ordinal) else { throw fail() }
                if let item = slot.item {
                    guard item.id == slot.id, item.request == operation.request, item.question == operation.question else { throw fail() }
                    guard item.digest.count == 64, item.digest.allSatisfy({ $0.isHexDigit && !$0.isUppercase }) else { throw fail() }
                    try item.metadata.validate(size: item.size, fileLimit: 50 * 1024 * 1024)
                }
                guard slot.phase == .reserved && slot.item == nil || slot.phase == .cancelled || slot.item != nil,
                      operation.phase != .cancelled || slot.phase == .cancelled,
                      operation.phase != .adopted || [.adopted, .cancelled].contains(slot.phase),
                      slot.phase != .adopted || slot.item != nil else { throw fail() }
            }
        }
        for request in value.requests {
            let reservations = value.operations.filter { $0.request == request.id }.flatMap(\.slots).filter { $0.phase == .reserved || $0.phase == .staged }.count
            guard request.order.count + reservations <= 100 else { throw fail() }
        }
    }
    private func commit(_ value: State, logical: Bool = true) throws {
        var next = value
        if logical { guard next.revision < 9_007_199_254_740_991 else { throw fail() }; next.revision += 1 }
        try validate(next)
        let encoder = JSONEncoder(); encoder.outputFormatting = [.sortedKeys]
        do { try disk.replace(encoder.encode(next)); state = next }
        catch { state = nil; throw error } // Replacement may have completed; never assume rollback.
    }
    private func load() throws -> State {
        if let state { return state }
        guard let bytes = try disk.load() else {
            let next = State(); try commit(next, logical: false); return next
        }
        var next = try JSONDecoder().decode(State.self, from: bytes); try validate(next)
        var changed = false
        for index in next.operations.indices where next.operations[index].process != Self.process && next.operations[index].phase == .opened {
            next.operations[index].phase = .cancelled
            for slot in next.operations[index].slots.indices {
                next.operations[index].slots[slot].phase = .cancelled
                let id = next.operations[index].slots[slot].id
                if !next.garbage.contains(id) { next.garbage.append(id) }
            }
            changed = true
        }
        // A readable replacement may still lack its durability acknowledgement.
        // Durability confirmation alone must not advance logical selection revision.
        try commit(next, logical: changed)
        if changed {
            try collectLocked()
            guard let state else { throw fail("Question attachment restart recovery failed.") }
            return state
        }
        return next
    }
    private func check(_ admission: T3QuestionAdmission, in value: State) throws {
        guard admission.owner == identity, admission.live, admissions[admission.id] === admission,
              value.requests.contains(where: {
                  $0.id == admission.request && $0.active && $0.questions[admission.question] == admission.signature
              }) else { throw fail() }
    }
    private func operation(_ token: T3QuestionOperation, in value: State, requireLive: Bool = true) throws -> Int {
        if requireLive { try check(token.admission, in: value) }
        guard token.admission.owner == identity, let index = value.operations.firstIndex(where: {
            $0.id == token.id && $0.admission == token.admission.id && $0.fingerprint == token.fingerprint
        }) else { throw fail() }
        return index
    }
    /// Called with an authoritative request catalog, never a transient missing UI projection.
    func register(_ scope: T3QuestionScope, questions: [String: String]) throws -> String {
        try locked {
            try scope.validate()
            var next = try load()
            if let prior = next.requests.first(where: { $0.scope == scope && $0.active }) {
                guard prior.questions == questions else { throw fail("The request questions changed; retire the prior native request first.") }
                return prior.id
            }
            let id = UUID().uuidString.lowercased(); next.requests.append(Request(id: id, scope: scope, questions: questions))
            try commit(next); return id
        }
    }
    func admit(request: String, question: String, signature: String, generation: Int, route: String, mount: UUID, fileLimit: Int) throws -> T3QuestionAdmission {
        try locked {
            var next = try load()
            guard !signature.isEmpty, signature.count <= 65_536, generation >= 0, !route.isEmpty, route.count <= 8192,
                  (0...50 * 1024 * 1024).contains(fileLimit),
                  next.requests.contains(where: { $0.id == request && $0.active && $0.questions[question] == signature }) else { throw fail() }
            let replaced = admissions.values.filter { $0.request == request && $0.question == question }
            for old in replaced {
                old.end()
                for index in next.operations.indices where next.operations[index].admission == old.id && next.operations[index].phase == .opened {
                    next.operations[index].phase = .cancelled
                    for slot in next.operations[index].slots.indices {
                        next.operations[index].slots[slot].phase = .cancelled
                        let id = next.operations[index].slots[slot].id
                        if !next.garbage.contains(id) { next.garbage.append(id) }
                    }
                }
            }
            if !replaced.isEmpty {
                try commit(next)
                for old in replaced { admissions.removeValue(forKey: old.id) }
                try collectLocked()
            }
            let admission = T3QuestionAdmission(owner: identity, request: request, question: question, signature: signature, generation: generation, route: route, mount: mount, fileLimit: fileLimit)
            admissions[admission.id] = admission
            return admission
        }
    }
    func open(_ admission: T3QuestionAdmission, kind: String) throws -> T3QuestionOperation {
        try locked {
            guard ["photos", "files", "paste"].contains(kind) else { throw fail("Invalid question attachment operation.") }
            var next = try load(); try check(admission, in: next)
            let input = try JSONEncoder().encode([
                admission.request, admission.question, admission.signature, String(admission.generation),
                admission.route, admission.mount.uuidString.lowercased(), String(admission.fileLimit), kind,
            ])
            let fingerprint = t3QuestionDigest(input), id = UUID().uuidString.lowercased()
            next.operations.append(Operation(id: id, fingerprint: fingerprint, admission: admission, process: Self.process))
            try commit(next); return T3QuestionOperation(id: id, fingerprint: fingerprint, admission: admission)
        }
    }
    func remaining(_ admission: T3QuestionAdmission) throws -> Int {
        try locked {
            let value = try load(); try check(admission, in: value)
            let reserved = value.operations.filter { $0.request == admission.request }.flatMap(\.slots).filter { $0.phase == .reserved || $0.phase == .staged }.count
            return 100 - value.items.filter { $0.request == admission.request }.count - reserved
        }
    }
    func reserve(_ token: T3QuestionOperation, ordinal: Int) throws -> T3QuestionReservation {
        try locked {
            var next = try load(); let op = try operation(token, in: next)
            guard next.operations[op].phase == .opened, (0..<100).contains(ordinal) else { throw fail() }
            if let slot = next.operations[op].slots.first(where: { $0.ordinal == ordinal }) {
                guard slot.phase != .cancelled else { throw fail() }; return T3QuestionReservation(id: slot.id, operation: token, ordinal: ordinal)
            }
            guard try remaining(token.admission) > 0 else { throw fail("You can attach up to 100 attachments per question response.") }
            let id = UUID().uuidString.lowercased(); next.operations[op].slots.append(Slot(id: id, ordinal: ordinal))
            try commit(next); return T3QuestionReservation(id: id, operation: token, ordinal: ordinal)
        }
    }
    /// The native producer may copy or convert bytes. Cancellation never waits for it.
    func stage(_ ticket: T3QuestionReservation, metadata: T3QuestionAttachmentMetadata, produce: () throws -> Data) throws {
        let existing: T3QuestionAttachment? = try locked {
            let value = try load(); let op = try operation(ticket.operation, in: value)
            guard value.operations[op].phase == .opened, let slot = value.operations[op].slots.first(where: { $0.id == ticket.id && $0.ordinal == ticket.ordinal }),
                  [.reserved, .staged].contains(slot.phase), holds[ticket.id, default: 0] == 0 else { throw fail() }
            holds[ticket.id, default: 0] += 1; return slot.item
        }
        defer { locked { releaseHold(ticket.id) } }
        if let existing {
            guard existing.metadata == metadata else { throw fail("This reserved attachment already has different metadata.") }
            let retained = try disk.read(ticket.id, limit: existing.size)
            guard retained.count == existing.size, t3QuestionDigest(retained) == existing.digest else {
                throw fail("The staged attachment changed.")
            }
            return
        }
        let bytes = try produce()
        try metadata.validate(size: bytes.count, fileLimit: ticket.operation.admission.fileLimit)
        let item = T3QuestionAttachment(id: ticket.id, request: ticket.operation.admission.request, question: ticket.operation.admission.question,
                                        metadata: metadata, size: bytes.count, digest: t3QuestionDigest(bytes))
        try locked {
            var next = try load(); let op = try operation(ticket.operation, in: next)
            guard next.operations[op].phase == .opened, let slot = next.operations[op].slots.firstIndex(where: { $0.id == ticket.id }), next.operations[op].slots[slot].phase == .reserved else { throw fail() }
            do {
                do { try disk.write(ticket.id, bytes: bytes) }
                catch {
                    // A prior write can have succeeded before its inventory save failed.
                    guard let retained = try? disk.read(ticket.id, limit: bytes.count), retained == bytes else { throw error }
                }
                try check(ticket.operation.admission, in: next)
                next.operations[op].slots[slot].item = item; next.operations[op].slots[slot].phase = .staged
                try commit(next)
            } catch {
                // A file or journal replacement may have succeeded. Its reservation remains cleanup authority.
                state = nil; throw error
            }
        }
    }
    func discard(_ ticket: T3QuestionReservation) throws {
        try locked {
            var next = try load(); let op = try operation(ticket.operation, in: next, requireLive: false)
            guard next.operations[op].phase != .adopted, let slot = next.operations[op].slots.firstIndex(where: { $0.id == ticket.id }) else { throw fail() }
            next.operations[op].slots[slot].phase = .cancelled
            if !next.garbage.contains(ticket.id) { next.garbage.append(ticket.id) }
            try commit(next); try collectLocked()
        }
    }
    func adopt(_ token: T3QuestionOperation) throws -> [T3QuestionAttachment] {
        try locked {
            var next = try load(); let op = try operation(token, in: next, requireLive: false)
            if next.operations[op].phase == .adopted { return next.operations[op].slots.sorted { $0.ordinal < $1.ordinal }.compactMap { $0.phase == .adopted ? $0.item : nil } }
            try check(token.admission, in: next)
            guard next.operations[op].phase == .opened, !next.operations[op].slots.contains(where: { $0.phase == .reserved }),
                  let request = next.requests.firstIndex(where: { $0.id == token.admission.request }) else { throw fail() }
            let items = next.operations[op].slots.sorted { $0.ordinal < $1.ordinal }.compactMap { $0.phase == .staged ? $0.item : nil }
            for item in items {
                let bytes = try disk.read(item.id, limit: item.size)
                guard bytes.count == item.size, t3QuestionDigest(bytes) == item.digest else { throw fail("The staged attachment changed.") }
            }
            next.items += items; next.requests[request].order += items.map(\.id)
            for slot in next.operations[op].slots.indices where next.operations[op].slots[slot].phase == .staged { next.operations[op].slots[slot].phase = .adopted }
            next.operations[op].phase = .adopted; try commit(next); return items
        }
    }
    func cancel(_ token: T3QuestionOperation) throws {
        try locked {
            var next = try load(); let op = try operation(token, in: next, requireLive: false)
            guard next.operations[op].phase == .opened else { return }
            next.operations[op].phase = .cancelled
            for slot in next.operations[op].slots.indices {
                next.operations[op].slots[slot].phase = .cancelled
                let id = next.operations[op].slots[slot].id; if !next.garbage.contains(id) { next.garbage.append(id) }
            }
            try commit(next); try collectLocked()
        }
    }
    func end(_ admission: T3QuestionAdmission) throws {
        admission.end()
        try locked {
            let value = try load()
            for op in value.operations where op.admission == admission.id && op.phase == .opened {
                try cancel(T3QuestionOperation(id: op.id, fingerprint: op.fingerprint, admission: admission))
            }
            admissions.removeValue(forKey: admission.id)
        }
    }
    func items(request: String, question: String) throws -> [T3QuestionAttachment] {
        try locked {
            let value = try load()
            guard let owner = value.requests.first(where: { $0.id == request }), owner.questions[question] != nil else { throw fail() }
            return owner.order.compactMap { id in value.items.first { $0.id == id && $0.question == question } }
        }
    }
    func replay(request: String, operation: String, fingerprint: String) throws -> [T3QuestionAttachment]? {
        try locked {
            let value = try load()
            guard Self.uuid(operation), fingerprint.count == 64,
                  let op = value.operations.first(where: {
                      $0.id == operation && $0.request == request && $0.fingerprint == fingerprint
                  }) else { throw fail() }
            if op.phase == .opened { return nil }
            return op.phase == .cancelled ? [] : op.slots.sorted { $0.ordinal < $1.ordinal }.compactMap { $0.phase == .adopted ? $0.item : nil }
        }
    }
    func remove(_ admission: T3QuestionAdmission, id: String) throws {
        try locked {
            var next = try load(); try check(admission, in: next)
            guard let request = next.requests.firstIndex(where: { $0.id == admission.request }) else { throw fail() }
            guard let item = next.items.firstIndex(where: {
                $0.id == id && $0.request == admission.request && $0.question == admission.question
            }) else {
                if let operation = next.operations.firstIndex(where: {
                    $0.admission == admission.id && $0.request == admission.request && $0.question == admission.question
                        && $0.slots.contains { $0.id == id && [.reserved, .staged, .cancelled].contains($0.phase) }
                }), let slot = next.operations[operation].slots.firstIndex(where: { $0.id == id }) {
                    // Remove is also the exact live operation's logical withdrawal. It
                    // commits before collection, so adoption cannot publish this slot;
                    // a producer/reader hold delays only physical deletion.
                    if next.operations[operation].slots[slot].phase != .cancelled {
                        next.operations[operation].slots[slot].phase = .cancelled
                        if !next.garbage.contains(id) { next.garbage.append(id) }
                        try commit(next)
                    }
                    try collectLocked()
                    return
                }
                // An adopted operation is immutable history, not current inventory.
                // It proves only that repeating removal of an escaped item is harmless.
                let wasAdopted = next.operations.contains { operation in
                    operation.request == admission.request && operation.question == admission.question
                        && operation.phase == .adopted
                        && operation.slots.contains { $0.id == id && $0.phase == .adopted && $0.item?.id == id }
                }
                guard wasAdopted else { throw fail() }
                try collectLocked()
                return
            }
            next.items.remove(at: item); next.requests[request].order.removeAll { $0 == id }; next.garbage.append(id)
            try commit(next); try collectLocked()
        }
    }
    func retire(request: String) throws {
        try locked {
            var next = try load(); guard let index = next.requests.firstIndex(where: { $0.id == request }) else { throw fail() }
            for admission in admissions.values where admission.request == request { admission.end() }
            next.requests[index].active = false
            let removed = next.items.filter { $0.request == request }.map(\.id)
            next.items.removeAll { $0.request == request }; next.requests[index].order = []
            for id in removed where !next.garbage.contains(id) { next.garbage.append(id) }
            for op in next.operations.indices where next.operations[op].request == request && next.operations[op].phase == .opened {
                next.operations[op].phase = .cancelled
                for slot in next.operations[op].slots.indices {
                    next.operations[op].slots[slot].phase = .cancelled
                    let id = next.operations[op].slots[slot].id; if !next.garbage.contains(id) { next.garbage.append(id) }
                }
            }
            try commit(next); try collectLocked()
        }
    }
    func lease(request: String, question: String, id: String) throws -> T3QuestionAttachmentLease {
        try locked {
            let value = try load()
            guard let item = value.items.first(where: { $0.id == id && $0.request == request && $0.question == question }) else { throw fail() }
            holds[id, default: 0] += 1
            return T3QuestionAttachmentLease(item: item, read: { [self] in
                let bytes = try disk.read(id, limit: item.size)
                guard bytes.count == item.size, t3QuestionDigest(bytes) == item.digest else { throw fail("The attachment bytes changed.") }
                return bytes
            }, release: { [self] in locked { releaseHold(id) } })
        }
    }
    func retainedEpochs() throws -> Set<String> { try locked { Set(try load().pins.map(\.epoch)) } }
    func ownedCommandIDs() throws -> Set<String> { try locked { Set(try load().pins.map(\.command)) } }
    /// Negative replay filtering only. These names grant no pin, receipt or mutation authority.
    func blockedCommandIDs() -> Set<String> {
        locked {
            if let value = try? load() { return Set(value.pins.map(\.command)) }
            guard let bytes = try? disk.load(), let raw = try? JSONSerialization.jsonObject(with: bytes) as? [String: Any],
                  let pins = raw["pins"] as? [[String: Any]] else { return [] }
            return Set(pins.compactMap { $0["command"] as? String }.filter { !$0.isEmpty && $0.utf8.count <= 65_536 })
        }
    }
    func quarantineUnverifiedPins() throws {
        try locked {
            var next = try load(), changed = false
            for index in next.pins.indices where [.bound, .issued].contains(next.pins[index].phase) {
                next.pins[index].phase = .quarantined; changed = true
            }
            if changed { try commit(next, logical: false) }
        }
    }
    /// A name alone never proves the receipt which owns a bound/issued inventory pin.
    func reconcilePins(receipts: [String: T3QuestionPinReceipt], pinless: Set<String>) throws {
        try locked {
            var next = try load(), changed = false, invalid = false
            for index in next.pins.indices {
                let pin = next.pins[index]
                guard [.bound, .issued, .quarantined].contains(pin.phase) else { continue }
                let request = next.requests.first { $0.id == pin.request }
                let receipt = receipts[pin.command]
                let matches = receipt.map { receipt in
                    receipt.id == pin.id && receipt.digest == pin.pinDigest && receipt.payload == pin.payloadDigest && receipt.record == pin.recordDigest && receipt.request == pin.request
                        && receipt.revision == pin.inventoryRevision && receipt.questions == pin.questions && request?.scope == receipt.scope
                } == true && !pinless.contains(pin.command)
                if !matches || pin.phase == .quarantined {
                    invalid = true
                    if pin.phase != .quarantined { next.pins[index].phase = .quarantined; changed = true }
                }
            }
            // Every provided receipt must name exactly one native pin, including a prepared create gap.
            if next.pins.contains(where: { pinless.contains($0.command) }) { invalid = true }
            for receipt in receipts.values {
                guard let pin = next.pins.first(where: { $0.id == receipt.id }), pin.command == receipt.command,
                      pin.pinDigest == receipt.digest, pin.payloadDigest == receipt.payload, pin.recordDigest == receipt.record, pin.request == receipt.request,
                      pin.inventoryRevision == receipt.revision, pin.questions == receipt.questions,
                      next.requests.first(where: { $0.id == pin.request })?.scope == receipt.scope else { invalid = true; continue }
            }
            if changed { try commit(next, logical: false) }
            if invalid { throw fail("The question receipt and native pin binding disagree. Retained ownership needs recovery.") }
        }
    }
    /// Exact native incarnation retirement never borrows a successor's request or intake.
    func retireBoundRequest(_ binding: T3QuestionPinReceipt) throws {
        try locked {
            let value = try load()
            guard let request = value.requests.first(where: { $0.id == binding.request }), request.scope == binding.scope,
                  request.questions == binding.questions,
                  let pin = value.pins.first(where: { $0.id == binding.id }), pin.command == binding.command,
                  pin.pinDigest == binding.digest, pin.payloadDigest == binding.payload, pin.recordDigest == binding.record else { throw fail() }
            if request.active { try retire(request: request.id) }
            else { try commit(value, logical: false) }
        }
    }
    func hasBoundPins() throws -> Bool { try locked { try load().pins.contains { [.bound, .issued, .quarantined].contains($0.phase) } } }
    private func pinRequest(_ pin: QuestionPin, in value: State) throws {
        guard let request = value.requests.first(where: { $0.id == pin.request && $0.active }), request.questions == pin.questions else { throw fail("The native request catalog changed.") }
        if pin.requireEmpty {
            guard value.revision == pin.inventoryRevision, request.order.isEmpty,
                  !value.items.contains(where: { $0.request == request.id }),
                  !value.operations.contains(where: { $0.request == request.id && $0.phase == .opened }) else {
                throw fail("Finish or remove every question attachment and intake before submitting text answers.")
            }
        }
    }
    /// Local selection hashing holds byte leases, never this inventory's mutex.
    /// The journal's real text path passes no selection and performs no large file work under its mutex.
    func preparePin(request: String, questions: [String: String], command: String, epoch: String, process: String,
                    payloadDigest: String, requireEmpty: Bool, selection: [String], recordDigest: String? = nil) throws -> [String: Any] {
        let captured: (Int, [T3QuestionAttachmentLease]) = try locked {
            let value = try load()
            guard let owner = value.requests.first(where: { $0.id == request && $0.active }), owner.questions == questions,
                  selection.count <= 100, Set(selection).count == selection.count else { throw fail() }
            if requireEmpty {
                guard owner.order.isEmpty, !value.items.contains(where: { $0.request == request }),
                      !value.operations.contains(where: { $0.request == request && $0.phase == .opened }) else { throw fail("The question request still owns attachments or intake.") }
            }
            let leases = try selection.map { id -> T3QuestionAttachmentLease in
                guard let item = value.items.first(where: { $0.id == id && $0.request == request }) else { throw fail() }
                return try lease(request: request, question: item.question, id: id)
            }
            return (value.revision, leases)
        }
        defer { captured.1.forEach { $0.release() } }
        for lease in captured.1 { _ = try lease.read() }
        return try locked {
            var next = try load()
            guard next.revision == captured.0, let owner = next.requests.first(where: { $0.id == request && $0.active }), owner.questions == questions,
                  captured.1.allSatisfy({ next.items.contains($0.item) }) else { throw fail("The question selection changed during preparation.") }
            let items = captured.1.map(\.item)
            guard items.filter({ $0.metadata.wireKind == .image }).reduce(0, { $0 + $1.size }) <= 80 * 1024 * 1024 else { throw fail() }
            if let prior = next.pins.first(where: { $0.command == command }) {
                guard prior.request == request, prior.epoch == epoch, prior.process == process, prior.payloadDigest == payloadDigest,
                      prior.recordDigest == (recordDigest ?? payloadDigest),
                      prior.questions == questions, prior.items == items, prior.requireEmpty == requireEmpty, prior.phase == .prepared else { throw fail() }
                try pinRequest(prior, in: next); try commit(next, logical: false); return prior.evidence
            }
            let pin = try QuestionPin(id: UUID().uuidString.lowercased(), request: request, command: command, epoch: epoch,
                process: process, inventoryRevision: next.revision, payloadDigest: payloadDigest, recordDigest: recordDigest ?? payloadDigest, questions: questions, items: items, requireEmpty: requireEmpty)
            try pinRequest(pin, in: next); next.pins.append(pin); try commit(next, logical: false); return pin.evidence
        }
    }
    @discardableResult func pinStatus(_ id: String) throws -> [String: Any] {
        try locked {
            let next = try load(); guard let pin = next.pins.first(where: { $0.id == id }) else { throw fail() }
            try commit(next, logical: false)
            return pin.evidence.merging(["phase": pin.phase.rawValue, "releaseToken": pin.releaseToken, "acknowledged": pin.acknowledged], uniquingKeysWith: { _, new in new })
        }
    }
    func bindPin(_ id: String, command: String, payloadDigest: String, pinDigest: String) throws {
        try locked {
            var next = try load(); guard let index = next.pins.firstIndex(where: { $0.id == id }), next.pins[index].command == command,
                  next.pins[index].payloadDigest == payloadDigest, next.pins[index].pinDigest == pinDigest,
                  [.prepared, .bound, .issued].contains(next.pins[index].phase) else { throw fail() }
            try pinRequest(next.pins[index], in: next)
            if next.pins[index].phase != .issued { next.pins[index].phase = .bound }; try commit(next, logical: false)
        }
    }
    func issuePin(_ id: String, command: String, payloadDigest: String, pinDigest: String) throws {
        try locked {
            var next = try load(); guard let index = next.pins.firstIndex(where: { $0.id == id }), next.pins[index].command == command,
                  next.pins[index].payloadDigest == payloadDigest, next.pins[index].pinDigest == pinDigest,
                  [.bound, .issued].contains(next.pins[index].phase) else { throw fail() }
            // A local nonempty hold has no native upload provenance, regardless of caller descriptor IDs.
            guard next.pins[index].items.isEmpty else { throw fail("Question attachment dispatch requires native upload provenance.") }
            try pinRequest(next.pins[index], in: next); next.pins[index].phase = .issued; try commit(next, logical: false)
        }
    }
    func releasePin(_ id: String, command: String, pinDigest: String) throws -> String {
        try locked {
            var next = try load(); guard let index = next.pins.firstIndex(where: { $0.id == id }), next.pins[index].command == command,
                  next.pins[index].pinDigest == pinDigest, next.pins[index].phase != .quarantined else { throw fail() }
            if next.pins[index].phase != .released {
                next.pins[index].phase = .released; next.pins[index].releaseToken = UUID().uuidString.lowercased()
            }
            let token = next.pins[index].releaseToken
            try commit(next, logical: false); try collectLocked(); return token
        }
    }
    func ackPin(_ id: String, command: String, token: String) throws {
        try locked {
            var next = try load(); guard let index = next.pins.firstIndex(where: { $0.id == id }), next.pins[index].command == command,
                  next.pins[index].phase == .released, next.pins[index].releaseToken == token else { throw fail() }
            next.pins[index].acknowledged = true; try commit(next, logical: false)
        }
    }
    /// Only seals already present before this actual process starts can authorize collection.
    func recoverPins(receipts: Set<String>, priorSeals: Set<String>, process: String) throws {
        try locked {
            var next = try load(), changed = false
            for index in next.pins.indices {
                let pin = next.pins[index]
                if !receipts.contains(pin.command), [.bound, .issued].contains(pin.phase) {
                    next.pins[index].phase = .quarantined; changed = true
                } else if pin.process != process, priorSeals.contains(pin.epoch), pin.phase == .prepared, !receipts.contains(pin.command) {
                    next.pins[index].phase = .released; next.pins[index].releaseToken = UUID().uuidString.lowercased(); next.pins[index].acknowledged = true; changed = true
                }
            }
            let obsolete = next.pins.filter { $0.process != process && priorSeals.contains($0.epoch) && $0.phase == .released && $0.acknowledged && !receipts.contains($0.command) }
            if !obsolete.isEmpty { next.pins.removeAll { pin in obsolete.contains { $0.id == pin.id } }; changed = true }
            if changed { try commit(next, logical: false); try collectLocked() }
        }
    }
    private func releaseHold(_ id: String) { holds[id, default: 0] -= 1; if holds[id] == 0 { holds.removeValue(forKey: id) }; try? collectLocked() }
    func collect() throws { try locked { try collectLocked() } }
    private func collectLocked() throws {
        var next = try load(), changed = false
        let pinned = Set(next.pins.filter { $0.phase != .released }.flatMap { $0.items.map(\.id) })
        for id in next.garbage where holds[id, default: 0] == 0 && !pinned.contains(id) {
            try disk.remove(id); next.garbage.removeAll { $0 == id }; changed = true
        }
        if changed { try commit(next) }
    }
}
