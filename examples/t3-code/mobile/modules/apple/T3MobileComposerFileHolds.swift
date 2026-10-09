#if os(iOS)
// Saved ordinary file Undo ownership; no picker/paste authority or persistent Undo history.
// @ref llp/1109.005-composer-and-transcript.decision.md#composer-command-foundation
import UIKit
import CryptoKit
import CoreFoundation
import Darwin

/// Only the native port registry creates claims. End never waits for disk I/O.
final class T3ComposerFileClaim: @unchecked Sendable {
    let identity: T3ComposerIdentity
    private let lock = NSLock()
    private var ended = false
    // The coordinator mutex exclusively owns records; the small lock owns only ended.
    var records: [String: T3ComposerFileRecord] = [:]
    init(identity: T3ComposerIdentity) { self.identity = identity }
    func end() { lock.lock(); ended = true; lock.unlock() }
    var live: Bool { lock.lock(); defer { lock.unlock() }; return !ended }
}
struct T3ComposerFileRecord {
    let request: Data
    let fingerprint: String
    let receipt: [String: Any]
    var released = false
}
struct T3ComposerFileRequest {
    let action: String
    let requestId: String
    let generation: Int
    let identity: T3ComposerIdentity
    let object: [String: Any]
    init(_ object: [String: Any]) throws {
        guard let action = object["action"] as? String, ["acquire", "release"].contains(action),
              let requestId = object["requestId"] as? String, Self.uuid(requestId),
              let generation = Self.integer(object["generation"]),
              let identity = object["identity"] as? [String: Any],
              let decoded = try? JSONDecoder().decode(T3ComposerIdentity.self, from: JSONSerialization.data(withJSONObject: identity)),
              decoded.admitted, !decoded.mountId.isEmpty else { throw Self.error("The file hold request is invalid.") }
        self.action = action; self.requestId = requestId; self.generation = generation; self.identity = decoded; self.object = object
    }
    static func error(_ message: String) -> T3Failure { .init(kind: "FileHold", message: message) }
    static func uuid(_ value: String) -> Bool { value.count == 36 && UUID(uuidString: value)?.uuidString.lowercased() == value }
    static func integer(_ value: Any?) -> Int? {
        guard let n = value as? NSNumber, CFGetTypeID(n) != CFBooleanGetTypeID(), n.doubleValue >= 0,
              n.doubleValue <= 9_007_199_254_740_991, n.doubleValue.rounded() == n.doubleValue else { return nil }
        return n.intValue
    }
    static func encoded(_ value: Any) throws -> Data { try JSONSerialization.data(withJSONObject: value, options: [.sortedKeys, .fragmentsAllowed]) }
    var signature: Data { get throws { try Self.encoded(["generation": generation, "target": object["target"] ?? NSNull(), "file": object["file"] ?? NSNull()]) } }
    func file() throws -> [String: Any] {
        guard let target = object["target"] as? [String: String], let origin = target["origin"], !origin.isEmpty,
              let environment = target["environmentId"], !environment.isEmpty, let thread = target["threadId"], !thread.isEmpty,
              !thread.hasPrefix("new:"), let key = target["draftKey"], key == "\(environment):\(thread)", !key.contains("~queued-edit~"),
              let owner = try? JSONSerialization.jsonObject(with: Data(identity.owner.utf8)) as? [Any],
              owner.count == 5 || owner.count == 6, owner[0] as? String == "ordinary", owner[1] as? String == origin,
              owner[2] as? String == environment, Self.integer(owner[3]) == generation, owner[4] as? String == key,
              let file = object["file"] as? [String: Any], Self.validFile(file),
              file["source"] as? String == "attached", file["draftKey"] as? String == key,
              file["environmentId"] as? String == environment else { throw Self.error("Only this ordinary draft's saved attached files can be held.") }
        return file
    }
    static func validFile(_ file: [String: Any]) -> Bool {
        guard let id = file["id"] as? String, uuid(id), let size = integer(file["sizeBytes"]), size <= 50 * 1024 * 1024,
              ["id", "contextId", "draftKey", "environmentId", "name", "mimeType", "source"].allSatisfy({ (file[$0] as? String)?.isEmpty == false }),
              file["attachmentId"] is String, ["staged", "ready"].contains(file["status"] as? String ?? "") else { return false }
        for field in ["videoWidth", "videoHeight"] where file[field] != nil {
            guard let n = file[field] as? NSNumber, CFGetTypeID(n) != CFBooleanGetTypeID(), n.doubleValue.isFinite, n.doubleValue > 0 else { return false }
        }
        return true
    }
    static func saved(_ file: [String: Any], preferences: [String: Any]) throws {
        guard let inventory = preferences["composerFiles"] as? [[String: Any]], inventory.allSatisfy(validFile) else {
            throw error("Saved file ownership is invalid. Keep the original attachment.")
        }
        let matching = inventory.filter { $0["id"] as? String == file["id"] as? String }
        guard matching.count == 1, try encoded(matching[0]) == encoded(file) else { throw error("Save the exact named file before retaining its Undo bytes.") }
    }
    static func fingerprint(root: URL, file: [String: Any]) throws -> String {
        let directory = root.appendingPathComponent("composer-files"), id = file["id"] as! String
        let dir = Darwin.open(directory.path, O_RDONLY | O_DIRECTORY | O_NOFOLLOW)
        guard dir >= 0 else { throw error("The canonical attachment directory is unavailable.") }
        defer { Darwin.close(dir) }
        let fd = openat(dir, id, O_RDONLY | O_NOFOLLOW | O_NONBLOCK)
        guard fd >= 0 else { throw error("The canonical attachment is unavailable.") }
        defer { Darwin.close(fd) }
        var before = stat(), after = stat()
        guard fstat(fd, &before) == 0, (before.st_mode & S_IFMT) == S_IFREG,
              before.st_size == Int64(file["sizeBytes"] as! Int) else { throw error("The attachment bytes changed.") }
        var hash = SHA256(), bytes = [UInt8](repeating: 0, count: 65536), total: Int64 = 0
        while true {
            let count = Darwin.read(fd, &bytes, bytes.count)
            if count < 0 && errno == EINTR { continue }
            guard count >= 0 else { throw error("The attachment could not be read.") }
            if count == 0 { break }
            total += Int64(count)
            guard total <= before.st_size else { throw error("The attachment changed while being read.") }
            hash.update(data: Data(bytes.prefix(count)))
        }
        func metadata(_ s: stat) -> String { "\(s.st_dev):\(s.st_ino):\(s.st_size):\(s.st_mtimespec.tv_sec):\(s.st_mtimespec.tv_nsec):\(s.st_ctimespec.tv_sec):\(s.st_ctimespec.tv_nsec)" }
        guard fstat(fd, &after) == 0, metadata(before) == metadata(after), total == before.st_size else { throw error("The attachment changed while being read.") }
        var current = stat()
        guard fstatat(dir, id, &current, AT_SYMLINK_NOFOLLOW) == 0, metadata(current) == metadata(before), (current.st_mode & S_IFMT) == S_IFREG else { throw error("The attachment was replaced while being read.") }
        return metadata(before) + ":" + hash.finalize().map { String(format: "%02x", $0) }.joined()
    }
}

