#if os(iOS)
// @ref llp/1109.005-composer-and-transcript.decision.md#queued-command-construction
// Native-only byte preparation. The caller must retain/recheck the real queue owner.
import Foundation
import CoreFoundation
import CryptoKit
import Darwin

enum T3OutboxInlineTemplate {
    typealias Object = [String: Any]
    private static let imageLimit = 10 * 1024 * 1024
    private static let totalLimit = 80 * 1024 * 1024
    private static func failure() -> T3Failure {
        T3Failure(kind: "Outbox", message: "The inline images no longer match the captured queued message.")
    }
    private static func fields(_ value: Object, _ keys: [String]) -> Bool { Set(value.keys) == Set(keys) }
    private static func copy(_ value: Object) throws -> Object {
        guard JSONSerialization.isValidJSONObject(value) else { throw failure() }
        let bytes = try JSONSerialization.data(withJSONObject: value, options: [.sortedKeys])
        guard bytes.count <= T3Wire.maximumBytes, let result = try JSONSerialization.jsonObject(with: bytes) as? Object else { throw failure() }
        return result
    }
    private static func content(_ command: Object) throws -> Object {
        guard let payload = command["payload"] as? Object else { throw failure() }
        if command["method"] as? String == "orchestration.launchThread" {
            guard let initial = payload["initialMessage"] as? Object else { throw failure() }; return initial
        }
        return payload
    }
    /// Validate metadata before opening any file. Temporary references never escape this function.
    static func validateMetadata(_ record: Object, _ template: Object, captured: Bool) throws -> [Object] {
        guard T3MobileOutbox.validateRecord(record), fields(template, ["owner", "commandTemplate", "inline"]),
              let owner = template["owner"] as? Object,
              fields(owner, ["origin", "environmentId", "threadId", "messageId", "commandId"]),
              owner.keys.allSatisfy({ T3MobileOutbox.jsonEqual(owner[$0], record[$0]) }),
              var command = template["commandTemplate"] as? Object,
              fields(command, ["owner", "stage", "method", "payload"]), T3MobileOutbox.jsonEqual(owner, command["owner"]),
              command["stage"] as? String == "start-turn",
              let bindings = template["inline"] as? [Object], !bindings.isEmpty, bindings.count <= 100,
              let locals = record["attachments"] as? [Object], locals.count <= 100 else { throw failure() }
        var message = try content(command)
        guard var attachments = message["attachments"] as? [Object], attachments.count == locals.count else { throw failure() }
        var previous = -1, total = 0, inlineIndices = Set<Int>(), ids = Set<String>(), temporaryLocals = locals
        for binding in bindings {
            guard fields(binding, captured ? ["index", "localId", "sha256"] : ["index", "localId"]),
                  T3OutboxDeliveryReceipt.integer(binding["index"]), let index = binding["index"] as? Int,
                  index > previous, index < attachments.count, let id = binding["localId"] as? String,
                  id == locals[index]["id"] as? String, UUID(uuidString: id) != nil, id.utf8.count == 36,
                  ids.insert(id.lowercased()).inserted, locals[index]["kind"] as? String == "image" else { throw failure() }
            if captured {
                guard let digest = binding["sha256"] as? String, digest.utf8.count == 64,
                      digest.range(of: "^[0-9a-f]{64}$", options: .regularExpression) != nil else { throw failure() }
            }
            var attachment = attachments[index]
            guard fields(attachment, ["type", "name", "mimeType", "sizeBytes"]), attachment["type"] as? String == "image",
                  ["name", "mimeType", "sizeBytes"].allSatisfy({ T3MobileOutbox.jsonEqual(attachment[$0], locals[index][$0]) }),
                  let mime = attachment["mimeType"] as? String, mime.lowercased().hasPrefix("image/"),
                  T3OutboxDeliveryReceipt.integer(attachment["sizeBytes"]), let size = attachment["sizeBytes"] as? Int,
                  size <= imageLimit else { throw failure() }
            total += size; guard total <= totalLimit else { throw failure() }
            previous = index; inlineIndices.insert(index)
            attachment["id"] = id; attachments[index] = attachment
            temporaryLocals[index]["uploadId"] = id; temporaryLocals[index]["uploadEnvironmentId"] = record["environmentId"]
        }
        for index in attachments.indices where !inlineIndices.contains(index) {
            guard attachments[index]["id"] is String, attachments[index]["dataUrl"] == nil else { throw failure() }
            let image = attachments[index]["type"] as? String == "image"
            let mime = (attachments[index]["mimeType"] as? String ?? "").lowercased()
            // Pinned getProviderAttachmentLimitError counts these MIME types even
            // on file references. It neither trims nor removes MIME parameters.
            if image || ["image/gif", "image/jpeg", "image/png", "image/webp"].contains(mime) {
                guard T3OutboxDeliveryReceipt.integer(attachments[index]["sizeBytes"]),
                      let size = attachments[index]["sizeBytes"] as? Int,
                      !image || size <= imageLimit else { throw failure() }
                total += size; guard total <= totalLimit else { throw failure() }
            }
        }
        message["attachments"] = attachments
        if command["method"] as? String == "orchestration.launchThread" {
            var payload = command["payload"] as! Object; payload["initialMessage"] = message; command["payload"] = payload
        } else { command["payload"] = message }
        var temporaryRecord = record; temporaryRecord["attachments"] = temporaryLocals
        _ = try T3OutboxDeliveryReceipt.make(["record": temporaryRecord, "payload": command["payload"]!,
            "messageId": record["messageId"]!, "stage": command["stage"]!, "method": command["method"]!,
            "expectedToken": "inline-shape-validation", "expectedRevision": 1],
            origin: owner["origin"] as! String, environment: owner["environmentId"] as! String)
        return bindings
    }
    /// Every component below the trusted app root is opened without following symlinks.
    /// The digest, encoded data and durability checks use this same regular-file descriptor.
    private static func bytes(root: URL, id: String, size: Int, synchronize: Bool) throws -> Data {
        let rootFD = open(root.path, O_RDONLY | O_DIRECTORY | O_NOFOLLOW | O_CLOEXEC)
        guard rootFD >= 0 else { throw failure() }; defer { close(rootFD) }
        let snapshots = openat(rootFD, "snapshots", O_RDONLY | O_DIRECTORY | O_NOFOLLOW | O_CLOEXEC)
        guard snapshots >= 0 else { throw failure() }; defer { close(snapshots) }
        let drafts = openat(snapshots, "drafts", O_RDONLY | O_DIRECTORY | O_NOFOLLOW | O_CLOEXEC)
        guard drafts >= 0 else { throw failure() }; defer { close(drafts) }
        let fd = openat(drafts, id.lowercased(), O_RDONLY | O_NOFOLLOW | O_CLOEXEC | O_NONBLOCK)
        guard fd >= 0 else { throw failure() }; defer { close(fd) }
        var before = stat()
        guard fstat(fd, &before) == 0, before.st_mode & mode_t(S_IFMT) == mode_t(S_IFREG), before.st_size == size else { throw failure() }
        var result = Data(), buffer = [UInt8](repeating: 0, count: 64 * 1024)
        result.reserveCapacity(size)
        while result.count < size {
            let count = buffer.withUnsafeMutableBytes { Darwin.read(fd, $0.baseAddress!, min($0.count, size - result.count)) }
            if count < 0 && errno == EINTR { continue }
            guard count > 0 else { throw failure() }; result.append(contentsOf: buffer.prefix(count))
        }
        var extra: UInt8 = 0
        guard Darwin.read(fd, &extra, 1) == 0 else { throw failure() }
        if synchronize {
            guard fsync(fd) == 0, fsync(drafts) == 0, fsync(snapshots) == 0, fsync(rootFD) == 0 else { throw failure() }
        }
        var after = stat()
        guard fstat(fd, &after) == 0, after.st_size == before.st_size,
              after.st_mtimespec.tv_sec == before.st_mtimespec.tv_sec, after.st_mtimespec.tv_nsec == before.st_mtimespec.tv_nsec,
              after.st_ctimespec.tv_sec == before.st_ctimespec.tv_sec, after.st_ctimespec.tv_nsec == before.st_ctimespec.tv_nsec else { throw failure() }
        return result
    }
    static func capture(root: URL, record: Object, template: Object) throws -> Object {
        let original = try copy(record)
        var result = try copy(template)
        let bindings = try validateMetadata(original, result, captured: false)
        let locals = original["attachments"] as! [Object]
        result["inline"] = try bindings.map { binding -> Object in
            var value = binding; let index = value["index"] as! Int
            let data = try bytes(root: root, id: value["localId"] as! String, size: locals[index]["sizeBytes"] as! Int, synchronize: true)
            value["sha256"] = SHA256.hash(data: data).map { String(format: "%02x", $0) }.joined()
            return value
        }
        return try copy(result)
    }
    /// Native-only assets.persistChatAttachments payload. The caller must admit the actual RPC budget;
    /// this does not widen the existing wire/journal limits or return base64 through the JS bridge.
    static func expand(root: URL, record: Object, captured: Object) throws -> Object {
        let original = try copy(record), template = try copy(captured)
        let bindings = try validateMetadata(original, template, captured: true)
        let message = try content(template["commandTemplate"] as! Object)
        let attachments = message["attachments"] as! [Object]
        let images = try bindings.map { binding -> Object in
            let index = binding["index"] as! Int
            var image = attachments[index]
            let data = try bytes(root: root, id: binding["localId"] as! String, size: image["sizeBytes"] as! Int, synchronize: false)
            let digest = SHA256.hash(data: data).map { String(format: "%02x", $0) }.joined()
            guard digest == binding["sha256"] as? String else { throw failure() }
            image["dataUrl"] = "data:\(image["mimeType"] as! String);base64,\(data.base64EncodedString())"
            return image
        }
        let owner = template["owner"] as! Object
        return ["threadId": owner["threadId"]!, "messageId": owner["messageId"]!, "attachments": images]
    }
}
#endif
