// App-local Effect RPC wire and retained inbox. No Exact transport is extended.
import Foundation

struct T3Failure: Error {
    let kind: String
    let message: String
    var uncertain = false
    var json: [String: Any] { ["kind": kind, "message": message, "uncertain": uncertain] }
}

enum T3Wire {
    static let maximumBytes = 16 * 1024 * 1024

    static func encode(_ message: [String: Any]) throws -> String {
        let data = try JSONSerialization.data(withJSONObject: message, options: [.fragmentsAllowed])
        guard data.count <= maximumBytes else { throw T3Failure(kind: "Limit", message: "The request is too large.") }
        return String(decoding: data, as: UTF8.self)
    }

    static func decode(_ data: Data) throws -> [[String: Any]] {
        guard data.count <= maximumBytes else { throw T3Failure(kind: "Limit", message: "The server frame is too large.") }
        let object = try JSONSerialization.jsonObject(with: data)
        let frames: [[String: Any]]
        if let array = object as? [[String: Any]] { frames = array }
        else if let one = object as? [String: Any] { frames = [one] }
        else { throw T3Failure(kind: "Protocol", message: "The server sent an invalid RPC frame.") }
        guard frames.allSatisfy({ $0["_tag"] is String }) else {
            throw T3Failure(kind: "Protocol", message: "The server sent an untagged RPC frame.")
        }
        return frames
    }

    static func identifier(_ value: Any?) -> String? {
        if let value = value as? String { return value }
        if let value = value as? NSNumber { return value.stringValue }
        return nil
    }

    static func request(id: String, method: String, payload: Any) -> [String: Any] {
        ["_tag": "Request", "id": id, "tag": method, "payload": payload, "headers": [["x-t3-orchestration-protocol", "2"]]]
    }

    static func failure(_ value: Any) -> T3Failure {
        if let object = value as? [String: Any] {
            if let cause = object["cause"] { return failure(cause) }
            if let error = object["error"] { return failure(error) }
            if let defect = object["defect"] { return failure(defect) }
            let tag = object["_tag"] as? String ?? "RPC"
            let text = object["message"] as? String ?? (tag == "Interrupt" ? "The server interrupted the request." : "The server refused the request (\(tag)).")
            return T3Failure(kind: tag, message: text)
        }
        if let array = value as? [Any], let first = array.first { return failure(first) }
        return T3Failure(kind: "RPC", message: (value as? String) ?? "The server request failed.")
    }
}

/// Chunk ACKs mean retained here; application ACKs mean reduced by TypeScript.
/// Reading never removes events, so superseded Exact answers cannot lose data.
struct T3Inbox {
    private(set) var latest = 0
    private(set) var floor = 0
    private(set) var bytes = 0
    private(set) var entries: [(seq: Int, bytes: Int, json: [String: Any])] = []
    private var notificationPending = false
    let limit: Int
    let byteLimit: Int
    init(limit: Int = 4096, byteLimit: Int = 16 * 1024 * 1024) {
        self.limit = limit; self.byteLimit = byteLimit
    }
    @discardableResult
    mutating func append(generation: Int, key: String, subscriptionId: String = "", value: Any) throws -> Bool {
        let size = try JSONSerialization.data(withJSONObject: value, options: [.fragmentsAllowed]).count
        let notify = !notificationPending
        notificationPending = true
        latest += 1
        if entries.count >= limit || bytes + size > byteLimit {
            // Explicit reset: the client must request authoritative snapshots.
            entries.removeAll(keepingCapacity: true); bytes = 0; floor = latest
        }
        guard size <= byteLimit else { return notify }
        entries.append((latest, size, ["seq": latest, "generation": generation, "key": key, "subscriptionId": subscriptionId, "value": value]))
        bytes += size
        return notify
    }
    func read(after: Int) -> [String: Any] {
        var page: [[String: Any]] = [], used = 0
        for entry in entries where entry.seq > after {
            // A single large value goes through the bridge's chunk transfer;
            // ordinary pages stay comfortably below native.later's 1 MiB cap.
            if !page.isEmpty && used + entry.bytes + 256 > 512 * 1024 { break }
            page.append(entry.json); used += entry.bytes + 256
        }
        return ["events": page, "latest": latest, "reset": after < floor || after > latest]
    }
    mutating func acknowledge(through: Int) {
        let accepted = min(max(through, 0), latest)
        let removed = entries.prefix { $0.seq <= accepted }
        bytes -= removed.reduce(0) { $0 + $1.bytes }
        entries.removeFirst(removed.count)
        floor = max(floor, accepted)
        if accepted == latest { notificationPending = false }
    }
    mutating func reset() { entries.removeAll(keepingCapacity: true); bytes = 0; floor = latest; notificationPending = false }
}