/// Main-thread native registry. Worker closures retain a real claim, never a view.
final class T3MobileComposerFileHolds {
    private struct Entry { weak var port: T3MobileComposerEditor?; let claim: T3ComposerFileClaim }
    private var entries: [ObjectIdentifier: Entry] = [:]
    private let coordinator: T3MobileQueuedEdit
    private let queue = DispatchQueue(label: "t3.composer-file-holds", qos: .utility)
    private var alive = true
    init(coordinator: T3MobileQueuedEdit) { self.coordinator = coordinator }
    private func retireMissingPorts() {
        let missing = entries.filter { $0.value.port == nil }
        for (key, entry) in missing { entries.removeValue(forKey: key); retire(entry.claim) }
    }
    func register(_ port: T3MobileComposerEditor) {
        retireMissingPorts()
        unregister(port)
        guard alive, let identity = port.composerIdentity else { return }
        entries[ObjectIdentifier(port)] = Entry(port: port, claim: .init(identity: identity))
    }
    func unregister(_ port: T3MobileComposerEditor) {
        guard let entry = entries.removeValue(forKey: ObjectIdentifier(port)) else { return }
        retire(entry.claim)
    }
    private func retire(_ claim: T3ComposerFileClaim) {
        claim.end()
        queue.async { [coordinator] in coordinator.endFileHoldOwner(claim) }
    }
    func perform(_ object: [String: Any], reply: @escaping ([String: Any]) -> Void) {
        retireMissingPorts()
        let generation = T3ComposerFileRequest.integer(object["generation"]) ?? 0
        do {
            let request = try T3ComposerFileRequest(object)
            let matches = entries.values.filter { $0.claim.identity == request.identity && $0.port != nil }
            guard alive, matches.count == 1, let entry = matches.first,
                  request.action == "release" || entry.port?.composerFileHoldEligible == true else {
                throw T3ComposerFileRequest.error("The captured native editor has ended or is unavailable.")
            }
            let claim = entry.claim
            queue.async { [coordinator] in
                let result: [String: Any]
                do { result = ["ok": true, "generation": generation, "value": try coordinator.performFileHold(request, claim: claim)] }
                catch { let error = error as? T3Failure ?? .init(kind: "FileHold", message: error.localizedDescription)
                    result = ["ok": false, "generation": generation, "error": error.json] }
                DispatchQueue.main.async { reply(result) }
            }
        } catch {
            let error = error as? T3Failure ?? .init(kind: "FileHold", message: error.localizedDescription)
            reply(["ok": false, "generation": generation, "error": error.json])
        }
    }
    func destroy() {
        guard alive else { return }; alive = false
        let claims = entries.values.map(\.claim); entries.removeAll()
        claims.forEach(retire)
    }
}
#endif
