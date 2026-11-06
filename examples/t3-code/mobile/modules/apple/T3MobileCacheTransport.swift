import Foundation
import CoreFoundation

final class T3MobileCacheTransport {
    typealias Object = [String: Any]
    private static let frameBytes = 384 * 1_024
    private static let pieceSize = 48 * 1_024
    private static let textBytes = 16 * 1_024 * 1_024
    private static let stagingBytes = 32 * 1_024 * 1_024
    private static let stale: Object = ["written": false, "stale": true]

    private struct Scope {
        var environment: String?
        var kind: String?
        var key: String?

        init(_ value: Object) {
            environment = value["environmentId"] as? String
            kind = value["kind"] as? String
            key = value["key"] as? String
        }

        func intersects(_ other: Scope) -> Bool {
            for (left, right) in [(environment, other.environment), (kind, other.kind), (key, other.key)] {
                if let left, let right, !left.utf8.elementsEqual(right.utf8) { return false }
            }
            return true
        }
    }

    private final class Upload {
        let target: Object
        let units: Int
        let ticket: String
        var data = Data()
        var receivedUnits = 0
        var pieces = [Range<Int>]()
        var isWrite: Bool { target["action"] as? String == "write" }

        init(target: Object, units: Int, ticket: String) {
            self.target = target
            self.units = units
            self.ticket = ticket
        }
    }

    private final class Outgoing {
        let data: Data
        let pieces: [Range<Int>]
        let identity: Object?
        let ticket: String?
        var served = Set<Int>()

        init(data: Data, identity: Object?, ticket: String?) {
            self.data = data
            self.identity = identity
            self.ticket = ticket
            var pieces = [Range<Int>](), start = 0
            while start < data.count {
                var end = min(start + T3MobileCacheTransport.pieceSize, data.count)
                while end < data.count, data[end] & 0xc0 == 0x80 { end -= 1 }
                pieces.append(start..<end)
                start = end
            }
            self.pieces = pieces
        }
    }

    private enum State {
        case upload(Upload)
        case outgoing(Outgoing)
        case finished(bytes: Int, result: Object)
        case readFinished(identity: Object?, ticket: String?)
        case staleWrite

        var byteCount: Int {
            switch self {
            case .upload(let upload): return upload.data.count
            case .outgoing(let outgoing): return outgoing.data.count
            default: return 0
            }
        }
    }

    private final class Entry {
        let scope: Scope
        let deadline: DispatchTime
        var state: State

        init(scope: Scope, deadline: DispatchTime, state: State) {
            self.scope = scope
            self.deadline = deadline
            self.state = state
        }
    }

    private let store: T3MobileClientCache
    private let queue = DispatchQueue(label: "com.exact.t3code.cache-transport")
    private let lifetime: DispatchTimeInterval
    private let didExpire: (() -> Void)?
    private var timer: DispatchSourceTimer?
    private var entries = [String: Entry]()
    private var retainedBytes = 0
    private var closed = false

    init(store: T3MobileClientCache, lifetime: DispatchTimeInterval = .seconds(120), didExpire: (() -> Void)? = nil) {
        self.store = store
        self.lifetime = lifetime
        self.didExpire = didExpire
    }

    deinit { timer?.cancel() }

    func close() {
        queue.sync {
            closed = true
            entries.removeAll()
            retainedBytes = 0
            timer?.cancel()
            timer = nil
        }
    }

    func clearForgottenEnvironment(_ environmentId: String) throws {
        try queue.sync {
            let value: Object = ["action": "clear", "environmentId": environmentId]
            _ = try store.request(value)
            revoke(Scope(value), writes: true)
        }
    }

    func request(_ value: Object) throws -> Object {
        try queue.sync {
            guard !closed else { throw Self.fail("Client cache transport is closed.") }
            expire()
            defer { scheduleExpiry() }
            do {
                guard JSONSerialization.isValidJSONObject(value),
                      try JSONSerialization.data(withJSONObject: value).count <= Self.frameBytes else {
                    throw Self.fail("Client cache frame is too large or malformed.")
                }
                switch try Self.string(value, "action", maximum: 32) {
                case "textBegin": return try begin(value)
                case "textAppend": return try append(value)
                case "textFinish": return try finish(value)
                case "textPart": return try part(value)
                case "textReadFinish": return try readFinish(value)
                case "textRelease":
                    remove(try Self.string(value, "id", maximum: 128))
                    return ["released": true]
                default: return try perform(value)
                }
            } catch {
                if let id = value["id"] as? String { remove(id) }
                throw error
            }
        }
    }

    private static func fail(_ message: String) -> NSError {
        NSError(domain: "T3MobileCacheTransport", code: 1, userInfo: [NSLocalizedDescriptionKey: message])
    }

    private static func string(_ value: Object, _ field: String, maximum: Int) throws -> String {
        guard let result = value[field] as? String, !result.isEmpty,
              result.utf8.count <= maximum, !result.contains("\0") else {
            throw fail("Invalid client cache transfer \(field).")
        }
        return result
    }

