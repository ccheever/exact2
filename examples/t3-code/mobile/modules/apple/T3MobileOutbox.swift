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
        guard ["transferId", "draftKey", "fingerprint", "messageId", "threadId", "commandId", "mutationId"].allSatisfy({ jsonEqual(original[$0], current[$0]) }) else { return false }
        return ["completed", "released"].contains(current["state"] as? String ?? "")
            || jsonEqual(original["record"], current["record"]) && jsonEqual(original["capture"], current["capture"])
    }
    static func validateTransfer(_ claim: Object, id: String) -> Bool {
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

    func inventoryHolds(_ attachmentID: String) throws -> Bool {
        let inventory = envelopes()
        guard inventory.errors.isEmpty else { throw failure("Outbox attachment ownership is incomplete. Keep local files until its records can be read.") }
        return inventory.values.flatMap(payloads).contains { record in
            (record["attachments"] as? [Object] ?? []).contains { ($0["id"] as? String)?.lowercased() == attachmentID.lowercased() }
        }
    }
}
#endif
