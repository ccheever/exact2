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
    init(root: URL, replace: @escaping (Data, URL) throws -> Void) {
        directory = root.appendingPathComponent("mobile-outbox", isDirectory: true)
        self.replace = replace
    }
    private func failure(_ message: String, uncertain: Bool = false) -> T3Failure {
        T3Failure(kind: "Persistence", message: message, uncertain: uncertain)
    }
    // ECMAScript String.trim, matching the TypeScript record decoder (not Foundation whitespace).
    static let trimCharacters = CharacterSet(charactersIn: "\u{0009}\u{000A}\u{000B}\u{000C}\u{000D} \u{00A0}\u{1680}\u{2000}\u{2001}\u{2002}\u{2003}\u{2004}\u{2005}\u{2006}\u{2007}\u{2008}\u{2009}\u{200A}\u{2028}\u{2029}\u{202F}\u{205F}\u{3000}\u{FEFF}")
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
    static func validateMutation(_ request: Object, id: String) -> Bool {
        guard nonempty(request["mutationId"]), request["messageId"] as? String == id,
              member(request["operation"], ["enqueue", "update", "remove"]) else { return false }
        if request["operation"] as? String != "remove" {
            guard let record = request["record"] as? Object, record["messageId"] as? String == id, validateRecord(record) else { return false }
        }
        if let transfer = request["transfer"] {
            guard request["operation"] as? String == "enqueue", let claim = transfer as? Object,
                  validateTransfer(claim, id: id), claim["state"] as? String == "prepared",
                  claim["mutationId"] as? String == request["mutationId"] as? String,
                  jsonEqual(claim["record"], request["record"]) else { return false }
        }
        if let delivery = request["deliveryCleanup"] {
            guard let delivery = delivery as? Object,
                  Set(delivery.keys) == Set(["operationId", "ackRevision", "record"]),
                  nonempty(delivery["operationId"]), integer(delivery["ackRevision"], positive: true),
                  let record = delivery["record"] as? Object, validateRecord(record), record["messageId"] as? String == id,
                  request["operation"] as? String == "remove", request["requireUnheld"] as? Bool == true,
                  nonempty(request["expectedToken"]), integer(request["expectedRevision"], positive: true) else { return false }
        }
        if let revision = request["expectedRevision"], !integer(revision) { return false }
        if let token = request["expectedToken"], !nonempty(token) { return false }
        if let held = request["requireUnheld"], !boolean(held) { return false }
        return true
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
        guard Self.integer(value["sourceRevision"]), value["token"] is String,
              Self.boolean(value["confirmed"]), let outcomes = value["outcomes"] as? [String: Object], let removals = value["removals"] as? [String: Object] else {
            throw failure("The outbox transaction inventory is invalid.")
        }
        for (mutation, outcome) in outcomes {
            guard let result = outcome["result"] as? Object, result["mutationId"] as? String == mutation,
                  result["messageId"] as? String == id, Self.member(result["status"], ["committed", "stale", "failed"]),
                  Self.integer(result["revision"]), result["message"] is String,
                  let encoded = outcome["request"] as? String, let data = encoded.data(using: .utf8),
                  let request = try JSONSerialization.jsonObject(with: data) as? Object,
                  request["mutationId"] as? String == mutation, Self.validateMutation(request, id: id),
                  let owners = outcome["owners"] as? [Object], owners.allSatisfy({ Self.validateRecord($0) && $0["messageId"] as? String == id }) else {
                throw failure("The outbox outcome inventory is invalid.")
            }
            if let captured = request["transfer"] as? Object {
                guard let claim = value["transfer"] as? Object, Self.sameTransfer(captured, claim) else {
                    throw failure("The outbox outcome lost its captured draft owner.")
                }
            }
            for field in ["record", "removed"] {
                guard let raw = result[field], raw is NSNull || (raw as? Object).map({ Self.validateRecord($0) && $0["messageId"] as? String == id }) == true else {
                    throw failure("The outbox outcome payload is invalid.")
                }
            }
        }
        guard removals.values.allSatisfy({ Self.validateRecord($0) && $0["messageId"] as? String == id }) else { throw failure("The outbox removal inventory is invalid.") }
        if value["state"] as? String == "active", !Self.nonempty(value["token"]) { throw failure("The active outbox owner is invalid.") }
        if value["state"] as? String == "pending" {
            guard let mutation = value["mutation"] as? Object, Self.validateMutation(mutation, id: id) else { throw failure("The outbox pending mutation is invalid.") }
            if let captured = mutation["transfer"] as? Object {
                guard let claim = value["transfer"] as? Object, Self.jsonEqual(captured, claim) else {
                    throw failure("The prepared draft capture differs from its recorded mutation.")
                }
            }
            if value["previous"] is Object, !Self.nonempty(value["token"]) { throw failure("The previous outbox owner is invalid.") }
            if mutation["operation"] as? String == "remove" {
                guard value["proposed"] is NSNull else { throw failure("An outbox removal cannot propose a record.") }
            } else {
                guard let proposed = value["proposed"] as? Object, let requested = mutation["record"] as? Object,
                      try JSONSerialization.data(withJSONObject: proposed, options: [.sortedKeys])
                        == JSONSerialization.data(withJSONObject: requested, options: [.sortedKeys]) else {
                    throw failure("The outbox proposal differs from its recorded mutation.")
                }
            }
        }
        if let transfer = value["transfer"] {
            guard let claim = transfer as? Object, Self.validateTransfer(claim, id: id) else {
                throw failure("The captured draft transfer inventory is invalid.")
            }
        }
        return value
    }
    func load(_ id: String) throws -> Object? {
        let file = url(id)
        do { return try decode(file) }
        catch let error as CocoaError where error.code == .fileReadNoSuchFile { return nil }
    }
    func envelopes() -> (values: [Object], errors: [Object]) {
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
    func save(_ value: Object) throws {
        guard JSONSerialization.isValidJSONObject(value), let id = value["messageId"] as? String else { throw failure("The outbox record cannot be serialized.") }
        let data = try JSONSerialization.data(withJSONObject: value, options: [.sortedKeys])
        guard data.count <= T3Wire.maximumBytes else { throw failure("The outbox record is too large.") }
        try replace(data, url(id))
    }
    func payloads(_ value: Object) -> [Object] {
        ["record", "previous", "proposed"].compactMap { value[$0] as? Object }
            + (value["outcomes"] as? [String: Object] ?? [:]).values.flatMap { $0["owners"] as? [Object] ?? [] }
            + Array((value["removals"] as? [String: Object] ?? [:]).values)
            + [(value["transfer"] as? Object)?["record"] as? Object].compactMap { $0 }
    }

    static func jsonEqual(_ a: Any?, _ b: Any?) -> Bool {
        guard let a, let b else { return a == nil && b == nil }
        return (try? JSONSerialization.data(withJSONObject: [a], options: [.sortedKeys]))
            == (try? JSONSerialization.data(withJSONObject: [b], options: [.sortedKeys]))
    }
    static func captureFingerprint(_ capture: Object) throws -> String {
        SHA256.hash(data: try JSONSerialization.data(withJSONObject: capture, options: [.sortedKeys]))
            .map { String(format: "%02x", $0) }.joined()
    }
    private static func fields(_ value: Object, required: [String], optional: [String] = []) -> Bool {
        Set(required).isSubset(of: Set(value.keys)) && Set(value.keys).isSubset(of: Set(required + optional))
    }
    // Transfer-specific supported context admission. The ordinary outbox record
    // decoder remains unchanged; captured payloads require stronger provenance.
    private static func contextRecord(_ item: Object) -> Bool {
        func string(_ key: String, _ limit: Int, required: Bool = false) -> Bool {
            guard let value = item[key] as? String, value.utf16.count <= limit else { return false }
            return !required || nonempty(value)
        }
        func identifier(_ value: Any?) -> Bool {
            guard let value = value as? String, nonempty(value) else { return false }
            return matches(value, "^[a-zA-Z0-9_-]{1,128}$")
        }
        guard integer(item["version"], positive: true), item["version"] as? Int == 1,
              identifier(item["contextId"]), string("label", 200), let kind = item["kind"] as? String else { return false }
        switch kind {
        case "image", "file":
            return identifier(item["attachmentId"]) && string("name", 255, required: true)
                && string("mimeType", 100, required: true) && integer(item["sizeBytes"])
        case "thread":
            return nonempty(item["environmentId"]) && nonempty(item["threadId"]) && string("title", 200)
        case "terminal":
            return string("terminalId", 255, required: true) && string("terminalLabel", 255, required: true)
                && integer(item["lineStart"]) && integer(item["lineEnd"])
                && (item["lineEnd"] as! NSNumber).doubleValue >= (item["lineStart"] as! NSNumber).doubleValue && string("text", 64000)
        case "mention": return string("path", 2048, required: true)
        case "skill": return string("name", 255, required: true)
        case "review-comment":
            guard string("sectionId", 255, required: true), string("sectionTitle", 2048), string("filePath", 2048, required: true),
                  integer(item["startIndex"]), integer(item["endIndex"]),
                  (item["endIndex"] as! NSNumber).doubleValue >= (item["startIndex"] as! NSNumber).doubleValue,
                  string("rangeLabel", 2048), string("text", 16000), string("diff", 32000),
                  item["fenceLanguage"] == nil || string("fenceLanguage", 64) else { return false }
            if let raw = item["pullRequest"] {
                guard let pr = raw as? Object, integer(pr["number"], positive: true),
                      ["title", "url", "headBranch", "baseBranch"].allSatisfy({ (pr[$0] as? String).map { $0.utf16.count <= 2048 } == true }),
                      member(pr["state"], ["open", "closed", "merged"]), boolean(pr["isDraft"]) else { return false }
            }
            return true
        default: return false
        }
    }
    private static func contextRecords(_ raw: Any?, bounded: Bool = false) -> [Object]? {
        guard let raw else { return [] }
        guard let value = raw as? Object, integer(value["version"], positive: true), value["version"] as? Int == 1,
              let records = value["records"] as? [Object], !bounded || records.count <= 200,
              let data = try? JSONSerialization.data(withJSONObject: records, options: [.withoutEscapingSlashes]),
              let serialized = String(data: data, encoding: .utf8), !bounded || serialized.utf16.count <= 16_000_000 else { return nil }
        var seen = Set<String>()
        guard records.allSatisfy({ contextRecord($0) && seen.insert($0["contextId"] as! String).inserted }) else { return nil }
        return records
    }
    private static func contextReferences(_ text: String) -> [(kind: String, id: String)] {
        // The source regex counts UTF-16 units and uses ECMAScript whitespace.
        // ICU quantifiers count Unicode scalars, so scan its bounded grammar directly.
        let units = Array(text.utf16), prefix = Array("(t3-context://v1/".utf16)
        var seen = Set<String>(), result: [(kind: String, id: String)] = [], start = 0
        while start < units.count {
            guard units[start] == 91 else { start += 1; continue }
            var close = start + 1
            while close < units.count && close - start - 1 <= 512 && units[close] != 93 && units[close] != 10 { close += 1 }
            guard close < units.count, close - start - 1 <= 512, units[close] == 93,
                  close + 1 + prefix.count <= units.count,
                  Array(units[(close + 1)..<(close + 1 + prefix.count)]) == prefix else { start += 1; continue }
            let begin = close + 1 + prefix.count
            var end = begin
            while end < units.count && end - begin <= 200 && units[end] != 41 {
                if let scalar = UnicodeScalar(UInt32(units[end])), trimCharacters.contains(scalar) { break }
                end += 1
            }
            guard end < units.count, end > begin, end - begin <= 200, units[end] == 41 else { start += 1; continue }
            let parts = String(decoding: units[begin..<end], as: UTF16.self).split(separator: "/", omittingEmptySubsequences: false).map(String.init)
            start = end + 1
            guard parts.count == 2, matches(parts[0], "^[a-z][a-z0-9-]{0,39}$"), matches(parts[1], "^[a-zA-Z0-9_-]{1,128}$"),
                  seen.insert(parts.joined(separator: "/")).inserted else { continue }
            result.append((parts[0], parts[1]))
        }
        return result
    }
    private static func contextLabel(_ name: String, kind: String) -> String {
        let replaced = name.replacingOccurrences(of: #"[\[\]\\\r\n]"#, with: " ", options: .regularExpression)
        let words = replaced.components(separatedBy: trimCharacters).filter { !$0.isEmpty }
        // Match the source label normalization and its UTF-16 length bound.
        let joined = words.joined(separator: " ") as NSString
        let result = joined.substring(to: min(joined.length, 200))
        return result.isEmpty ? kind : result
    }
    private static func contextMatches(_ draft: Object, _ record: Object, _ originals: [Object], _ attachments: [Object]) -> Bool {
        let references = contextReferences(draft["text"] as! String)
        let selected = originals.filter { original in references.contains { $0.id == original["contextId"] as? String } }
        let submitted: [Object]
        if let raw = record["context"] {
            guard let context = raw as? Object, fields(context, required: ["version", "records"]),
                  integer(context["version"], positive: true), context["version"] as? Int == 1,
                  let records = context["records"] as? [Object] else { return false }
            submitted = records
        } else { submitted = [] }
        var expected: [Object] = []
        for original in selected {
            guard !references.contains(where: { $0.id == original["contextId"] as? String && $0.kind != original["kind"] as? String }),
                  expected.count < submitted.count else { return false }
            var normalized = original
            if member(original["kind"], ["image", "file"]) {
                let actual = submitted[expected.count]
                guard let attachment = attachments.first(where: { $0["id"] as? String == original["attachmentId"] as? String })
                    ?? attachments.first(where: { nonempty($0["uploadId"]) && $0["uploadId"] as? String == original["attachmentId"] as? String }),
                    attachment["kind"] as? String == original["kind"] as? String,
                    actual["attachmentId"] as? String == attachment["id"] as? String,
                    ["name", "mimeType", "sizeBytes"].allSatisfy({ jsonEqual(original[$0], attachment[$0]) }) else { return false }
                normalized["attachmentId"] = attachment["id"]
            }
            expected.append(normalized)
        }
        for reference in references {
            if selected.contains(where: { $0["contextId"] as? String == reference.id }) { continue }
            guard let attachment = attachments.first(where: {
                let id = $0["kind"] as? String == "file" ? $0["contextId"] as? String : "image_\($0["id"] as? String ?? "")"
                return $0["kind"] as? String == reference.kind && id == reference.id
            }) else { return false }
            expected.append(["version": 1, "contextId": reference.id, "kind": reference.kind,
                "label": contextLabel(attachment["name"] as! String, kind: reference.kind), "attachmentId": attachment["id"]!,
                "name": attachment["name"]!, "mimeType": attachment["mimeType"]!, "sizeBytes": attachment["sizeBytes"]!])
        }
        guard contextRecords(["version": 1, "records": expected], bounded: true) != nil else { return false }
        if draft["context"] != nil && originals.isEmpty || !expected.isEmpty {
            return record["context"] != nil && jsonEqual(submitted, expected)
        }
        return record["context"] == nil
    }
    static func validateCapture(_ capture: Object, record: Object? = nil) -> Bool {
        if capture["kind"] != nil { return validateOrdinaryCapture(capture, record: record) }
        guard fields(capture, required: ["version", "draft"]), integer(capture["version"], positive: true), capture["version"] as? Int == 1,
              let draft = capture["draft"] as? Object,
              fields(draft, required: ["key", "revision", "createdAt", "environmentId", "origin", "projectId", "text", "images", "files", "attachmentIds", "choices", "workspace"], optional: ["branchChoice", "context"]),
              let key = draft["key"] as? String, nonempty(key), matches(key, "^new-task:[a-zA-Z0-9_-]{1,128}$"),
              integer(draft["revision"]), canonicalOrigin(draft["origin"]), nonempty(draft["environmentId"]), nonempty(draft["projectId"]),
              draft["text"] is String, let images = draft["images"] as? [Object], let files = draft["files"] as? [Object],
              let order = draft["attachmentIds"] as? [String], JSONSerialization.isValidJSONObject(capture) else { return false }
        guard let context = contextRecords(draft["context"]) else { return false }
        if !(draft["choices"] is NSNull) {
            guard let choices = draft["choices"] as? Object,
                  fields(choices, required: [], optional: ["providerId", "modelId", "modelOptions", "runtimeMode", "interactionMode"]) else { return false }
            for name in ["runtimeMode", "interactionMode"] where choices[name] != nil { if !(choices[name] is String) { return false } }
            if ["providerId", "modelId", "modelOptions"].contains(where: { choices[$0] != nil }) {
                guard choices["providerId"] is String, choices["modelId"] is String, choices["modelOptions"] is [Object] else { return false }
            }
        }
        for name in ["workspace", "branchChoice"] {
            if name == "workspace" && draft[name] is NSNull || name == "branchChoice" && draft[name] == nil { continue }
            guard let workspace = draft[name] as? Object,
                  fields(workspace, required: name == "branchChoice" ? ["envMode", "branch", "worktreePath", "kind"] : ["envMode", "branch", "worktreePath"]),
                  member(workspace["envMode"], ["local", "worktree"]), ["branch", "worktreePath"].allSatisfy({ workspace[$0] is String }),
                  name != "branchChoice" || member(workspace["kind"], ["automatic", "explicit"]) else { return false }
        }
        var normalized: [String: Object] = [:]
        for (kind, attachments) in [("image", images), ("file", files)] {
            for attachment in attachments {
                guard let id = attachment["id"] as? String, normalized[id.lowercased()] == nil else { return false }
                let required = kind == "image" ? ["id", "name", "mimeType", "sizeBytes"]
                    : ["id", "contextId", "draftKey", "environmentId", "name", "mimeType", "sizeBytes", "source", "attachmentId", "status"]
                guard Set(required).isSubset(of: Set(attachment.keys)) else { return false }
                var item: Object = ["id": id, "kind": kind, "name": attachment["name"]!, "mimeType": attachment["mimeType"]!, "sizeBytes": attachment["sizeBytes"]!]
                let uploadField = kind == "image" ? "uploadId" : "attachmentId"
                guard attachment[uploadField] == nil || attachment[uploadField] is String else { return false }
                let upload = attachment[uploadField] as? String ?? ""
                item["uploadId"] = upload; item["status"] = upload.isEmpty ? "staged" : "ready"
                if !upload.isEmpty { item["uploadEnvironmentId"] = draft["environmentId"] }
                if kind == "file" {
                    guard attachment["draftKey"] as? String == key, attachment["environmentId"] as? String == draft["environmentId"] as? String,
                          attachment["source"] as? String == "attached", member(attachment["status"], ["staged", "ready"]) else { return false }
                    item["contextId"] = attachment["contextId"]
                }
                for field in ["source", "videoWidth", "videoHeight"] where attachment[field] != nil { item[field] = attachment[field] }
                normalized[id.lowercased()] = item
            }
        }
        guard order.count == normalized.count, Set(order.map { $0.lowercased() }).count == order.count,
              order.allSatisfy({ normalized[$0.lowercased()]?["id"] as? String == $0 }) else { return false }
        let attachments = order.compactMap { normalized[$0.lowercased()] }
        let probe: Object = ["schemaVersion": 1, "origin": draft["origin"]!, "environmentId": draft["environmentId"]!,
            "threadId": "capture", "messageId": "capture", "commandId": "capture", "text": draft["text"]!, "createdAt": draft["createdAt"]!, "attachments": attachments]
        guard validateRecord(probe) else { return false }
        if let record {
            guard validateRecord(record), record["origin"] as? String == draft["origin"] as? String,
                  record["environmentId"] as? String == draft["environmentId"] as? String,
                  (record["creation"] as? Object)?["projectId"] as? String == draft["projectId"] as? String,
                  record["text"] as? String == (draft["text"] as! String).trimmingCharacters(in: trimCharacters),
                  jsonEqual(record["attachments"], attachments), contextMatches(draft, record, context, attachments) else { return false }
        }
        return true
    }
    private static func sameTransfer(_ original: Object, _ current: Object) -> Bool {
        guard ["kind", "origin", "environmentId", "transferId", "draftKey", "fingerprint", "messageId", "threadId", "commandId", "mutationId"].allSatisfy({ jsonEqual(original[$0], current[$0]) }) else { return false }
        return ["completed", "released"].contains(current["state"] as? String ?? "")
            || jsonEqual(original["record"], current["record"]) && jsonEqual(original["capture"], current["capture"])
    }
    static func validateTransfer(_ claim: Object, id: String) -> Bool {
        if claim["kind"] != nil { return validateOrdinaryTransfer(claim, id: id) }
        guard fields(claim, required: ["transferId", "draftKey", "fingerprint", "messageId", "threadId", "commandId", "mutationId", "state", "record", "capture", "outcome"]),
              claim["transferId"] as? String == id, claim["messageId"] as? String == id,
              ["threadId", "commandId", "mutationId", "draftKey"].allSatisfy({ nonempty(claim[$0]) }),
              matches(claim["draftKey"] as! String, "^new-task:[a-zA-Z0-9_-]{1,128}$"),
              let digest = claim["fingerprint"] as? String, digest.count == 64, matches(digest, "^[a-f0-9]{64}$"),
              let state = claim["state"] as? String, ["prepared", "queued", "failed", "completed", "released"].contains(state) else { return false }
        if ["completed", "released"].contains(state) {
            guard claim["record"] is NSNull, claim["capture"] is NSNull else { return false }
        } else {
            guard let record = claim["record"] as? Object, let capture = claim["capture"] as? Object,
                  validateCapture(capture, record: record), (capture["draft"] as? Object)?["key"] as? String == claim["draftKey"] as? String,
                  (try? captureFingerprint(capture)) == digest,
                  ["messageId", "threadId", "commandId"].allSatisfy({ record[$0] as? String == claim[$0] as? String }) else { return false }
        }
        if state == "prepared" { return claim["outcome"] is NSNull }
        guard let outcome = claim["outcome"] as? Object, outcome["mutationId"] as? String == claim["mutationId"] as? String,
              outcome["messageId"] as? String == id, integer(outcome["revision"]), outcome["message"] is String,
              outcome["removed"] is NSNull, outcome["status"] as? String == (["failed", "released"].contains(state) ? "failed" : "committed") else { return false }
        return state == "queued" ? jsonEqual(outcome["record"], claim["record"]) : outcome["record"] is NSNull
    }

    // v2 ordinary capture deliberately does not use NewTask's reference projection.
    static func ordinaryTarget(_ value: Object) -> Bool {
        guard fields(value, required: ["origin", "environmentId", "threadId", "draftKey"]),
              canonicalOrigin(value["origin"]), ordinaryText(value["environmentId"]), ordinaryText(value["threadId"]),
              let thread = value["threadId"] as? String, !thread.hasPrefix("new:"),
              let key = value["draftKey"] as? String, !key.contains("~queued-edit~") else { return false }
        return key == "\(value["environmentId"] as! String):\(thread)"
    }
    static func ordinaryScope(_ value: Object, draft: Bool = false) -> Object {
        ["origin": value["origin"] ?? NSNull(), "environmentId": value["environmentId"] ?? NSNull(),
         "threadId": value["threadId"] ?? NSNull(), "draftKey": value[draft ? "key" : "draftKey"] ?? NSNull()]
    }
    private static func ordinaryText(_ raw: Any?, limit: Int = 4096) -> Bool {
        nonempty(raw) && (raw as! String).utf16.count <= limit
    }
    private static func ordinarySelection(_ raw: Any?, length: Int) -> Bool {
        if raw is NSNull { return true }
        guard let value = raw as? Object, fields(value, required: ["start", "end"]), integer(value["start"]), integer(value["end"]),
              let start = value["start"] as? Int, let end = value["end"] as? Int else { return false }
        return start <= end && end <= length
    }
    private static func ordinaryDocument(_ raw: Any?, text: String, persisted: Bool = false) -> Bool {
        guard let value = raw as? Object,
              fields(value, required: ["incarnation", "revision", "selection"] + (persisted ? ["origin", "environmentId", "threadId", "draftKey"] : [])),
              ordinaryText(value["incarnation"], limit: 128), integer(value["revision"]),
              ordinarySelection(value["selection"], length: text.utf16.count) else { return false }
        return !persisted || ordinaryTarget(ordinaryScope(value))
    }
    private static func ordinaryContext(_ raw: Any?, bounded: Bool) -> Bool {
        if raw is NSNull { return true }
        guard let value = raw as? Object, fields(value, required: ["version", "records"]), integer(value["version"]), value["version"] as? Int == 1,
              let records = value["records"] as? [Object], !bounded || records.count <= 200,
              let data = try? JSONSerialization.data(withJSONObject: records, options: [.withoutEscapingSlashes]),
              let string = String(data: data, encoding: .utf8), !bounded || string.utf16.count <= 16_000_000 else { return false }
        var seen = Set<String>()
        return records.allSatisfy { item in
            ordinaryContextRecord(item) && seen.insert(item["contextId"] as! String).inserted
        }
    }
    private static func ordinaryContextRecord(_ item: Object) -> Bool {
        if contextRecord(item) { return true }
        guard integer(item["version"]), item["version"] as? Int == 1,
              let id = item["contextId"] as? String, matches(id, "^[a-zA-Z0-9_-]{1,128}$"),
              let label = item["label"] as? String, label.utf16.count <= 200,
              let kind = item["kind"] as? String else { return false }
        func string(_ raw: Any?, _ limit: Int, nullable: Bool = false, trimmed: Bool = false) -> Bool {
            if nullable && raw is NSNull { return true }
            guard let value = raw as? String, value.utf16.count <= limit else { return false }
            return !trimmed || nonempty(value)
        }
        func element(_ value: Object) -> Bool {
            guard string(value["pageUrl"], 2048), string(value["pageTitle"], 2048, nullable: true),
                  string(value["tagName"], 255, trimmed: true), string(value["selector"], 2048, nullable: true),
                  string(value["htmlPreview"], 8000), string(value["componentName"], 2048, nullable: true), string(value["styles"], 8000) else { return false }
            if value["source"] is NSNull { return true }
            guard let source = value["source"] as? Object else { return false }
            return ["functionName", "fileName"].allSatisfy { string(source[$0], 2048, nullable: true) }
                && ["lineNumber", "columnNumber"].allSatisfy { source[$0] is NSNull || integer(source[$0]) }
        }
        if kind == "element" { return element(item) }
        if kind == "preview-annotation" {
            guard ["annotationId", "pageUrl", "targetSummary"].allSatisfy({ string(item[$0], 2048) }),
                  string(item["pageTitle"], 2048, nullable: true), string(item["comment"], 8000),
                  let changes = item["styleChanges"] as? [String], changes.count <= 200, changes.allSatisfy({ $0.utf16.count <= 2048 }) else { return false }
            if let raw = item["elements"] { guard let values = raw as? [Object], values.count <= 50, values.allSatisfy(element) else { return false } }
            if let raw = item["elementIds"] { guard let values = raw as? [String], values.count <= 50, values.allSatisfy({ $0.utf16.count <= 2048 }) else { return false } }
            for key in ["regionCount", "strokeCount"] where item[key] != nil { if !integer(item[key]) { return false } }
            if let raw = item["screenshotContextId"] { guard let id = raw as? String, matches(id, "^[a-zA-Z0-9_-]{1,128}$") else { return false } }
            if let raw = item["styleChangeDetails"] {
                guard let values = raw as? [Object], values.count <= 200, values.allSatisfy({ value in
                    string(value["targetId"], 2048) && string(value["selector"], 2048, nullable: true) && string(value["property"], 2048)
                        && string(value["previousValue"], 8000) && string(value["value"], 8000)
                }) else { return false }
            }
            return true
        }
        // Known producer kinds cannot escape their validation through the future-kind branch.
        guard !["image", "file", "thread", "terminal", "mention", "skill", "review-comment"].contains(kind),
              matches(kind, "^[a-z][a-z0-9-]{0,39}$"), let payload = item["payload"],
              let bytes = try? JSONSerialization.data(withJSONObject: payload, options: [.fragmentsAllowed, .withoutEscapingSlashes]),
              let string = String(data: bytes, encoding: .utf8) else { return false }
        return string.utf16.count <= 64000
    }
    /// Returns complete mixed normalized attachments. Preserved current inventory grants no upload authority.
    private static func ordinaryInventory(_ value: Object, target: Object, bounded: Bool) -> [Object]? {
        guard let images = value["images"] as? [Object], let files = value["files"] as? [Object],
              let order = value["attachmentIds"] as? [String], !bounded || images.count + files.count <= 100 else { return nil }
        let rawOrder: [String]
        if value["attachmentOrder"] is NSNull { rawOrder = [] }
        else { guard let raw = value["attachmentOrder"] as? [String], raw.allSatisfy({ ordinaryText($0) }), Set(raw).count == raw.count else { return nil }; rawOrder = raw }
        var normalized: [String: Object] = [:], insertion: [String] = [], seen = Set<String>(), contexts = Set<String>()
        for (kind, rows) in [("image", images), ("file", files)] {
            for row in rows {
                guard let id = row["id"] as? String, ordinaryText(id), !bounded || matches(id, "^[a-fA-F0-9]{8}-[a-fA-F0-9]{4}-[a-fA-F0-9]{4}-[a-fA-F0-9]{4}-[a-fA-F0-9]{12}$"),
                      seen.insert(id.lowercased()).inserted, ordinaryText(row["name"], limit: 255), ordinaryText(row["mimeType"], limit: 100), integer(row["sizeBytes"]) else { return nil }
                let upload: String
                if kind == "image" {
                    guard row["uploadId"] == nil || row["uploadId"] is String, row["source"] == nil || row["source"] is Object else { return nil }
                    upload = row["uploadId"] as? String ?? ""
                } else {
                    guard row["draftKey"] as? String == target["draftKey"] as? String, row["environmentId"] as? String == target["environmentId"] as? String,
                          ordinaryText(row["contextId"], limit: 128), contexts.insert(row["contextId"] as! String).inserted,
                          bounded ? row["source"] as? String == "attached" : ordinaryText(row["source"]),
                          member(row["status"], ["staged", "ready"]), let actual = row["attachmentId"] as? String,
                          row["status"] as? String != "ready" || !actual.isEmpty else { return nil }
                    upload = actual
                    for key in ["videoWidth", "videoHeight"] where row[key] != nil { if !positive(row[key]) { return nil } }
                }
                guard upload.isEmpty || matches(upload, "^[a-zA-Z0-9_-]{1,128}$") else { return nil }
                var next: Object = ["kind": kind, "id": id, "name": row["name"]!, "mimeType": row["mimeType"]!, "sizeBytes": row["sizeBytes"]!,
                    "uploadId": upload, "status": upload.isEmpty ? "staged" : "ready"]
                if !upload.isEmpty { next["uploadEnvironmentId"] = target["environmentId"] }
                if kind == "file" { next["contextId"] = row["contextId"]; next["source"] = row["source"]
                    for key in ["videoWidth", "videoHeight"] where row[key] != nil { next[key] = row[key] }
                } else if let source = row["source"] { next["source"] = source }
                normalized[id] = next; insertion.append(id)
            }
        }
        var ranked = Set<String>()
        let expected = (rawOrder + insertion).filter { normalized[$0] != nil && ranked.insert($0).inserted }
        guard order == expected, order.count == normalized.count else { return nil }
        return order.compactMap { normalized[$0] }
    }
    // Source365aa87982 shared/image.imageMimeType: semantic image recognition does
    // not change the original file storage kind or its native byte-read authority.
    private static func ordinaryImageMetadata(_ attachment: Object) -> Bool {
        guard let raw = attachment["mimeType"] as? String, let name = attachment["name"] as? String else { return false }
        let mime = String(raw.split(separator: ";", maxSplits: 1, omittingEmptySubsequences: false).first ?? "")
            .trimmingCharacters(in: trimCharacters).lowercased()
        if ["image/gif", "image/jpeg", "image/png", "image/webp"].contains(mime) { return true }
        if mime.hasPrefix("image/") || !["", "application/octet-stream", "binary/octet-stream", "application/unknown"].contains(mime) { return false }
        guard let dot = name.lastIndex(of: ".") else { return false }
        let suffix = String(name[name.index(after: dot)...]).trimmingCharacters(in: trimCharacters).lowercased()
        return ["gif", "jpeg", "jpg", "png", "webp"].contains(suffix)
    }
    static func validateOrdinaryCapture(_ capture: Object, record: Object? = nil) -> Bool {
        guard fields(capture, required: ["version", "kind", "draft"]), integer(capture["version"]), capture["version"] as? Int == 2,
              capture["kind"] as? String == "ordinary", let draft = capture["draft"] as? Object,
              fields(draft, required: ["key", "origin", "environmentId", "threadId", "document", "text", "context", "contextRevision", "images", "files", "attachmentIds", "attachmentOrder"]),
              ordinaryTarget(ordinaryScope(draft, draft: true)), let text = draft["text"] as? String, text.utf16.count <= 1_000_000,
              ordinaryDocument(draft["document"], text: text), integer(draft["contextRevision"]), ordinaryContext(draft["context"], bounded: true),
              JSONSerialization.isValidJSONObject(capture), let attachments = ordinaryInventory(draft, target: ordinaryScope(draft, draft: true), bounded: true) else { return false }
        for context in (draft["context"] as? Object)?["records"] as? [Object] ?? [] where context["attachmentId"] != nil {
            guard let attachment = attachments.first(where: { jsonEqual($0["id"], context["attachmentId"]) }),
                  jsonEqual(attachment["kind"], context["kind"]) || context["kind"] as? String == "image"
                    && attachment["kind"] as? String == "file" && ordinaryImageMetadata(attachment),
                  ["name", "mimeType", "sizeBytes"].allSatisfy({ jsonEqual(attachment[$0], context[$0]) }) else { return false }
        }
        guard let record else { return true }
        return fields(record, required: ["schemaVersion", "origin", "environmentId", "threadId", "messageId", "commandId", "text", "attachments", "modelSelection", "runtimeMode", "interactionMode", "createdAt"], optional: ["context", "dispatchMode"])
            && validateRecord(record) && ["origin", "environmentId", "threadId"].allSatisfy({ jsonEqual(record[$0], draft[$0]) })
            && record["text"] as? String == text.trimmingCharacters(in: trimCharacters) && jsonEqual(record["context"] ?? NSNull(), draft["context"])
            && jsonEqual(record["attachments"], attachments)
    }
    static func validateOrdinaryTransfer(_ claim: Object, id: String) -> Bool {
        guard fields(claim, required: ["kind", "origin", "environmentId", "transferId", "draftKey", "fingerprint", "messageId", "threadId", "commandId", "mutationId", "state", "record", "capture", "outcome"]),
              claim["kind"] as? String == "ordinary", ordinaryTarget(ordinaryScope(claim)), claim["transferId"] as? String == id, claim["messageId"] as? String == id,
              ["transferId", "commandId", "mutationId"].allSatisfy({ ordinaryText(claim[$0]) }),
              let digest = claim["fingerprint"] as? String, matches(digest, "^[a-f0-9]{64}$"),
              let state = claim["state"] as? String, ["prepared", "queued", "failed", "completed", "released"].contains(state) else { return false }
        if ["completed", "released"].contains(state) {
            guard claim["record"] is NSNull, claim["capture"] is NSNull else { return false }
        } else {
            guard let record = claim["record"] as? Object, let capture = claim["capture"] as? Object, validateOrdinaryCapture(capture, record: record),
                  jsonEqual(ordinaryScope(capture["draft"] as! Object, draft: true), ordinaryScope(claim)),
                  (try? captureFingerprint(capture)) == digest,
                  ["messageId", "threadId", "commandId"].allSatisfy({ jsonEqual(record[$0], claim[$0]) }) else { return false }
        }
        if state == "prepared" { return claim["outcome"] is NSNull }
        guard let outcome = claim["outcome"] as? Object, jsonEqual(outcome["mutationId"], claim["mutationId"]), outcome["messageId"] as? String == id,
              integer(outcome["revision"]), outcome["message"] is String, outcome["removed"] is NSNull,
              outcome["status"] as? String == (["failed", "released"].contains(state) ? "failed" : "committed") else { return false }
        return state == "queued" ? jsonEqual(outcome["record"], claim["record"]) : outcome["record"] is NSNull
    }
    static func validateOrdinaryCompletion(_ marker: Object, claim: Object) -> Bool {
        guard claim["state"] as? String == "queued", let capture = claim["capture"] as? Object, let draft = capture["draft"] as? Object,
              let before = draft["document"] as? Object,
              fields(marker, required: ["version", "kind", "draftKey", "fingerprint", "disposition", "before", "after"]),
              integer(marker["version"]), marker["version"] as? Int == 2, marker["kind"] as? String == "ordinary",
              jsonEqual(marker["draftKey"], claim["draftKey"]), jsonEqual(marker["fingerprint"], claim["fingerprint"]),
              member(marker["disposition"], ["cleared", "preserved"]),
              jsonEqual(marker["before"], ["incarnation": before["incarnation"]!, "revision": before["revision"]!]),
              let after = marker["after"] as? Object,
              fields(after, required: ["document", "text", "context", "images", "files", "attachmentIds", "attachmentOrder"]),
              let text = after["text"] as? String, text.utf16.count <= 1_000_000, ordinaryDocument(after["document"], text: text, persisted: true),
              let document = after["document"] as? Object, jsonEqual(ordinaryScope(document), ordinaryScope(claim)),
              ordinaryInventory(after, target: ordinaryScope(claim), bounded: false) != nil,
              let ids = after["attachmentIds"] as? [String], jsonEqual(after["attachmentOrder"], ids.isEmpty ? NSNull() : ids as Any) else { return false }
        if !(after["context"] is NSNull) {
            guard let context = after["context"] as? Object, fields(context, required: ["origin", "environmentId", "key", "revision", "text", "context"]),
                  ["origin", "environmentId"].allSatisfy({ jsonEqual(context[$0], claim[$0]) }), jsonEqual(context["key"], claim["draftKey"]),
                  integer(context["revision"]), context["text"] as? String == text, !(context["context"] is NSNull), ordinaryContext(context["context"], bounded: false) else { return false }
        }
        let same = jsonEqual(document["incarnation"], before["incarnation"]), revision = document["revision"] as! Int, original = before["revision"] as! Int
        if marker["disposition"] as? String == "cleared" {
            return same && original < 9_007_199_254_740_991 && revision == original + 1 && text.isEmpty && after["context"] is NSNull
                && (after["images"] as! [Object]).isEmpty && (after["files"] as! [Object]).isEmpty && ids.isEmpty
                && jsonEqual(document["selection"], ["start": 0, "end": 0])
        }
        return !same || revision > original || revision == original && text == draft["text"] as? String
    }
    /// Called under the existing coordinator mutex. Rewrites the SAME validated bytes to establish durability.
    func ordinaryTransferEvidence(_ claim: Object) throws {
        let path = directory.deletingLastPathComponent().appendingPathComponent("t3-code.json")
        let properties = try path.resourceValues(forKeys: [.isRegularFileKey, .isSymbolicLinkKey, .fileSizeKey])
        guard properties.isRegularFile == true, properties.isSymbolicLink != true, (properties.fileSize ?? Int.max) <= T3Wire.maximumBytes else { throw failure("The saved ordinary draft is unavailable.") }
        let data = try Data(contentsOf: path)
        guard data.count <= T3Wire.maximumBytes, let preferences = try JSONSerialization.jsonObject(with: data) as? Object,
              let markers = preferences["mobileOutboxTransferCompletions"] as? [String: Object], let marker = markers[claim["transferId"] as! String],
              Self.validateOrdinaryCompletion(marker, claim: claim), let after = marker["after"] as? Object,
              let key = claim["draftKey"] as? String, let origin = claim["origin"] as? String, let environment = claim["environmentId"] as? String else { throw failure("Save the exact ordinary draft completion before releasing it.") }
        let identity = String(decoding: try JSONSerialization.data(withJSONObject: [origin, environment, key], options: [.withoutEscapingSlashes]), as: UTF8.self)
        guard let documents = preferences["mobileComposerEditor"] as? Object, Self.integer(documents["version"]), documents["version"] as? Int == 1,
              let entries = documents["documents"] as? [String: Object], Self.jsonEqual(entries[identity], after["document"]),
              let drafts = preferences["drafts"] as? [String: String], (drafts[key] ?? "") == after["text"] as? String,
              let images = preferences["snapshotDrafts"] as? [String: [Object]],
              let files = (preferences["composerFiles"] ?? [Object]()) as? [Object],
              let orders = (preferences["mobileAttachmentOrder"] ?? Object()) as? [String: [String]] else { throw failure("The saved ordinary draft projection changed.") }
        // Validate complete mixed sibling inventories; a duplicate cannot hide in another row/type.
        var groupedFiles: [String: [Object]] = [:]
        for row in files {
            guard Self.ordinaryText(row["draftKey"]), Self.ordinaryText(row["environmentId"]) else { throw failure("The saved file inventory is invalid.") }
            groupedFiles[row["draftKey"] as! String, default: []].append(row)
        }
        for draftKey in Set(images.keys).union(groupedFiles.keys) {
            guard Self.ordinaryText(draftKey) else { throw failure("The saved draft inventory is invalid.") }
            let imageRows = images[draftKey] ?? [], fileRows = groupedFiles[draftKey] ?? []
            let environment = fileRows.first?["environmentId"] as? String ?? "unused"
            let probe: Object = ["images": imageRows, "files": fileRows,
                "attachmentIds": (imageRows + fileRows).compactMap { $0["id"] as? String }, "attachmentOrder": NSNull()]
            guard Self.ordinaryInventory(probe, target: ["draftKey": draftKey, "environmentId": environment], bounded: false) != nil else { throw failure("The saved mixed attachment inventory is invalid.") }
        }
        guard orders.values.allSatisfy({ values in values.allSatisfy { Self.ordinaryText($0) } && Set(values).count == values.count }),
              Self.jsonEqual(images[key] ?? [], after["images"]), Self.jsonEqual(files.filter { $0["draftKey"] as? String == key }, after["files"]),
              Self.jsonEqual(orders[key].map { $0 as Any } ?? NSNull(), after["attachmentOrder"]) else { throw failure("The saved attachment projection changed.") }
        let contexts: Object
        if let raw = preferences["mobileComposerContexts"] {
            guard let registry = raw as? Object, Self.integer(registry["version"]), registry["version"] as? Int == 1, let entries = registry["entries"] as? Object else { throw failure("The saved context inventory is invalid.") }
            contexts = entries
        } else { contexts = [:] }
        guard Self.jsonEqual(contexts[identity] ?? NSNull(), after["context"]) else { throw failure("The saved ordinary context changed.") }
        try replace(data, path)
    }

    func inventoryHolds(_ attachmentID: String) throws -> Bool {
        let inventory = envelopes()
        guard inventory.errors.isEmpty else { throw failure("Outbox attachment ownership is incomplete. Keep local files until its records can be read.") }
        return inventory.values.flatMap(payloads).contains { record in
            (record["attachments"] as? [Object] ?? []).contains { ($0["id"] as? String)?.lowercased() == attachmentID.lowercased() }
        }
    }
}
#endif