/// JSON values can exceed Exact's native.later reply limit (a whole turn or
/// diff). Replayable text pieces keep that concern entirely inside this app.
struct T3Transfers {
    private struct Payload { let parts: [String]; let bytes: Int; let created: Date }
    private var stored: [String: Payload] = [:]
    private(set) var bytes = 0
    let threshold: Int
    let limit: Int
    init(threshold: Int = 512 * 1024, limit: Int = 32 * 1024 * 1024) { self.threshold = threshold; self.limit = limit }
    mutating func prepare(_ value: Any) throws -> Any {
        let data = try JSONSerialization.data(withJSONObject: value, options: [.fragmentsAllowed])
        guard data.count > threshold else { return value }
        expire()
        guard data.count <= T3Wire.maximumBytes, bytes + data.count <= limit else {
            throw T3Failure(kind: "Limit", message: "Too much server data is waiting to be read. Retry after other reads finish.")
        }
        var parts: [String] = [], part = "", count = 0
        for scalar in String(decoding: data, as: UTF8.self).unicodeScalars {
            part.unicodeScalars.append(scalar); count += 1
            if count == 48 * 1024 { parts.append(part); part = ""; count = 0 }
        }
        if !part.isEmpty { parts.append(part) }
        let id = UUID().uuidString
        stored[id] = Payload(parts: parts, bytes: data.count, created: Date()); bytes += data.count
        return ["_nativeTransfer": ["id": id, "parts": parts.count]]
    }
    func read(id: String, index: Int) throws -> [String: Any] {
        guard let payload = stored[id], payload.parts.indices.contains(index) else {
            throw T3Failure(kind: "stale", message: "The pending server response was released. Retry the read.")
        }
        return ["text": payload.parts[index]]
    }
    mutating func release(_ id: String) { if let payload = stored.removeValue(forKey: id) { bytes -= payload.bytes } }
    mutating func expire() {
        for (id, payload) in stored where Date().timeIntervalSince(payload.created) > 120 { release(id) }
    }
    mutating func reset() { stored.removeAll(); bytes = 0 }
}

enum T3Endpoint {
    static func origin(_ input: String) throws -> URL {
        guard var url = URLComponents(string: input.trimmingCharacters(in: .whitespacesAndNewlines)),
              let scheme = url.scheme?.lowercased(), ["https", "http"].contains(scheme),
              let host = url.host, !host.isEmpty, url.user == nil, url.password == nil else {
            throw T3Failure(kind: "Address", message: "Enter an http://localhost or https:// server address.")
        }
        // Pairing secrets must not travel over unencrypted remote HTTP.
        guard scheme == "https" || ["localhost", "127.0.0.1", "::1", "[::1]"].contains(host.lowercased()) else {
            throw T3Failure(kind: "Address", message: "A remote server requires HTTPS.")
        }
        url.scheme = scheme; url.host = host.lowercased(); url.path = ""; url.query = nil; url.fragment = nil
        guard let result = url.url else { throw T3Failure(kind: "Address", message: "The server address is invalid.") }
        return result
    }
    static func path(_ path: String, at origin: URL) throws -> URL {
        guard path.hasPrefix("/"), !path.hasPrefix("//"), let url = URL(string: path, relativeTo: origin)?.absoluteURL,
              try self.origin(url.absoluteString) == origin, url.fragment == nil else {
            throw T3Failure(kind: "Address", message: "The request must stay on the connected server.")
        }
        return url
    }
    static func credential(_ input: String, at origin: URL) throws -> String {
        let text = input.trimmingCharacters(in: .whitespacesAndNewlines)
        guard text.contains("://") else { return text }
        guard let parts = URLComponents(string: text), try self.origin(text) == origin else {
            throw T3Failure(kind: "Credential", message: "Use a pairing link from this server, or paste its pairing token.")
        }
        let fragment = parts.percentEncodedFragment.flatMap { URLComponents(string: "https://pair.invalid/?" + $0)?.queryItems }
        guard let token = (fragment ?? []).first(where: { $0.name == "token" })?.value
            ?? parts.queryItems?.first(where: { $0.name == "token" })?.value, !token.isEmpty else {
            throw T3Failure(kind: "Credential", message: "The pairing link has no token.")
        }
        return token
    }
    static func form(_ values: [String: String]) -> Data {
        var parts = URLComponents()
        parts.queryItems = values.keys.sorted().map { URLQueryItem(name: $0, value: values[$0]) }
        // application/x-www-form-urlencoded treats literal '+' as a space.
        return Data((parts.percentEncodedQuery ?? "").replacingOccurrences(of: "+", with: "%2B").utf8)
    }
}