    private static func integer(_ value: Object, _ field: String, maximum: Int) throws -> Int {
        guard let number = value[field] as? NSNumber, CFGetTypeID(number) != CFBooleanGetTypeID(),
              number.doubleValue.isFinite, number.doubleValue.rounded() == number.doubleValue,
              number.doubleValue >= 0, number.doubleValue <= Double(maximum) else {
            throw fail("Invalid client cache transfer \(field).")
        }
        return number.intValue
    }

    private static func piece(_ value: Object) throws -> (Data, Int) {
        guard let text = value["text"] as? NSString, (1...pieceSize).contains(text.length) else {
            throw fail("Invalid client cache text piece.")
        }
        var index = 0
        while index < text.length {
            let unit = text.character(at: index)
            if (0xd800...0xdbff).contains(unit) {
                index += 1
                guard index < text.length, (0xdc00...0xdfff).contains(text.character(at: index)) else {
                    throw fail("Client cache text contains an unpaired surrogate.")
                }
            } else if (0xdc00...0xdfff).contains(unit) {
                throw fail("Client cache text contains an unpaired surrogate.")
            }
            index += 1
        }
        return (Data((text as String).utf8), text.length)
    }

    private func ticket(_ identity: Object) throws -> String {
        try store.request(identity.merging(["action": "ticket"]) { _, next in next })["ticket"] as! String
    }

    private func begin(_ value: Object) throws -> Object {
        let units = try Self.integer(value, "units", maximum: Self.textBytes)
        guard let target = value["target"] as? Object,
              target["payload"] == nil, target["expectedPayload"] == nil,
              let action = target["action"] as? String, action == "write" || action == "remove" else {
            throw Self.fail("Invalid client cache transfer target.")
        }
        let supplied: String?
        if action == "write" {
            guard try Self.integer(target, "schemaVersion", maximum: 1) == 1 else {
                throw Self.fail("Invalid client cache schema version.")
            }
            supplied = try Self.string(target, "ticket", maximum: 256)
        } else { supplied = nil }
        let current = try ticket(target)
        if let supplied, supplied != current { return Self.stale }
        let upload = Upload(target: target, units: units, ticket: current)
        let id = try insert(.upload(upload), scope: Scope(target))
        return ["transfer": ["kind": "upload", "id": id]]
    }

    private func entry(_ value: Object) throws -> (String, Entry) {
        let id = try Self.string(value, "id", maximum: 128)
        guard let entry = entries[id] else { throw Self.fail("Client cache transfer is unavailable.") }
        return (id, entry)
    }

    private func checkUpload(_ upload: Upload, entry: Entry) throws -> Bool {
        if try ticket(upload.target) == upload.ticket { return true }
        if upload.isWrite {
            retainedBytes -= entry.state.byteCount
            entry.state = .staleWrite
            return false
        }
        throw Self.fail("Client cache transfer is stale.")
    }

    private func append(_ value: Object) throws -> Object {
        let index = try Self.integer(value, "index", maximum: 1_023)
        let (data, units) = try Self.piece(value)
        let (_, entry) = try entry(value)
        if case .staleWrite = entry.state { return Self.stale }
        guard case .upload(let upload) = entry.state else { throw Self.fail("Invalid client cache transfer phase.") }
        guard try checkUpload(upload, entry: entry) else { return Self.stale }
        if index < upload.pieces.count {
            guard upload.data[upload.pieces[index]].elementsEqual(data) else {
                throw Self.fail("Conflicting client cache text replay.")
            }
        } else {
            guard index == upload.pieces.count, upload.receivedUnits + units <= upload.units,
                  upload.data.count + data.count <= Self.textBytes,
                  retainedBytes + data.count <= Self.stagingBytes else {
                throw Self.fail("Client cache text exceeds its transfer bounds.")
            }
            upload.pieces.append(upload.data.count..<(upload.data.count + data.count))
            upload.data.append(data)
            upload.receivedUnits += units
            retainedBytes += data.count
        }
        return ["next": index + 1]
    }

    private func finish(_ value: Object) throws -> Object {
        let bytes = try Self.integer(value, "bytes", maximum: Self.textBytes)
        let (id, entry) = try entry(value)
        switch entry.state {
        case .staleWrite: return Self.stale
        case .finished(let expected, let result):
            guard bytes == expected else { throw Self.fail("Conflicting client cache finish replay.") }
            return result
        case .upload(let upload):
            guard try checkUpload(upload, entry: entry) else { return Self.stale }
            guard upload.receivedUnits == upload.units, upload.data.count == bytes,
                  let text = String(data: upload.data, encoding: .utf8) else {
                throw Self.fail("Client cache text transfer is incomplete.")
            }
            let field = upload.isWrite ? "payload" : "expectedPayload"
            let result = try perform(upload.target.merging([field: text]) { _, next in next }, excluding: id)
            retainedBytes -= entry.state.byteCount
            entry.state = .finished(bytes: bytes, result: result)
            return result
        default: throw Self.fail("Invalid client cache transfer phase.")
        }
    }

    private func validateRead(identity: Object?, ticket captured: String?) throws {
        if let identity, let captured, try ticket(identity) != captured {
            throw Self.fail("Client cache read transfer is stale.")
        }
    }

