import Foundation
import SQLite3

@main struct ClientCacheTransportTests {
    typealias Object = [String: Any]
    static let pieceSize = 48 * 1_024
    static let megabyte = 1_024 * 1_024

    static func main() throws {
        if CommandLine.arguments.count == 3, CommandLine.arguments[1] == "--bridge" {
            let store = T3MobileClientCache(directory: URL(fileURLWithPath: CommandLine.arguments[2]))
            let owner = T3MobileCacheTransport(store: store)
            defer { owner.close() }
            while let line = readLine() {
                let reply: Object
                do {
                    guard let value = try JSONSerialization.jsonObject(with: Data(line.utf8)) as? Object else {
                        throw CocoaError(.coderInvalidValue)
                    }
                    reply = ["ok": true, "generation": 0, "value": try owner.request(value)]
                } catch {
                    reply = ["ok": false, "generation": 0, "error": ["kind": "Cache", "message": error.localizedDescription]]
                }
                let encoded = try JSONSerialization.data(withJSONObject: reply, options: [.withoutEscapingSlashes])
                FileHandle.standardOutput.write(encoded + Data([10]))
            }
            return
        }

        let root = FileManager.default.temporaryDirectory.appendingPathComponent("t3-cache-transport-" + UUID().uuidString)
        try FileManager.default.createDirectory(at: root, withIntermediateDirectories: true)
        defer { try? FileManager.default.removeItem(at: root) }
        var checks = 0, failures = [String]()
        func check(_ condition: Bool, _ message: String) {
            checks += 1
            if !condition { failures.append(message) }
        }
        func refuse(_ message: String, _ run: () throws -> Void) {
            do { try run(); check(false, message) } catch { check(true, message) }
        }
        func test(_ name: String, _ run: () throws -> Void) {
            do { try run() } catch { check(false, "\(name): \(error)") }
        }
        func identity(_ environment: String = "one", _ kind: String = "thread", _ key: String = "t") -> Object {
            ["environmentId": environment, "kind": kind, "key": key]
        }
        func payload(_ bytes: Int, environment: String = "one", prefix: String = "") -> String {
            let start = "{\"environmentId\":\"\(environment)\",\"text\":\"" + prefix
            return start + String(repeating: "x", count: bytes - start.utf8.count - 2) + "\"}"
        }
        func exact(_ left: String?, _ right: String) -> Bool { left?.utf8.elementsEqual(right.utf8) == true }
        let path = root.appendingPathComponent("cache")
        var store = T3MobileClientCache(directory: path)
        var owner = T3MobileCacheTransport(store: store)
        func call(_ action: String, _ fields: Object = [:], using other: T3MobileCacheTransport? = nil) throws -> Object {
            let value = fields.merging(["action": action]) { _, new in new }
            let encoded = try JSONSerialization.data(withJSONObject: value)
            let parsed = try JSONSerialization.jsonObject(with: encoded) as! Object
            let result = try (other ?? owner).request(parsed)
            check(try JSONSerialization.data(withJSONObject: result).count <= 384 * 1_024, "reply respects encoded frame ceiling")
            return result
        }
        func ticket(_ identity: Object) throws -> String { try call("ticket", identity)["ticket"] as! String }
        func target(_ identity: Object, token: String? = nil) throws -> Object {
            identity.merging(["action": "write", "schemaVersion": 1, "ticket": try token ?? ticket(identity)]) { _, new in new }
        }
        func begin(_ target: Object, units: Int, using other: T3MobileCacheTransport? = nil) throws -> String {
            let result = try call("textBegin", ["target": target, "units": units], using: other)
            guard let transfer = result["transfer"] as? Object, let id = transfer["id"] as? String else {
                throw CocoaError(.coderInvalidValue)
            }
            return id
        }
        func pieces(_ text: String) -> [String] {
            let bytes = Data(text.utf8)
            var start = 0, result = [String]()
            while start < bytes.count {
                var end = min(start + pieceSize, bytes.count)
                while end < bytes.count, bytes[end] & 0xc0 == 0x80 { end -= 1 }
                result.append(String(data: bytes[start..<end], encoding: .utf8)!)
                start = end
            }
            return result
        }
        func send(_ id: String, _ text: String) throws {
            for (index, piece) in pieces(text).enumerated() {
                check(try call("textAppend", ["id": id, "index": index, "text": piece])["next"] as? Int == index + 1,
                      "append acknowledges its own index")
            }
        }
        func release(_ id: String) throws { _ = try call("textRelease", ["id": id]) }
        func upload(_ target: Object, _ text: String) throws -> Object {
            let id = try begin(target, units: text.utf16.count)
            defer { try? release(id) }
            try send(id, text)
            return try call("textFinish", ["id": id, "bytes": text.utf8.count])
        }
        func write(_ identity: Object, _ text: String) throws -> Object { try upload(target(identity), text) }
        func transfer(_ result: Object) throws -> Object {
            guard let transfer = result["transfer"] as? Object else { throw CocoaError(.coderInvalidValue) }
            return transfer
        }
        func download(_ manifest: Object, finish: Bool = true) throws -> String {
            let id = manifest["id"] as! String, count = manifest["parts"] as! Int
            var bytes = Data()
            for index in 0..<count {
                let part = try call("textPart", ["id": id, "index": index])
                check(part["index"] as? Int == index, "part echoes exact index")
                let text = part["text"] as! String
                check(text.utf8.count <= pieceSize, "download pieces obey UTF8 ceiling")
                bytes.append(contentsOf: text.utf8)
            }
            let text = String(data: bytes, encoding: .utf8)!
            check(bytes.count == manifest["bytes"] as? Int, "download exact byte count")
            check(text.utf16.count == manifest["units"] as? Int, "download exact UTF16 count")
            if finish { check(try call("textReadFinish", ["id": id])["complete"] as? Bool == true, "read ownership checked on completion") }
            return text
        }
        func read(_ identity: Object) throws -> Object? {
            let reply = try call("read", identity)
            guard let manifest = reply["transfer"] as? Object else { return reply["record"] as? Object }
            defer { try? release(manifest["id"] as! String) }
            var result = manifest["metadata"] as! Object
            result["payload"] = try download(manifest)
            return result
        }
        func tamper(_ sql: String) throws {
            var database: OpaquePointer?
            guard sqlite3_open(path.appendingPathComponent("cache.sqlite3").path, &database) == SQLITE_OK else {
                throw CocoaError(.fileReadUnknown)
            }
            defer { sqlite3_close(database) }
            guard sqlite3_exec(database, sql, nil, nil, nil) == SQLITE_OK else { throw CocoaError(.fileWriteUnknown) }
        }

        test("inline and exact text") {
            check(try call("read", identity())["record"] is NSNull, "absent record remains inline")
            let text = #"{ "environmentId" : "one", "escaped":"\u0061\u0000\/", "number":1.00, "unicode":"éé😀" }"#
            let fields = try target(identity()).merging(["payload": text]) { _, new in new }
            check(try call("write", fields)["written"] as? Bool == true, "small original write works")
            let before = try read(identity())!
            check(exact(before["payload"] as? String, text), "literal escapes, whitespace and normalization roundtrip")
            check(try write(identity(), text)["changed"] as? Bool == false, "same bytes deduplicate through upload")
            check(try read(identity())?["updatedAt"] as? Int64 == before["updatedAt"] as? Int64, "duplicate preserves store timestamp")
        }
        test("4 MiB and 16 MiB store limits") {
            for (kind, key, count) in [("server-config", "config", 4 * megabyte), ("thread", "large", 16 * megabyte)] {
                let id = identity("one", kind, key)
                let value = payload(count, prefix: #"\u0061\n\t\"\\"# + "e\u{301}é😀")
                check(try write(id, value)["written"] as? Bool == true, "full \(count)-byte payload writes")
                check(exact(try read(id)?["payload"] as? String, value), "full \(count)-byte payload reads exactly")
                owner.close()
                store = T3MobileClientCache(directory: path)
                owner = T3MobileCacheTransport(store: store)
                check(exact(try read(id)?["payload"] as? String, value), "full \(count)-byte payload survives restart")
                let remove = id.merging(["action": "remove"]) { _, new in new }
                check(try upload(remove, payload(count, prefix: "different"))["removed"] as? Int == 0,
                      "different conditional text preserves record")
                check(try upload(remove, value)["removed"] as? Int == 1, "full \(count)-byte CAS removes exact text")
                refuse("one byte over \(count)-byte store limit") { _ = try write(id, payload(count + 1)) }
                refuse("UTF8 byte limit applies independently of UTF16 units") { _ = try write(id, payload(count + 1, prefix: "😀")) }
            }
        }
        test("emoji at chunk boundary") {
            let prefix = "{\"environmentId\":\"one\",\"text\":\""
            let text = prefix + String(repeating: "x", count: pieceSize - prefix.utf8.count - 1) + "😀e\u{301}" + String(repeating: "x", count: pieceSize) + "\"}"
            _ = try write(identity(), text)
            check(exact(try read(identity())?["payload"] as? String, text), "UTF8 boundary never splits emoji or normalizes combining marks")
        }
        test("replay, gaps and conflicts") {
            let text = payload(100), writeTarget = try target(identity())
            let id = try begin(writeTarget, units: text.utf16.count)
            try send(id, text)
            check(try call("textAppend", ["id": id, "index": 0, "text": text])["next"] as? Int == 1, "exact duplicate append accepted")
            let first = try call("textFinish", ["id": id, "bytes": text.utf8.count])
            let timestamp = try read(identity())?["updatedAt"] as? Int64
            let second = try call("textFinish", ["id": id, "bytes": text.utf8.count])
            check(NSDictionary(dictionary: first).isEqual(to: second), "finish replay returns original result")
            check(try read(identity())?["updatedAt"] as? Int64 == timestamp, "finish replay never rewrites SQL")
            _ = try call("clear", identity())
            check(try call("textFinish", ["id": id, "bytes": text.utf8.count])["written"] as? Bool == true, "completed result survives clear without SQL replay")
            check(try read(identity()) == nil, "finish replay cannot resurrect cleared row")
            refuse("conflicting finish retires handle") { _ = try call("textFinish", ["id": id, "bytes": text.utf8.count - 1]) }
            refuse("retired finish unavailable") { _ = try call("textFinish", ["id": id, "bytes": text.utf8.count]) }
            for mode in ["gap", "conflict", "canonical", "partial", "bytes", "wrong-phase"] {
                let handle = try begin(target(identity()), units: 100)
                if mode == "canonical" { _ = try call("textAppend", ["id": handle, "index": 0, "text": "é"]) }
                else if mode == "conflict" { _ = try call("textAppend", ["id": handle, "index": 0, "text": "x"]) }
                refuse("\(mode) refuses") {
                    switch mode {
                    case "gap": _ = try call("textAppend", ["id": handle, "index": 1, "text": "x"])
                    case "conflict": _ = try call("textAppend", ["id": handle, "index": 0, "text": "y"])
                    case "canonical": _ = try call("textAppend", ["id": handle, "index": 0, "text": "e\u{301}"])
                    case "wrong-phase": _ = try call("textPart", ["id": handle, "index": 0])
                    default: _ = try call("textFinish", ["id": handle, "bytes": mode == "bytes" ? 1 : 0])
                    }
                }
                refuse("\(mode) retires handle") { _ = try call("textFinish", ["id": handle, "bytes": 0]) }
            }
            for action in ["textAppend", "textFinish", "textPart", "textReadFinish"] {
                refuse("unknown \(action) has no store effect") { _ = try call(action, ["id": "unknown", "index": 0, "text": "x", "bytes": 0]) }
            }
            _ = try call("textRelease", ["id": "unknown"])
            _ = try call("textRelease", ["id": "unknown"])
        }
        test("malformed frame validation") {
            let writeTarget = try target(identity())
            for bad in [-1, 1.5, true, 16 * megabyte + 1, "1"] as [Any] {
                refuse("invalid units rejected") { _ = try call("textBegin", ["target": writeTarget, "units": bad]) }
            }
            for field in ["index", "bytes"] {
                for bad in [-1, 1.5, true, "1"] as [Any] {
                    let id = try begin(writeTarget, units: 1)
                    refuse("invalid \(field) rejected") {
                        _ = try call(field == "index" ? "textAppend" : "textFinish", ["id": id, field: bad, "text": "x"])
                    }
                    refuse("malformed \(field) retires handle") { _ = try call("textFinish", ["id": id, "bytes": 0]) }
                }
            }
            for text in ["", String(repeating: "x", count: pieceSize + 1)] {
                let id = try begin(writeTarget, units: pieceSize + 1)
                refuse("invalid piece size rejected") { _ = try call("textAppend", ["id": id, "index": 0, "text": text]) }
            }
            let emoji = String(repeating: "😀", count: pieceSize / 2)
            let fullUnits = try begin(writeTarget, units: pieceSize)
            check(try call("textAppend", ["id": fullUnits, "index": 0, "text": emoji])["next"] as? Int == 1,
                  "piece ceiling counts UTF16 units rather than UTF8 bytes")
            try release(fullUnits)
            let surrogate = try begin(writeTarget, units: 1)
            let invalidUnits: [unichar] = [0xd800]
            let invalidText = NSString(characters: invalidUnits, length: 1)
            refuse("unpaired UTF16 scalar rejected before bridging") {
                _ = try owner.request(["action": "textAppend", "id": surrogate, "index": 0, "text": invalidText])
            }
            refuse("unpaired scalar retires transfer") { _ = try call("textFinish", ["id": surrogate, "bytes": 0]) }
            for schema in [true, 1.5, 2] as [Any] {
                refuse("schema validation retained") {
                    _ = try call("textBegin", ["target": writeTarget.merging(["schemaVersion": schema]) { _, new in new }, "units": 1])
                }
            }
            refuse("complete oversized inline request rejected") {
                _ = try call("write", writeTarget.merging(["payload": payload(384 * 1_024)]) { _, new in new })
            }
            refuse("invalid JSON still rejected by real store") { _ = try upload(writeTarget, "{") }
            refuse("array payload still rejected by real store") { _ = try upload(writeTarget, "[]") }
            refuse("foreign payload still rejected by real store") { _ = try upload(writeTarget, payload(100, environment: "other")) }
            let id = try begin(writeTarget, units: 1_025)
            for index in 0..<1_024 { _ = try call("textAppend", ["id": id, "index": index, "text": "x"]) }
            refuse("tiny pieces cannot exceed 1024") { _ = try call("textAppend", ["id": id, "index": 1_024, "text": "x"]) }
        }
        test("every scope revokes rowless writers and active reads") {
            let scopes: [(String, Object)] = [("clear", [:]), ("clear", ["environmentId": "one"]),
                ("clear", ["environmentId": "one", "kind": "thread"]), ("clear", identity()),
                ("clearKind", ["kind": "thread"]), ("remove", identity())]
            for (action, fields) in scopes {
                _ = try call("clear")
                let text = payload(2 * pieceSize)
                let rowless = try begin(target(identity()), units: text.utf16.count)
                _ = try call(action, fields)
                check(try call("textAppend", ["id": rowless, "index": 0, "text": "x"])["stale"] as? Bool == true, "\(action) retires rowless writer")
                check(try call("textFinish", ["id": rowless, "bytes": 0])["written"] as? Bool == false, "stale write result is replayable")
                try release(rowless)
                _ = try write(identity(), text)
                let manifest = try transfer(call("read", identity()))
                _ = try call(action, fields)
                refuse("\(action) revokes captured read") { _ = try call("textPart", ["id": manifest["id"]!, "index": 0]) }
            }
            let foreign = identity("other", "server-config", "config")
            let value = payload(100, environment: "other")
            let unaffected = try begin(target(foreign), units: value.utf16.count)
            _ = try call("clear", identity())
            try send(unaffected, value)
            check(try call("textFinish", ["id": unaffected, "bytes": value.utf8.count])["written"] as? Bool == true, "unrelated identity upload survives key clear")
            try release(unaffected)
        }
        test("CAS, replacement and read completion ownership") {
            let text = payload(2 * pieceSize), replacement = payload(2 * pieceSize, prefix: "new")
            _ = try write(identity(), text)
            let manifest = try transfer(call("read", identity()))
            let pending = try begin(target(identity()), units: text.utf16.count)
            let removeTarget = identity().merging(["action": "remove"]) { _, new in new }
            check(try upload(removeTarget, replacement)["removed"] as? Int == 0, "failed CAS leaves row")
            check(exact(try download(manifest, finish: false), text), "failed CAS leaves captured read")
            try send(pending, text)
            check(try call("textFinish", ["id": pending, "bytes": text.utf8.count])["written"] as? Bool == true, "failed CAS preserves sibling writer")
            try release(pending)
            check(try call("textReadFinish", ["id": manifest["id"]!])["complete"] as? Bool == true, "unchanged write preserves captured read")
            try release(manifest["id"] as! String)
            let oldRead = try transfer(call("read", identity()))
            _ = try download(oldRead, finish: false)
            _ = try write(identity(), replacement)
            refuse("replacement invalidates read at finish") { _ = try call("textReadFinish", ["id": oldRead["id"]!]) }
            check(try upload(removeTarget, text)["removed"] as? Int == 0, "old CAS cannot remove replacement")
            let matchedRead = try transfer(call("read", identity()))
            let writer = try begin(target(identity()), units: text.utf16.count)
            check(try upload(removeTarget, replacement)["removed"] as? Int == 1, "matched CAS removes replacement")
            refuse("matched CAS revokes outgoing") { _ = try call("textPart", ["id": matchedRead["id"]!, "index": 0]) }
            check(try call("textFinish", ["id": writer, "bytes": 0])["stale"] as? Bool == true, "matched CAS stales sibling writer")
            try release(writer)
            _ = try write(identity(), text)
            let incomplete = try transfer(call("read", identity()))
            refuse("partial read cannot finish") { _ = try call("textReadFinish", ["id": incomplete["id"]!]) }
        }
        test("external tickets, corrupt discard and forget") {
            let text = payload(2 * pieceSize)
            _ = try write(identity(), text)
            let manifest = try transfer(call("read", identity()))
            let other = T3MobileClientCache(directory: path)
            _ = try other.request(["action": "clear", "environmentId": "one"])
            refuse("part validates durable ticket from another owner") { _ = try call("textPart", ["id": manifest["id"]!, "index": 0]) }
            _ = try write(identity(), text)
            let complete = try transfer(call("read", identity()))
            _ = try download(complete, finish: false)
            _ = try other.request(["action": "clearKind", "kind": "thread"])
            refuse("finish validates durable ticket from another owner") { _ = try call("textReadFinish", ["id": complete["id"]!]) }
            _ = try write(identity(), text)
            let corruptRead = try transfer(call("read", identity()))
            let writer = try begin(target(identity()), units: text.utf16.count)
            try tamper("UPDATE client_cache SET payload='{' WHERE environment_id='one'")
            check(try call("read", identity())["discarded"] as? Bool == true, "real store discards corruption")
            refuse("corrupt discard revokes prior read") { _ = try call("textPart", ["id": corruptRead["id"]!, "index": 0]) }
            check(try call("textFinish", ["id": writer, "bytes": 0])["stale"] as? Bool == true, "corrupt discard stales writer")
            try release(writer)
            let forgotten = try begin(target(identity()), units: text.utf16.count)
            try owner.clearForgottenEnvironment("one")
            check(try call("textFinish", ["id": forgotten, "bytes": 0])["stale"] as? Bool == true, "native forget revokes rowless writer")
            try release(forgotten)
        }
        test("independent overlaps and capacity") {
            _ = try call("clear")
            let first = payload(100, prefix: "first"), last = payload(100, prefix: "last")
            let sameTarget = try target(identity())
            let a = try begin(sameTarget, units: first.utf16.count), b = try begin(sameTarget, units: last.utf16.count)
            try send(a, first)
            try send(b, last)
            _ = try call("textFinish", ["id": a, "bytes": first.utf8.count])
            _ = try call("textFinish", ["id": b, "bytes": last.utf8.count])
            check(exact(try read(identity())?["payload"] as? String, last), "same-ticket writes preserve commit ordering")
            try release(a)
            try release(b)
            var handles = [String]()
            for _ in 0..<8 { handles.append(try begin(target(identity()), units: 1)) }
            refuse("eight slots bound overlapping transfers") { _ = try begin(target(identity()), units: 1) }
            try release(handles.removeLast())
            let replacement = try begin(target(identity()), units: 1)
            for id in handles + [replacement] { try release(id) }
            let huge = payload(16 * megabyte)
            _ = try write(identity(), huge)
            let outgoing = try transfer(call("read", identity()))
            let staged = try begin(target(identity("one", "thread", "other")), units: huge.utf16.count)
            try send(staged, huge)
            let overflow = try begin(target(identity()), units: 1)
            refuse("incoming and outgoing share aggregate byte bound") { _ = try call("textAppend", ["id": overflow, "index": 0, "text": "x"]) }
            try release(outgoing["id"] as! String)
            let available = try begin(target(identity()), units: 1)
            _ = try call("textAppend", ["id": available, "index": 0, "text": "x"])
            try release(available)
            check(try call("textFinish", ["id": staged, "bytes": huge.utf8.count])["written"] as? Bool == true, "capacity failure leaves other upload intact")
            try release(staged)
        }
        test("metadata replies are framed and revoked") {
            _ = try call("clear")
            let value = payload(100)
            for index in 0..<64 {
                _ = try write(identity("one", "thread", String(repeating: "k", count: 8_000) + String(index)), value)
            }
            let fields: Object = ["kind": "thread", "limit": 128]
            let manifest = try transfer(call("list", fields))
            check(manifest["kind"] as? String == "json", "large metadata has JSON transfer kind")
            let text = try download(manifest)
            let object = try JSONSerialization.jsonObject(with: Data(text.utf8)) as! Object
            check((object["rows"] as? [Object])?.count == 64, "metadata transfer is complete")
            try release(manifest["id"] as! String)
            let revoked = try transfer(call("list", fields))
            _ = try write(identity(), value)
            refuse("changed write revokes related metadata transfer") { _ = try call("textPart", ["id": revoked["id"]!, "index": 0]) }
        }
        test("idle expiry, teardown and admitted forget") {
            let expired = DispatchSemaphore(value: 0)
            let expiring = T3MobileCacheTransport(store: store, lifetime: .nanoseconds(0), didExpire: { expired.signal() })
            let handle = try begin(target(identity()), units: 1, using: expiring)
            check(expired.wait(timeout: .now() + 2) == .success, "native timer expires transfer while idle")
            refuse("expired handle unavailable") { _ = try call("textFinish", ["id": handle, "bytes": 0], using: expiring) }
            expiring.close()
            let text = payload(100)
            _ = try write(identity(), text)
            let admittedTicket = try ticket(identity())
            let pending = try begin(target(identity()), units: text.utf16.count)
            owner.close()
            owner.close()
            refuse("closed owner refuses original calls") { _ = try call("read", identity()) }
            refuse("closed owner refuses transfer calls") { _ = try call("textFinish", ["id": pending, "bytes": 0]) }
            try owner.clearForgottenEnvironment("one")
            store = T3MobileClientCache(directory: path)
            owner = T3MobileCacheTransport(store: store)
            check(try read(identity()) == nil, "admitted forget remains durable after close")
            let stale = try call("textBegin", ["target": target(identity(), token: admittedTicket), "units": text.utf16.count])
            check(stale["stale"] as? Bool == true, "restart rejects producer predating forget")
            refuse("restart cannot recover old transfer") { _ = try call("textFinish", ["id": pending, "bytes": text.utf8.count]) }
        }
        owner.close()
        print("Client cache transport: \(checks - failures.count)/\(checks) checks passed")
        for failure in failures { print("FAIL: \(failure)") }
        if !failures.isEmpty { exit(1) }
    }
}
