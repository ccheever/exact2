#if os(iOS)
// @ref llp/1109.005-composer-and-transcript.decision.md#local-outbox-storage
// Local outbox storage only. The owning queued-edit coordinator supplies its lock.
import Foundation
import CoreFoundation
import CryptoKit
import Darwin

final class T3MobileOutbox {
    typealias Object = [String: Any]
    private let directory: URL
    private let replace: (Data, URL) throws -> Void
    private var holds: [String: Set<String>] = [:]
    init(root: URL, replace: @escaping (Data, URL) throws -> Void) {
        directory = root.appendingPathComponent("mobile-outbox", isDirectory: true)
        self.replace = replace
    }
    private func failure(_ message: String, uncertain: Bool = false) -> T3Failure {
        T3Failure(kind: "Persistence", message: message, uncertain: uncertain)
    }
    // ECMAScript String.trim, matching the TypeScript record decoder (not Foundation whitespace).
    private static let trimCharacters = CharacterSet(charactersIn: "\u{0009}\u{000A}\u{000B}\u{000C}\u{000D} \u{00A0}\u{1680}\u{2000}\u{2001}\u{2002}\u{2003}\u{2004}\u{2005}\u{2006}\u{2007}\u{2008}\u{2009}\u{200A}\u{2028}\u{2029}\u{202F}\u{205F}\u{3000}\u{FEFF}")
    private static func nonempty(_ value: Any?) -> Bool {
        guard let value = value as? String else { return false }
        return !value.isEmpty && value.trimmingCharacters(in: trimCharacters) == value
    }
    private static func integer(_ value: Any?, positive: Bool = false) -> Bool {
        guard let value = value as? NSNumber, CFGetTypeID(value) != CFBooleanGetTypeID() else { return false }
        let n = value.doubleValue
        return n.isFinite && n.rounded() == n && n >= (positive ? 1 : 0) && n <= 9_007_199_254_740_991
    }
    private static func boolean(_ value: Any?) -> Bool {
        guard let value = value as? NSNumber else { return false }
        return CFGetTypeID(value) == CFBooleanGetTypeID()
    }
    private static func positive(_ value: Any?) -> Bool {
        guard let value = value as? NSNumber, CFGetTypeID(value) != CFBooleanGetTypeID() else { return false }
        return value.doubleValue.isFinite && value.doubleValue > 0
    }
    private static func member(_ value: Any?, _ choices: [String]) -> Bool {
        guard let value = value as? String else { return false }; return choices.contains(value)
    }
    private static func matches(_ value: String, _ pattern: String) -> Bool {
        value.range(of: pattern, options: .regularExpression) != nil
    }
    static func canonicalOrigin(_ value: Any?) -> Bool {
        guard let value = value as? String, let url = URLComponents(string: value),
              let scheme = url.scheme, ["http", "https"].contains(scheme), let host = url.url?.host(),
              !host.isEmpty, url.user == nil, url.password == nil, url.path.isEmpty,
              url.query == nil, url.fragment == nil else { return false }
        let port = url.port, defaultPort = scheme == "https" ? 443 : 80
        // URL.host() keeps ASCII IDNA but omits IPv6 brackets; the serialized origin requires them.
        let serializedHost = host.contains(":") && !host.hasPrefix("[") ? "[\(host)]" : host
        guard port == nil || (0...65535).contains(port!), let canonicalHost = canonicalHost(serializedHost) else { return false }
        return value == "\(scheme)://\(canonicalHost)\(port == nil || port == defaultPort ? "" : ":\(port!)")"
    }
    private static func canonicalHost(_ host: String) -> String? {
        if host.hasPrefix("[") {
            var address = in6_addr()
            let input = String(host.dropFirst().dropLast())
            guard input.withCString({ inet_pton(AF_INET6, $0, &address) }) == 1 else { return nil }
            let bytes = withUnsafeBytes(of: address) { Array($0) }
            let words = stride(from: 0, to: 16, by: 2).map { Int(bytes[$0]) * 256 + Int(bytes[$0 + 1]) }
            var bestStart = -1, bestLength = 1, start = 0
            while start < 8 {
                if words[start] != 0 { start += 1; continue }
                var end = start
                while end < 8 && words[end] == 0 { end += 1 }
                if end - start > bestLength { bestStart = start; bestLength = end - start }
                start = end
            }
            let parts = words.map { String($0, radix: 16) }
            if bestStart < 0 { return "[" + parts.joined(separator: ":") + "]" }
            return "[" + parts[..<bestStart].joined(separator: ":") + "::" + parts[(bestStart + bestLength)...].joined(separator: ":") + "]"
        }
        guard host.unicodeScalars.allSatisfy({ $0.isASCII }), !host.contains("%"), !host.contains("\\") else { return nil }
        let last = host.split(separator: ".").last.map(String.init) ?? ""
        if matches(last, "^[0-9]+$") || matches(last, "^0[xX][0-9a-fA-F]+$") {
            let parts = host.split(separator: ".", omittingEmptySubsequences: false)
            guard parts.count == 4, parts.allSatisfy({ part in
                guard let number = Int(part), (0...255).contains(number) else { return false }
                return String(number) == part
            }) else { return nil }
        }
        return host.lowercased()
    }
    static func validateRecord(_ record: Object) -> Bool {
        guard integer(record["schemaVersion"], positive: true), record["schemaVersion"] as? Int == 1,
              canonicalOrigin(record["origin"]), ["environmentId", "threadId", "messageId", "commandId"].allSatisfy({ nonempty(record[$0]) }),
              record["text"] is String, let attachments = record["attachments"] as? [Object],
              JSONSerialization.isValidJSONObject(record) else { return false }
        var ids = Set<String>()
        for file in attachments {
            guard let id = file["id"] as? String, id.utf16.count == 36, matches(id, "^[0-9a-fA-F]{8}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{12}$"),
                  ids.insert(id.lowercased()).inserted, nonempty(file["name"]), nonempty(file["mimeType"]), integer(file["sizeBytes"]),
                  let upload = file["uploadId"] as? String, member(file["status"], ["staged", "uploading", "ready", "error"]),
                  file["status"] as? String != "ready" || nonempty(upload) else { return false }
            if let owner = file["uploadEnvironmentId"], !nonempty(owner) { return false }
            if !upload.isEmpty && file["uploadEnvironmentId"] as? String != record["environmentId"] as? String { return false }
            if file["kind"] as? String == "image" {
                if let source = file["source"], !(source is Object) { return false }
            } else {
                guard file["kind"] as? String == "file", nonempty(file["contextId"]), nonempty(file["source"]) else { return false }
                for field in ["videoWidth", "videoHeight"] where file[field] != nil { if !positive(file[field]) { return false } }
            }
        }
        if let context = record["context"] {
            guard let context = context as? Object, integer(context["version"], positive: true), context["version"] as? Int == 1,
                  let records = context["records"] as? [Object] else { return false }
            var contextIDs = Set<String>()
            for item in records {
                guard integer(item["version"], positive: true), item["version"] as? Int == 1,
                      let id = item["contextId"] as? String, nonempty(id), matches(id, "^[a-zA-Z0-9_-]{1,128}$"), contextIDs.insert(id).inserted,
                      let label = item["label"] as? String, label.utf16.count <= 200,
                      let kind = item["kind"] as? String, matches(kind, "^[a-z][a-z0-9-]{0,39}$") else { return false }
            }
        }
        if let model = record["modelSelection"] {
            guard let model = model as? Object, nonempty(model["instanceId"]), nonempty(model["model"]) else { return false }
            if let options = model["options"] {
                guard let options = options as? [Object], options.allSatisfy({ nonempty($0["id"]) && (boolean($0["value"]) || nonempty($0["value"])) }) else { return false }
            }
        }
        for (field, values) in [("runtimeMode", ["approval-required", "auto-accept-edits", "auto", "full-access"]),
                                ("interactionMode", ["default", "plan"]), ("dispatchMode", ["auto", "queue", "steer", "restart"])] {
            if record[field] != nil && !member(record[field], values) { return false }
        }
        if let creation = record["creation"] {
            guard let creation = creation as? Object, nonempty(creation["projectId"]), member(creation["workspaceMode"], ["local", "worktree"]),
                  creation["branch"] is NSNull || creation["branch"] is String,
                  creation["worktreePath"] is NSNull || creation["worktreePath"] is String else { return false }
            for field in ["projectTitle", "projectCwd"] where creation[field] != nil { if !(creation[field] is String) { return false } }
            if creation["startFromOrigin"] != nil && !boolean(creation["startFromOrigin"]) { return false }
        }
        guard let date = record["createdAt"] as? String, date.utf16.count == 24,
              matches(date, "^[0-9]{4}-[0-9]{2}-[0-9]{2}T[0-9]{2}:[0-9]{2}:[0-9]{2}\\.[0-9]{3}Z$") else { return false }
        // JavaScript's ISO form includes astronomical year 0000; Foundation's formatter does not.
        let fields = date.split(whereSeparator: { "-T:.Z".contains($0) }).compactMap { Int($0) }
        guard fields.count == 7 else { return false }
        let year = fields[0], month = fields[1], day = fields[2]
        let leap = year % 4 == 0 && (year % 100 != 0 || year % 400 == 0)
        let days = [31, leap ? 29 : 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
        return (1...12).contains(month) && (1...days[month - 1]).contains(day)
            && (0...23).contains(fields[3]) && (0...59).contains(fields[4]) && (0...59).contains(fields[5])
    }
    private func url(_ id: String) -> URL {
        let digest = SHA256.hash(data: Data(id.utf8)).map { String(format: "%02x", $0) }.joined()
        return directory.appendingPathComponent(digest + ".json")
    }
    private func decode(_ file: URL) throws -> Object {
        let properties = try file.resourceValues(forKeys: [.isRegularFileKey, .isSymbolicLinkKey, .fileSizeKey])
        guard properties.isRegularFile == true, properties.isSymbolicLink != true,
              (properties.fileSize ?? Int.max) <= T3Wire.maximumBytes,
              let value = try JSONSerialization.jsonObject(with: Data(contentsOf: file)) as? Object,
              Self.integer(value["version"], positive: true), value["version"] as? Int == 1,
              let id = value["messageId"] as? String, Self.nonempty(id), url(id).lastPathComponent == file.lastPathComponent,
              Self.integer(value["revision"], positive: true), Self.member(value["state"], ["active", "deleted", "pending"]) else {
            throw failure("An outbox record or ownership envelope is invalid.")
        }
        let fields = value["state"] as? String == "pending" ? ["previous", "proposed"] : ["record"]
        for field in fields {
            guard let item = value[field] else { throw failure("An outbox record is missing its ownership data.") }
            if item is NSNull { continue }
            guard let record = item as? Object, record["messageId"] as? String == id, Self.validateRecord(record) else {
                throw failure("An outbox record contains invalid attachment ownership.")
            }
        }
        if value["state"] as? String == "active" && !(value["record"] is Object) || value["state"] as? String == "deleted" && !(value["record"] is NSNull) {
            throw failure("An outbox record has inconsistent state.")
        }
        if let removed = value["removed"] {
            guard value["state"] as? String == "deleted", let record = removed as? Object,
                  record["messageId"] as? String == id, Self.validateRecord(record) else { throw failure("An outbox deletion receipt is invalid.") }
        }
        return value
    }
    private func load(_ id: String) throws -> Object? {
        let file = url(id)
        do { return try decode(file) }
        catch let error as CocoaError where error.code == .fileReadNoSuchFile { return nil }
    }
    private func envelopes() -> (values: [Object], errors: [Object]) {
        var values: [Object] = [], errors: [Object] = []
        let files: [URL]
        do { files = try FileManager.default.contentsOfDirectory(at: directory, includingPropertiesForKeys: [.isRegularFileKey]) }
        catch let error as CocoaError where error.code == .fileReadNoSuchFile { return ([], []) }
        catch { return ([], [["file": "mobile-outbox", "message": "The outbox directory could not be read."]]) }
        for file in files.sorted(by: { $0.lastPathComponent < $1.lastPathComponent }) where file.pathExtension == "json" {
            do { values.append(try decode(file)) }
            catch { errors.append(["file": file.lastPathComponent, "message": "This outbox record could not be read; its attachment ownership is unknown."]) }
        }
        return (values, errors)
    }
    private func save(_ value: Object) throws {
        guard JSONSerialization.isValidJSONObject(value), let id = value["messageId"] as? String else { throw failure("The outbox record cannot be serialized.") }
        let data = try JSONSerialization.data(withJSONObject: value, options: [.sortedKeys])
        guard data.count <= T3Wire.maximumBytes else { throw failure("The outbox record is too large.") }
        do { try replace(data, url(id)) }
        catch { throw failure("The outbox mutation may have reached disk. Read and confirm its saved revision before continuing.", uncertain: true) }
    }
    private func payloads(_ value: Object) -> [Object] {
        ["record", "previous", "proposed", "removed"].compactMap { value[$0] as? Object }
    }
    private func held(_ id: String) -> Bool { !(holds[id] ?? []).isEmpty }
    func inventoryHolds(_ attachmentID: String) throws -> Bool {
        let inventory = envelopes()
        guard inventory.errors.isEmpty else { throw failure("Outbox attachment ownership is incomplete. Keep local files until its records can be read.") }
        return inventory.values.flatMap(payloads).contains { record in
            (record["attachments"] as? [Object] ?? []).contains { ($0["id"] as? String)?.lowercased() == attachmentID.lowercased() }
        }
    }
    func read() -> Object {
        let inventory = envelopes()
        return ["complete": inventory.errors.isEmpty, "errors": inventory.errors,
                "records": inventory.values.compactMap { value -> Object? in
                    guard let record = value["record"] as? Object ?? value["proposed"] as? Object ?? value["previous"] as? Object else { return nil }
                    return ["record": record, "revision": value["revision"]!, "pending": value["state"] as? String == "pending", "held": held(value["messageId"] as! String)]
                },
                "mutations": inventory.values.filter { $0["state"] as? String == "pending" },
                "removals": inventory.values.filter { $0["removed"] != nil }]
    }
    private func identity(_ request: Object) throws -> String {
        guard let id = request["messageId"] as? String, Self.nonempty(id) else { throw failure("Choose an outbox message.") }; return id
    }
    private func expected(_ request: Object) throws -> Int {
        guard Self.integer(request["expectedRevision"], positive: true), let revision = request["expectedRevision"] as? Int else { throw failure("Choose an exact outbox revision.") }; return revision
    }
    private func mutation(_ id: String, current: Object?, next: Object?) throws -> Object {
        guard current?["state"] as? String != "pending", current?["removed"] == nil else { throw failure("Confirm the previous outbox mutation or cleanup before changing this message.") }
        let revision = (current?["revision"] as? Int ?? 0) + 1
        guard Self.integer(revision, positive: true) else { throw failure("The outbox revision is exhausted.") }
        let value: Object = ["version": 1, "messageId": id, "revision": revision, "state": "pending",
                             "previous": current?["record"] ?? NSNull(), "proposed": next.map { $0 as Any } ?? NSNull()]
        try save(value)
        return ["messageId": id, "applied": true, "removed": next == nil, "pending": true, "revision": revision,
                "record": next.map { $0 as Any } ?? current?["record"] ?? NSNull()]
    }
    func perform(_ request: Object) throws -> Object {
        let action = request["action"] as? String ?? ""
        if action == "read" { return read() }
        if action == "clearEnvironment" {
            guard let environment = request["environmentId"] as? String, Self.nonempty(environment), Self.canonicalOrigin(request["origin"]) else { throw failure("Choose an exact outbox environment.") }
            let inventory = envelopes()
            guard inventory.errors.isEmpty else { throw failure("The outbox must load completely before clearing an environment.") }
            var removals: [Object] = [], errors: [Object] = []
            for value in inventory.values {
                if value["state"] as? String == "pending", payloads(value).contains(where: {
                    $0["environmentId"] as? String == environment && $0["origin"] as? String == request["origin"] as? String
                }) {
                    errors.append(["messageId": value["messageId"]!, "message": "Confirm the pending mutation before clearing this environment."]); continue
                }
                guard let record = value["record"] as? Object, record["environmentId"] as? String == environment,
                      record["origin"] as? String == request["origin"] as? String else { continue }
                do { removals.append(try mutation(value["messageId"] as! String, current: value, next: nil)) }
                catch { errors.append(["messageId": value["messageId"]!, "message": "The outbox removal was not confirmed."]) }
            }
            return ["removals": removals, "errors": errors]
        }
        let id: String
        if action == "enqueue" || action == "update" {
            guard let next = request["record"] as? Object, Self.validateRecord(next), let key = next["messageId"] as? String else { throw failure("The outbox record is invalid.") }
            id = key; let prior = try load(id)
            if action == "update" {
                let revision = try expected(request)
                guard prior?["state"] as? String == "active", prior?["revision"] as? Int == revision else {
                    return ["applied": false, "revision": prior?["revision"] ?? 0]
                }
            }
            return try mutation(id, current: prior, next: next)
        }
        id = try identity(request)
        let prior = try load(id), revision = prior?["revision"] as? Int ?? 0
        if action == "releaseHold" {
            guard let owner = request["owner"] as? String, Self.nonempty(owner) else { throw failure("Choose the pending task editor owner.") }
            let released = holds[id]?.remove(owner) != nil
            if holds[id]?.isEmpty == true { holds.removeValue(forKey: id) }
            return ["released": released]
        }
        if action == "hold" {
            let expected = try expected(request)
            guard let owner = request["owner"] as? String, Self.nonempty(owner) else { throw failure("Choose the pending task editor owner.") }
            guard prior?["state"] as? String == "active", revision == expected else { return ["held": false, "revision": revision] }
            holds[id, default: []].insert(owner)
            return ["held": true, "record": prior!["record"]!, "revision": revision]
        }
        if action == "remove" {
            if request["expectedRevision"] != nil {
                let expected = try expected(request)
                if revision != expected { return ["removed": false, "revision": revision] }
            }
            if request["requireUnheld"] as? Bool == true && held(id) { return ["removed": false, "revision": revision] }
            guard prior?["state"] as? String == "active" else { return ["removed": false, "revision": revision] }
            return try mutation(id, current: prior, next: nil)
        }
        if action == "confirm" {
            let expected = try expected(request)
            guard let prior, revision == expected else { return ["current": false, "revision": revision] }
            if prior["state"] as? String != "pending" {
                return ["current": prior["state"] as? String == "active" && !held(id), "revision": revision, "record": prior["record"] ?? NSNull()]
            }
            guard Self.member(request["decision"], ["commit", "rollback"]) else { throw failure("Confirm or roll back the exact saved outbox mutation.") }
            let record = prior[request["decision"] as? String == "commit" ? "proposed" : "previous"] as? Object
            var next: Object = ["version": 1, "messageId": id, "revision": revision,
                                "state": record == nil ? "deleted" : "active", "record": record.map { $0 as Any } ?? NSNull()]
            if record == nil, request["decision"] as? String == "commit", let removed = prior["previous"] as? Object { next["removed"] = removed }
            try save(next)
            return ["current": record != nil && !held(id), "removed": next["removed"] != nil, "revision": revision, "record": record.map { $0 as Any } ?? next["removed"] ?? NSNull()]
        }
        if action == "completeRemoval" {
            let expected = try expected(request)
            guard var prior, prior["state"] as? String == "deleted", revision == expected else { return ["completed": false, "revision": revision] }
            prior.removeValue(forKey: "removed"); try save(prior)
            return ["completed": true, "revision": revision]
        }
        throw failure("Unknown local outbox action.")
    }
    // Persist releases in the coordinator before a confirmation removes their owner.
    func releaseCandidates(_ request: Object) throws -> [Object] {
        guard ["confirm", "completeRemoval"].contains(request["action"] as? String ?? "") else { return [] }
        let id = try identity(request), revision = try expected(request)
        guard let prior = try load(id), prior["revision"] as? Int == revision else { return [] }
        if request["action"] as? String == "completeRemoval" { return (prior["removed"] as? Object)?["attachments"] as? [Object] ?? [] }
        guard prior["state"] as? String == "pending", Self.member(request["decision"], ["commit", "rollback"]) else { return [] }
        let commit = request["decision"] as? String == "commit"
        let kept = prior[commit ? "proposed" : "previous"] as? Object
        // A committed deletion keeps its own receipt until editor cleanup completes.
        if kept == nil && commit { return [] }
        let ids = Set((kept?["attachments"] as? [Object] ?? []).compactMap { ($0["id"] as? String)?.lowercased() })
        return payloads(prior).flatMap { $0["attachments"] as? [Object] ?? [] }.filter { !ids.contains(($0["id"] as? String ?? "").lowercased()) }
    }
}
#endif