    private func part(_ value: Object) throws -> Object {
        let index = try Self.integer(value, "index", maximum: 1_023)
        let (_, entry) = try entry(value)
        guard case .outgoing(let outgoing) = entry.state, index < outgoing.pieces.count else {
            throw Self.fail("Invalid client cache transfer part.")
        }
        try validateRead(identity: outgoing.identity, ticket: outgoing.ticket)
        let text = String(data: outgoing.data[outgoing.pieces[index]], encoding: .utf8)!
        outgoing.served.insert(index)
        return ["index": index, "text": text]
    }

    private func readFinish(_ value: Object) throws -> Object {
        let (_, entry) = try entry(value)
        switch entry.state {
        case .outgoing(let outgoing):
            try validateRead(identity: outgoing.identity, ticket: outgoing.ticket)
            guard outgoing.served.count == outgoing.pieces.count else {
                throw Self.fail("Client cache read transfer is incomplete.")
            }
            retainedBytes -= entry.state.byteCount
            entry.state = .readFinished(identity: outgoing.identity, ticket: outgoing.ticket)
        case .readFinished(let identity, let captured): try validateRead(identity: identity, ticket: captured)
        default: throw Self.fail("Invalid client cache transfer phase.")
        }
        return ["complete": true]
    }

    private func perform(_ value: Object, excluding id: String? = nil) throws -> Object {
        let action = value["action"] as? String
        let captured = action == "read" ? try ticket(value) : nil
        let result = try store.request(value)
        let scope = Scope(value)
        switch action {
        case "write" where result["changed"] as? Bool == true: revoke(scope, writes: false, excluding: id)
        case "clear", "clearKind": revoke(scope, writes: true, excluding: id)
        case "remove" where value["expectedPayload"] == nil || (result["removed"] as? Int ?? 0) > 0:
            revoke(scope, writes: true, excluding: id)
        case "read" where result["discarded"] as? Bool == true: revoke(scope, writes: true, excluding: id)
        default: break
        }
        if let record = result["record"] as? Object, let payload = record["payload"] as? String {
            if payload.utf8.count <= Self.pieceSize { return result }
            var metadata = record
            metadata.removeValue(forKey: "payload")
            return try outgoing(payload, kind: "payload", metadata: metadata, scope: scope, identity: value, ticket: captured)
        }
        let encoded = try JSONSerialization.data(withJSONObject: result)
        if encoded.count <= Self.frameBytes { return result }
        guard encoded.count <= Self.stagingBytes else { throw Self.fail("Client cache reply exceeds transfer capacity.") }
        return try outgoing(String(data: encoded, encoding: .utf8)!, kind: "json", scope: scope)
    }

    private func outgoing(_ text: String, kind: String, metadata: Object? = nil, scope: Scope,
                          identity: Object? = nil, ticket: String? = nil) throws -> Object {
        let body = Outgoing(data: Data(text.utf8), identity: identity, ticket: ticket)
        let id = try insert(.outgoing(body), scope: scope)
        var manifest: Object = ["kind": kind, "id": id, "units": text.utf16.count,
                                "bytes": body.data.count, "parts": body.pieces.count]
        if let metadata { manifest["metadata"] = metadata }
        return ["transfer": manifest]
    }

    private func insert(_ state: State, scope: Scope) throws -> String {
        guard entries.count < 8, retainedBytes + state.byteCount <= Self.stagingBytes else {
            throw Self.fail("Client cache transfer capacity is exhausted.")
        }
        let id = UUID().uuidString
        entries[id] = Entry(scope: scope, deadline: .now() + lifetime, state: state)
        retainedBytes += state.byteCount
        return id
    }

    private func revoke(_ scope: Scope, writes: Bool, excluding excluded: String? = nil) {
        for (id, entry) in entries where id != excluded && entry.scope.intersects(scope) {
            switch entry.state {
            case .upload(let upload) where writes:
                if upload.isWrite {
                    retainedBytes -= entry.state.byteCount
                    entry.state = .staleWrite
                } else { remove(id) }
            case .outgoing, .readFinished: remove(id)
            default: break
            }
        }
        scheduleExpiry()
    }

    private func remove(_ id: String) {
        if let entry = entries.removeValue(forKey: id) { retainedBytes -= entry.state.byteCount }
    }

    private func expire() {
        let now = DispatchTime.now()
        var expired = false
        for (id, entry) in entries where entry.deadline <= now {
            remove(id)
            expired = true
        }
        if expired { didExpire?() }
    }

    private func scheduleExpiry() {
        guard let deadline = entries.values.map(\.deadline).min(), !closed else {
            timer?.cancel()
            timer = nil
            return
        }
        if timer == nil {
            let source = DispatchSource.makeTimerSource(queue: queue)
            source.setEventHandler { [weak self] in
                self?.expire()
                self?.scheduleExpiry()
            }
            source.schedule(deadline: deadline)
            source.resume()
            timer = source
        } else { timer?.schedule(deadline: deadline) }
    }
}
