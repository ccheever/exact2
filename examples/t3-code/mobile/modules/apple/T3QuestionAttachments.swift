// Native question intake only. Uploads and response pins are separate, unimplemented owners.
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
        let version: Int; var requests: [Request]; var operations: [Operation]; var items: [T3QuestionAttachment]; var garbage: [String]
        init() { version = 1; requests = []; operations = []; items = []; garbage = [] }
        init(from decoder: Decoder) throws {
            try questionKeys(decoder, ["version", "requests", "operations", "items", "garbage"])
            let c = try decoder.container(keyedBy: CodingKeys.self)
            version = try c.decode(Int.self, forKey: .version); requests = try c.decode([Request].self, forKey: .requests)
            operations = try c.decode([Operation].self, forKey: .operations); items = try c.decode([T3QuestionAttachment].self, forKey: .items); garbage = try c.decode([String].self, forKey: .garbage)
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
        guard value.version == 1, value.requests.count <= 10_000, value.operations.count <= 100_000, value.items.count <= 1_000_000,
              Set(value.requests.map(\.id)).count == value.requests.count, Set(value.operations.map(\.id)).count == value.operations.count,
              Set(value.items.map(\.id)).count == value.items.count, Set(value.garbage).count == value.garbage.count,
              value.garbage.allSatisfy(Self.uuid) else { throw fail("Question attachment inventory is corrupt.") }
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
    private func commit(_ next: State) throws {
        try validate(next)
        let encoder = JSONEncoder(); encoder.outputFormatting = [.sortedKeys]
        do { try disk.replace(encoder.encode(next)); state = next }
        catch { state = nil; throw error } // Replacement may have completed; never assume rollback.
    }
    private func load() throws -> State {
        if let state { return state }
        var next: State
        if let bytes = try disk.load() { next = try JSONDecoder().decode(State.self, from: bytes); try validate(next) }
        else { next = State(); try commit(next) }
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
        if changed {
            try commit(next)
            try collectLocked()
            guard let state else { throw fail("Question attachment restart recovery failed.") }
            return state
        }
        state = next
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
            return T3QuestionAttachmentLease(item: item, read: { [self] in try locked {
                let bytes = try disk.read(id, limit: item.size)
                guard bytes.count == item.size, t3QuestionDigest(bytes) == item.digest else { throw fail("The attachment bytes changed.") }
                return bytes
            } }, release: { [self] in locked { releaseHold(id) } })
        }
    }
    private func releaseHold(_ id: String) { holds[id, default: 0] -= 1; if holds[id] == 0 { holds.removeValue(forKey: id) }; try? collectLocked() }
    func collect() throws { try locked { try collectLocked() } }
    private func collectLocked() throws {
        var next = try load(), changed = false
        for id in next.garbage where holds[id, default: 0] == 0 {
            try disk.remove(id); next.garbage.removeAll { $0 == id }; changed = true
        }
        if changed { try commit(next) }
    }
}
