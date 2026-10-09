// swiftc -o /tmp/t3-client-cache-tests modules/apple/T3MobileClientCache.swift tests/ClientCacheTests.swift -lsqlite3
import Foundation
import SQLite3

@main struct ClientCacheTests {
    typealias Object = [String: Any]
    static func main() throws {
        let manager = FileManager.default
        let root = manager.temporaryDirectory.appendingPathComponent("t3-client-cache-" + UUID().uuidString)
        try manager.createDirectory(at: root, withIntermediateDirectories: true)
        defer { try? manager.removeItem(at: root) }
        var checks = 0, failures = [String]()
        func check(_ condition: Bool, _ message: String) {
            checks += 1
            if !condition { failures.append(message) }
        }
        func refuse(_ message: String, _ run: () throws -> Void) {
            do { try run(); check(false, message) } catch { check(true, message) }
        }
        func test(_ message: String, _ run: () throws -> Void) {
            do { try run() } catch { check(false, "\(message): \(error)") }
        }
        let path = root.appendingPathComponent("mobile-client-cache")
        var owner: T3MobileClientCache? = T3MobileClientCache(directory: path)
        func id(_ env: String = "one", _ kind: String = "shell", _ key: String = "snapshot") -> Object {
            ["environmentId": env, "kind": kind, "key": key]
        }
        func call(_ action: String, _ fields: Object = [:], using other: T3MobileClientCache? = nil) throws -> Object {
            let reply = try (other ?? owner!).request(fields.merging(["action": action]) { _, new in new })
            // The module sends JSON, so inspect numbers after that boundary rather
            // than casting SQLite's native Int64 values directly to Swift Int.
            let data = try JSONSerialization.data(withJSONObject: reply)
            return try JSONSerialization.jsonObject(with: data) as! Object
        }
        func payload(_ env: String = "one", _ text: String = "hello") throws -> String {
            String(data: try JSONSerialization.data(withJSONObject: ["format": "t3-code-mobile-read-cache", "schemaVersion": 1, "environmentId": env, "text": text], options: [.sortedKeys]), encoding: .utf8)!
        }
        func ticket(_ identity: Object, using other: T3MobileClientCache? = nil) throws -> String {
            try call("ticket", identity, using: other)["ticket"] as! String
        }
        func write(_ identity: Object, _ value: String, _ captured: String? = nil, using other: T3MobileClientCache? = nil) throws -> Object {
            let token = try captured ?? ticket(identity, using: other)
            return try call("write", identity.merging(["payload": value, "schemaVersion": 1, "ticket": token]) { _, new in new }, using: other)
        }
        func read(_ identity: Object, using other: T3MobileClientCache? = nil) throws -> Object? {
            try call("read", identity, using: other)["record"] as? Object
        }
        func tamper(_ sql: String) throws {
            var database: OpaquePointer?
            guard sqlite3_open(path.appendingPathComponent("cache.sqlite3").path, &database) == SQLITE_OK else { throw CocoaError(.fileReadUnknown) }
            defer { sqlite3_close(database) }
            guard sqlite3_exec(database, sql, nil, nil, nil) == SQLITE_OK else { throw CocoaError(.fileWriteUnknown) }
        }

        let protectedNames = ["t3-code.json", "outbox.json", "credentials", "composer-files/attachment", "incoming-shares/original"]
        let protected = Data("protected preferences, credentials and attachment bytes 🧭".utf8)
        for name in protectedNames {
            let file = root.appendingPathComponent(name)
            try manager.createDirectory(at: file.deletingLastPathComponent(), withIntermediateDirectories: true)
            try protected.write(to: file)
        }
        test("empty cache") {
            check(try read(id()) == nil, "missing record returns null")
            let summary = try call("inspect")
            check(summary["recordCount"] as? Int == 0 && summary["payloadBytes"] as? Int == 0, "empty summary is actual zero")
        }
        test("upsert and UTF8") {
            let unicode = try payload("one", "é👨‍👩‍👧‍👦東京")
            check(try write(id(), unicode)["written"] as? Bool == true, "first write accepted")
            let summary = try call("inspect")
            check(summary["recordCount"] as? Int == 1, "one upsert record")
            check(summary["payloadBytes"] as? Int == unicode.utf8.count, "payload summary counts UTF8 bytes")
            check(try read(id())?["payload"] as? String == unicode, "Unicode payload round trips exactly")
            check(try write(id(), unicode)["changed"] as? Bool == false, "identical payload deduplicated")
            let replacement = try payload("one", "new value")
            check(try write(id(), replacement)["changed"] as? Bool == true, "changed payload updates")
            check(try call("inspect")["recordCount"] as? Int == 1, "replacement does not add a row")
            owner = nil
            owner = T3MobileClientCache(directory: path)
            check(try read(id())?["payload"] as? String == replacement, "production database survives owner restart")
        }
        test("identity isolation and SQL binding") {
            let hostile = "two'; DROP TABLE client_cache;--"
            _ = try write(id(hostile), payload(hostile))
            _ = try write(id("one", "thread", "thread'; DELETE FROM client_cache;--"), payload())
            check(try read(id(hostile))?["environmentId"] as? String == hostile, "environment SQL metacharacters are data")
            check(try call("inspect")["recordCount"] as? Int == 3, "same keys in different environments remain separate")
            check(try call("inspect", ["environmentId": hostile])["recordCount"] as? Int == 1, "scoped summary selects exact environment")
            let foreign = try ticket(id("foreign"))
            check(try write(id(), payload(), foreign)["stale"] as? Bool == true, "producer ticket is bound to exact identity")
        }
        test("invalid requests") {
            let token = try ticket(id())
            for (label, fields) in [
                ("invalid JSON", ["payload": "{"]),
                ("array payload", ["payload": "[]"]),
                ("foreign identity", ["payload": try payload("foreign")]),
                ("upstream schema is not app schema", ["schemaVersion": 3]),
                ("boolean schema rejected", ["schemaVersion": true]),
                ("fractional schema rejected", ["schemaVersion": 1.5]),
                ("oversized shell", ["payload": try payload("one", String(repeating: "x", count: 4 * 1_024 * 1_024))])
            ] as [(String, Object)] {
                refuse(label) {
                    _ = try call("write", id().merging(["schemaVersion": 1, "payload": try payload(), "ticket": token]) { _, next in next }.merging(fields) { _, next in next })
                }
            }
            refuse("unknown kind") { _ = try call("ticket", id("one", "credentials")) }
            refuse("empty environment") { _ = try call("ticket", id("")) }
            refuse("NUL identity") { _ = try call("ticket", id("a\0b")) }
            refuse("wrong shell key") { _ = try call("ticket", id("one", "shell", "other")) }
            refuse("unknown action") { _ = try call("erase-directory") }
            refuse("clear kind without environment") { _ = try call("clear", ["kind": "shell"]) }
            refuse("clear key without kind") { _ = try call("clear", ["environmentId": "one", "key": "snapshot"]) }
            check(try call("inspect")["recordCount"] as? Int == 3, "refused operations preserve records")
        }
        test("clear and producer ordering across independent owners") {
            let second = T3MobileClientCache(directory: path)
            let before = try ticket(id(), using: second)
            let otherBefore = try ticket(id("other"), using: second)
            _ = try write(id("other"), payload("other"), otherBefore, using: second)
            _ = try call("clear", ["environmentId": "one"])
            check(try read(id()) == nil, "environment clear removes selected data")
            check(try write(id(), payload(), before, using: second)["stale"] as? Bool == true, "pre-clear producer rejected across owner")
            check(try write(id("other"), payload("other", "fresh"), otherBefore, using: second)["written"] as? Bool == true, "environment clear leaves another environment ticket valid")
            let after = try ticket(id(), using: second)
            check(after != before, "clear changes producer epoch")
            check(try write(id(), payload(), after, using: second)["written"] as? Bool == true, "fresh post-clear snapshot can repopulate")
            owner = nil; owner = T3MobileClientCache(directory: path)
            check(try write(id(), payload(), before)["stale"] as? Bool == true, "old producer remains invalid after restart")
            let thread = id("one", "thread", "t")
            let threadTicket = try ticket(thread)
            _ = try write(thread, payload())
            _ = try call("clear", ["environmentId": "one", "kind": "shell"])
            check(try write(thread, payload(), threadTicket)["written"] as? Bool == true, "kind clear preserves unrelated kind producer")
            let beforeRemove = try ticket(thread)
            _ = try call("remove", thread)
            check(try write(thread, payload(), beforeRemove)["stale"] as? Bool == true, "remove prevents deleted thread resurrection")
            let beforeAll = try ticket(id("other"))
            _ = try call("clear")
            check(try call("inspect")["recordCount"] as? Int == 0, "all clear deletes actual records")
            check(try write(id("other"), payload("other"), beforeAll)["stale"] as? Bool == true, "all clear invalidates every scope")
        }
        test("corrupt records") {
            _ = try write(id(), payload())
            let before = try ticket(id())
            try tamper("UPDATE client_cache SET payload='{'")
            check(try read(id()) == nil, "invalid saved JSON discarded")
            check(try call("inspect")["recordCount"] as? Int == 0, "invalid record removed from summary")
            check(try write(id(), payload(), before)["stale"] as? Bool == true, "corrupt discard invalidates old writer")
            _ = try write(id(), payload())
            try tamper("UPDATE client_cache SET schema_version=99")
            check(try read(id()) == nil, "unsupported saved version discarded")
            _ = try write(id(), payload())
            try tamper("UPDATE client_cache SET payload='{\"environmentId\":\"foreign\"}'")
            check(try read(id()) == nil, "foreign saved identity discarded")
        }
        test("all kinds and scoped key clear") {
            let identities = [id(), id("one", "thread", "t"), id("one", "server-config", "config"), id("one", "vcs-refs", "/cwd"), id("one", "project-favicon", "favicon")]
            for identity in identities { _ = try write(identity, payload()) }
            check((try call("inspect")["rows"] as? [Object])?.count == 5, "all five real kinds inspected")
            _ = try call("clear", id("one", "vcs-refs", "/cwd"))
            check(try call("inspect")["recordCount"] as? Int == 4, "exact key clear retains other kinds")
            let before = try ticket(id())
            check(try call("remove", id().merging(["expectedPayload": "old value"]) { _, next in next })["removed"] as? Int == 0, "conditional remove preserves newer payload")
            check(try write(id(), payload(), before)["written"] as? Bool == true, "failed conditional remove preserves producer ticket")
            check(try call("remove", id().merging(["expectedPayload": try payload()]) { _, next in next })["removed"] as? Int == 1, "conditional remove deletes exact payload")
            _ = try call("clear")
        }
        test("kind clear invalidates global producers without clearing unrelated data") {
            let icon = id("one", "project-favicon", "icon"), pending = id("no-rows", "project-favicon", "icon")
            _ = try write(icon, payload())
            _ = try write(id(), payload())
            let iconTicket = try ticket(icon), pendingTicket = try ticket(pending), shellTicket = try ticket(id())
            check(try call("clearKind", ["kind": "project-favicon"])["removed"] as? Int == 1, "kind clear removes exact rows")
            check(try read(id()) != nil, "kind clear preserves shell data")
            check(try write(icon, payload(), iconTicket)["stale"] as? Bool == true, "kind clear rejects existing producer")
            check(try write(pending, payload("no-rows"), pendingTicket)["stale"] as? Bool == true, "kind clear rejects rowless environment producer")
            check(try write(id(), payload(), shellTicket)["written"] as? Bool == true, "unrelated producer remains valid")
            check(try write(icon, payload())["written"] as? Bool == true, "fresh icon producer can write")
            owner = nil; owner = T3MobileClientCache(directory: path)
            check(try write(pending, payload("no-rows"), pendingTicket)["stale"] as? Bool == true, "kind invalidation survives reopening")
            refuse("kind clear refuses ambiguous scope") { _ = try call("clearKind", ["kind": "shell", "environmentId": "one"]) }
            _ = try call("clear")
        }
        test("bounded global keyset enumeration") {
            let bmp = "\u{E000}", astral = "😀"
            let identities = [id(bmp, "project-favicon", "a"), id(astral, "project-favicon", "a"),
                              id("one", "project-favicon", bmp), id("one", "project-favicon", astral)]
            for identity in identities { _ = try write(identity, payload(identity["environmentId"] as! String)) }
            _ = try write(id(), payload())
            try tamper("UPDATE client_cache SET updated_at=100")
            let first = try call("list", ["kind": "project-favicon", "limit": 2])["rows"] as! [Object]
            check(first.count == 2, "page is bounded and excludes another kind")
            check(first.map { $0["key"] as! String } == [bmp, astral], "equal-time keys use UTF8 scalar order")
            check(first.allSatisfy { $0["payload"] == nil }, "metadata page does not load payloads")
            for row in first { _ = try call("remove", row) }
            let second = try call("list", ["kind": "project-favicon", "limit": 2, "after": first.last!])["rows"] as! [Object]
            check(second.map { $0["environmentId"] as! String } == [bmp, astral], "deletion between pages preserves every later environment")
            let end = try call("list", ["kind": "project-favicon", "limit": 2, "after": second.last!])["rows"] as! [Object]
            check(end.isEmpty, "last key yields empty page")
            for invalid in [0, -1, 129, 1.5, true] as [Any] {
                refuse("invalid page limit rejected") { _ = try call("list", ["kind": "project-favicon", "limit": invalid]) }
            }
            for invalid in [-1, 1.5, true, 9_007_199_254_740_992] as [Any] {
                let cursor = first[0].merging(["updatedAt": invalid]) { _, next in next }
                refuse("invalid cursor timestamp rejected") { _ = try call("list", ["kind": "project-favicon", "limit": 2, "after": cursor]) }
            }
            refuse("cross-kind cursor rejected") { _ = try call("list", ["kind": "thread", "limit": 2, "after": first[0]]) }
            refuse("malformed cursor rejected") { _ = try call("list", ["kind": "project-favicon", "limit": 2, "after": "bad"]) }
            _ = try call("clear")
        }
        test("separate database ticket and failure isolation") {
            let other = T3MobileClientCache(directory: root.appendingPathComponent("different-cache"))
            let token = try ticket(id())
            check(try write(id(), payload(), token, using: other)["stale"] as? Bool == true, "ticket cannot target another database")
            let bad = root.appendingPathComponent("bad-cache")
            try manager.createDirectory(at: bad, withIntermediateDirectories: true)
            try Data("not sqlite".utf8).write(to: bad.appendingPathComponent("cache.sqlite3"))
            let corrupt = T3MobileClientCache(directory: bad)
            refuse("corrupt database reports unavailable, not zero") { _ = try corrupt.request(["action": "inspect"]) }
            let blocked = T3MobileClientCache(directory: root.appendingPathComponent("t3-code.json"))
            refuse("file cannot serve as cache directory") { _ = try blocked.request(["action": "inspect"]) }
        }
        test("symlink refusal") {
            let link = root.appendingPathComponent("linked-cache")
            try manager.createSymbolicLink(at: link, withDestinationURL: path)
            refuse("cache directory symlink rejected") { _ = try T3MobileClientCache(directory: link).request(["action": "inspect"]) }
            for suffix in ["", "-wal", "-shm", "-journal"] {
                let folder = root.appendingPathComponent("link-" + UUID().uuidString)
                try manager.createDirectory(at: folder, withIntermediateDirectories: true)
                try manager.createSymbolicLink(at: folder.appendingPathComponent("cache.sqlite3" + suffix), withDestinationURL: root.appendingPathComponent("t3-code.json"))
                refuse("SQLite \(suffix) symlink rejected") { _ = try T3MobileClientCache(directory: folder).request(["action": "inspect"]) }
            }
        }
        test("protected files") {
            for name in protectedNames { check(try Data(contentsOf: root.appendingPathComponent(name)) == protected, "\(name) remains byte-identical") }
        }
        print("Client cache: \(checks - failures.count)/\(checks) checks passed")
        for failure in failures { print("FAIL: \(failure)") }
        if !failures.isEmpty { exit(1) }
    }
}
