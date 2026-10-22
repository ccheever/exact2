import Foundation

/// Production transition helper and production POSIX replacement, on separate isolated roots.
/// The UIKit module and transport are SDK typechecked separately; this is component execution.
private final class QuestionPreferences {
    let disk: T3QuestionAttachmentFiles
    let attachments: T3QuestionAttachments
    let root: URL
    let lock = NSLock()
    var fail = false
    lazy var owner = T3QuestionCommands(root: root, read: { [self] in
        guard let data = try disk.load() else { return [:] }
        return try JSONSerialization.jsonObject(with: data) as! [String: Any]
    }, save: { [self] value in
        if fail { throw CocoaError(.fileWriteOutOfSpace) }
        try disk.replace(T3QuestionCommands.encoded(value))
    }, inventory: { [self] in attachments })
    init(_ root: URL) {
        self.root = root
        for path in ["preferences", "attachments"] { try! FileManager.default.createDirectory(at: root.appendingPathComponent(path), withIntermediateDirectories: true) }
        disk = .init(dataRoot: root.appendingPathComponent("preferences"))
        attachments = .init(disk: T3QuestionAttachmentFiles(dataRoot: root.appendingPathComponent("attachments")))
    }
    func raw() throws -> [String: Any] { guard let bytes = try disk.load() else { return [:] }; return try JSONSerialization.jsonObject(with: bytes) as! [String: Any] }
    func call(_ action: String, _ extra: [String: Any] = [:], connection: T3QuestionConnection? = QuestionCommandTests.connection) throws -> [String: Any] {
        lock.lock(); defer { lock.unlock() }
        var input = extra.merging(["op": "mobileQuestionJournal", "action": action], uniquingKeysWith: { _, new in new })
        if action != "read" { input["mutationId"] = "\(extra["commandId"] ?? "prepare"):\(action):\(extra["epoch"] ?? ""):\(extra["revision"] ?? "")" }
        return try owner.request(input, connection: connection)
    }
}

@main private struct QuestionCommandTests {
    static let connection = T3QuestionConnection(home: "https://home.test", endpoint: "https://route.test", environment: "env", generation: 1, catalog: ["https://home.test", "https://route.test"])
    static var failures: [String] = [], count = 0, roots: [URL] = []
    static func check(_ value: Bool, _ name: String) { count += 1; if !value { failures.append(name) } }
    static func run(_ name: String, _ body: () throws -> Void) { do { try body() } catch { count += 1; failures.append("\(name): \(error)") } }
    static func refuses(_ name: String, _ body: () throws -> Void) { do { try body(); check(false, name) } catch { check(true, name) } }
    static func root() -> URL { let value = FileManager.default.temporaryDirectory.appendingPathComponent("t3-question-command-\(UUID().uuidString)"); roots.append(value); return value }
    static func record(_ command: String = "command", type: String = "runtime-request.respond") -> [String: Any] {
        var payload: [String: Any] = ["type": type, "commandId": command, "threadId": "thread"]
        if type == "run.interrupt" { payload["runId"] = "run"; payload["holdQueue"] = true }
        else { payload["requestId"] = "request"; if type == "runtime-request.respond" { payload["answers"] = ["q": "Unicode café 🧪 / quotes \" backslash \\ "] } }
        return ["origin": connection.endpoint, "environmentId": "env", "threadId": "thread", "requestId": "request", "signature": "request-signature",
            "pending": ["method": "orchestration.dispatchCommand", "payload": payload, "description": "Answers", "threadId": "thread", "text": "", "uncertain": false],
            "drafts": ["q": ["selected": [String](), "custom": "  local untrimmed  "]], "phase": "preflight"]
    }
    static func marker(_ value: [String: Any], _ command: String = "command") -> [String: Any] {
        let journal = value[T3QuestionCommands.metadataKey] as! [String: Any]
        let meta = (journal["commands"] as! [String: [String: Any]])[command]!
        return ["epoch": journal["epoch"]!, "revision": journal["revision"]!, "commandId": command,
                "recordRevision": meta["recordRevision"]!, "recordDigest": meta["recordDigest"]!, "payloadDigest": meta["payloadDigest"]!]
    }
    static func prepare(_ prefs: QuestionPreferences, _ command: String = "command", type: String = "runtime-request.respond") throws -> [String: Any] {
        _ = try prefs.call("read")
        let prepared = try prefs.call("prepare", ["record": record(command, type: type), "catalog": ["q": "signature"],
            "generation": 1, "expectedOrigin": connection.endpoint, "expectedEnvironmentId": "env"])
        let journal = prepared[T3QuestionCommands.metadataKey] as! [String: Any]
        let created = try prefs.call("create", ["epoch": journal["epoch"]!, "revision": journal["revision"]!, "commandId": command])
        return try prefs.call("bind", marker(created, command))
    }
    static func issued(_ prefs: QuestionPreferences, _ value: [String: Any], rpc: String) throws -> (String, String, String) {
        let phase = try prefs.call("phase", marker(value).merging(["phase": "issued"], uniquingKeysWith: { _, n in n }))
        prefs.lock.lock(); defer { prefs.lock.unlock() }
        return try prefs.owner.issue(marker(phase), connection: connection, rpcID: rpc)
    }
    static func clean(_ prefs: QuestionPreferences, _ value: [String: Any]) throws -> [String: Any] {
        let record = (value[T3QuestionCommands.recordsKey] as! [String: [String: Any]])["command"]!
        var value = value
        if record["phase"] as? String == "settled", ((record["pending"] as! [String: Any])["payload"] as! [String: Any])["type"] as? String != "run.interrupt" {
            value = try prefs.call("terminal", marker(value).merging(["phase": "settled", "consumption": ["complete": true, "threadId": "thread", "requests": [[String: Any]](), "latestItemId": ""]], uniquingKeysWith: { _, n in n }))
        }
        let released = try prefs.call("release", marker(value))
        let journal = released[T3QuestionCommands.metadataKey] as! [String: Any], meta = (journal["commands"] as! [String: [String: Any]])["command"]!
        let ack = try prefs.call("ack", marker(released).merging(["releaseToken": meta["releaseToken"]!], uniquingKeysWith: { _, n in n }))
        return try prefs.call("remove", marker(ack))
    }
    static func child(_ mode: String, _ root: URL) throws {
        let prefs = QuestionPreferences(root)
        if mode == "child-prepared" {
            let prepared = try prefs.call("prepare", ["record": record(), "catalog": ["q": "signature"], "generation": 1,
                "expectedOrigin": connection.endpoint, "expectedEnvironmentId": "env"])
            let journal = prepared[T3QuestionCommands.metadataKey] as! [String: Any]
            _ = try prefs.call("seal", ["epoch": journal["epoch"]!, "revision": journal["revision"]!]); exit(0)
        }
        let value = try prepare(prefs)
        if mode == "child-terminal" {
            let attempt = try issued(prefs, value, rpc: "1-1")
            let completed = try prefs.owner.complete(command: attempt.1, attempt: attempt.2, result: ["ok": true])
            _ = try clean(prefs, completed)
        }
        if mode != "child-unsealed" {
            let state = try prefs.call("read"), journal = state[T3QuestionCommands.metadataKey] as! [String: Any]
            _ = try prefs.call("seal", ["epoch": journal["epoch"]!, "revision": journal["revision"]!])
        }
        // Actual abrupt process exit. No same-process recreation can substitute for this witness.
        exit(0)
    }
    static func launch(_ mode: String, _ root: URL) throws {
        let process = Process(); process.executableURL = URL(fileURLWithPath: CommandLine.arguments[0]); process.arguments = [mode, root.path]
        try process.run(); process.waitUntilExit(); check(process.terminationStatus == 0, "Actual child process \(mode) exits successfully")
    }
    static func main() throws {
        if CommandLine.arguments.count == 3 { try child(CommandLine.arguments[1], URL(fileURLWithPath: CommandLine.arguments[2])); return }
        defer { roots.forEach { try? FileManager.default.removeItem(at: $0) } }
        run("wire and outcome") {
            let prefs = QuestionPreferences(root()), value = try prepare(prefs), attempt = try issued(prefs, value, rpc: "1-1")
            let wire = try JSONSerialization.jsonObject(with: Data(attempt.0.utf8)) as! [String: Any]
            check(wire["id"] as? String == "1-1", "Fresh envelope uses its live correlation ID")
            check(T3QuestionCommands.equal(wire["payload"], (record()["pending"] as! [String: Any])["payload"]), "Unicode and escapes survive opaque native payload")
            refuses("Another live attempt is refused") { _ = try prefs.owner.issue(marker(try prefs.call("read")), connection: connection, rpcID: "1-2") }
            let uncertain = try prefs.owner.complete(command: attempt.1, attempt: attempt.2, result: ["ok": false, "_definitiveFailure": true])
            check(((uncertain[T3QuestionCommands.recordsKey] as! [String: [String: Any]])["command"]?["phase"] as? String) == "uncertain", "Generic typed Fail remains uncertain")
            let retry = try issued(prefs, uncertain, rpc: "1-9")
            check(attempt.0.components(separatedBy: "\"payload\":")[1] == retry.0.components(separatedBy: "\"payload\":")[1], "Explicit Retry preserves exact payload bytes with a fresh envelope ID")
            let done = try prefs.owner.complete(command: retry.1, attempt: retry.2, result: ["ok": true])
            let removed = try clean(prefs, done)
            check((removed[T3QuestionCommands.recordsKey] as! [String: Any]).isEmpty, "Terminal release and ack precede receipt removal")
            check(((removed[T3QuestionCommands.metadataKey] as! [String: Any])["tombstones"] as! [String: Any])["command"] != nil, "Same-process removal keeps a command tombstone")
        }
        run("save-only outcome") {
            let prefs = QuestionPreferences(root()), attempt = try issued(prefs, prepare(prefs), rpc: "1-1")
            prefs.fail = true
            refuses("Success plus failed outcome save surfaces failure") { _ = try prefs.owner.complete(command: attempt.1, attempt: attempt.2, result: ["ok": true]) }
            check(prefs.owner.hasWork, "Native outcome stays owned after failed save")
            refuses("Read cannot advertise an unsaved outcome") { _ = try prefs.call("read") }
            prefs.fail = false; let recovered = try prefs.call("read")
            check(((recovered[T3QuestionCommands.recordsKey] as! [String: [String: Any]])["command"]?["phase"] as? String) == "settled", "Fresh status saves the retained success without dispatch")
            check(!prefs.owner.hasWork, "Saved outcome releases native live work retention")
        }
        run("ordinary merge and stale CAS") {
            let prefs = QuestionPreferences(root()), initial = try prepare(prefs)
            var current = try prefs.raw(); current["drafts"] = ["newer": "keep"]; current["composerFiles"] = [["id": "ordinary"]]
            current["mobilePendingTaskEditors"] = ["retained": true]; current["mobileOutboxTransferCompletions"] = ["newer": true]
            let successor: [String: Any] = ["payload": ["commandId": "ordinary", "type": "run.interrupt"]]
            current["pending"] = ["env": successor]; try prefs.disk.replace(T3QuestionCommands.encoded(current))
            let delta = marker(initial).merging(["phase": "issued"], uniquingKeysWith: { _, n in n })
            _ = try prefs.call("phase", delta)
            _ = try prefs.call("phase", delta)
            var other = delta; other["phase"] = "settled"
            check(try prefs.call("terminal", other)["stale"] as? Bool == true,
                  "Two distinct equal-revision CAS deltas yield one commit and current stale status")
            refuses("A repeated mutation ID with different phase inputs is refused") {
                var altered = delta; altered["phase"] = "uncertain"; _ = try prefs.call("phase", altered)
            }
            let saved = try prefs.raw(); check(T3QuestionCommands.equal(saved["drafts"], current["drafts"]) && T3QuestionCommands.equal(saved["composerFiles"], current["composerFiles"]), "Question CAS preserves current ordinary draft and collector facts")
            var stale: [String: Any] = ["ordinarySetting": "changed", "pending": ["env": ["payload": ["commandId": "command"]]], T3QuestionCommands.recordsKey: ["corrupt": true]]
            stale = try T3QuestionCommands.merge(previous: saved, incoming: stale)
            check(T3QuestionCommands.equal(stale[T3QuestionCommands.recordsKey], saved[T3QuestionCommands.recordsKey]), "Stale or malformed incoming question map cannot replace the native map")
            check(T3QuestionCommands.equal((stale["pending"] as! [String: Any])["env"], successor), "Filtered stale question pending preserves ordinary successor")
            check(stale["ordinarySetting"] as? String == "changed", "Ordinary save still accepts unrelated setting change")
        }
        run("bound pins and missing preferences") {
            let location = root(), prefs = QuestionPreferences(location); _ = try prepare(prefs)
            try FileManager.default.removeItem(at: location.appendingPathComponent("preferences/question-attachments/inventory.json"))
            refuses("Missing preference file beside a bound pin fails closed") { _ = try prefs.call("read") }
            check(try prefs.attachments.hasBoundPins(), "Missing preferences retain bound byte ownership")
        }
        run("strict decoders") {
            check(T3QuestionCommands.integer(true) == nil && T3QuestionCommands.integer(1.5) == nil && T3QuestionCommands.integer(9_007_199_254_740_992) == nil, "Booleans, nonintegral values and unsafe revisions are rejected")
            var bad = record(type: "run.interrupt"), pending = bad["pending"] as! [String: Any], payload = pending["payload"] as! [String: Any]
            payload["holdQueue"] = 1; pending["payload"] = payload; bad["pending"] = pending
            refuses("Integer holdQueue cannot impersonate true") { _ = try T3QuestionCommands.payload(bad) }
            let prefs = QuestionPreferences(root()); try prefs.disk.replace(T3QuestionCommands.encoded([T3QuestionCommands.recordsKey: ["command": record()]]))
            let initialized = try prefs.call("read")
            check(((initialized[T3QuestionCommands.metadataKey] as! [String: Any])["commands"] as! [String: [String: Any]])["command"]?["homeOrigin"] as? String == connection.home, "Bare current receipt stamps verified home without rewriting captured route")
            let changed = T3QuestionConnection(home: "https://different.test", endpoint: connection.endpoint, environment: "env", generation: 1, catalog: ["https://different.test"])
            let phase = try prefs.call("phase", marker(initialized).merging(["phase": "issued"], uniquingKeysWith: { _, n in n }))
            refuses("Same environment with changed home cannot issue") { _ = try prefs.owner.issue(marker(phase), connection: changed, rpcID: "1-1") }
            var malformed = try prefs.raw(); malformed[T3QuestionCommands.metadataKey] = NSNull(); try prefs.disk.replace(T3QuestionCommands.encoded(malformed))
            refuses("Present null metadata fails closed") { _ = try prefs.call("read") }
        }
        run("endpoint normalization and guarded home") {
            for (raw, expected) in [(" HTTPS://Example.TEST/path?x=1#fragment ", "https://example.test"),
                                     ("https://example.test:8443/api", "https://example.test:8443"),
                                     ("http://[::1]:4312/base", "http://[::1]:4312"),
                                     ("https://[2001:db8::1]:443/path", "https://[2001:db8::1]:443")] {
                let normalized = try T3Endpoint.origin(raw).absoluteString.trimmingCharacters(in: CharacterSet(charactersIn: "/"))
                let scope = try T3QuestionScope(origin: normalized, environment: "env", thread: "thread", request: "request")
                check(scope.origin == expected, "Actual native normalization/scope agreement for \(raw)")
            }
            let prefs = QuestionPreferences(root()), initial = try prepare(prefs)
            let phase = try prefs.call("phase", marker(initial).merging(["phase": "issued"], uniquingKeysWith: { _, n in n }))
            let route = T3QuestionConnection(home: connection.home, endpoint: "https://route-two.test", environment: "env", generation: 2,
                catalog: [connection.home, connection.endpoint, "https://route-two.test"])
            var changed = marker(phase); changed["payloadDigest"] = String(repeating: "0", count: 64)
            refuses("Mutated immutable payload digest cannot issue") { _ = try prefs.owner.issue(changed, connection: route, rpcID: "2-1") }
            var method = marker(phase); method["method"] = "ordinary"
            refuses("Marker cannot inject a changed method") { _ = try prefs.owner.issue(method, connection: route, rpcID: "2-2") }
            let attempt = try prefs.owner.issue(marker(phase), connection: route, rpcID: "2-3")
            check(attempt.1 == "command", "Verified stable home permits a new active route without changing captured command origin")
            _ = try prefs.owner.complete(command: attempt.1, attempt: attempt.2, result: ["ok": false])
            let bare = QuestionPreferences(root()); try bare.disk.replace(T3QuestionCommands.encoded([T3QuestionCommands.recordsKey: ["command": record()]]))
            let removed = T3QuestionConnection(home: connection.home, endpoint: route.endpoint, environment: "env", generation: 2, catalog: [connection.home, route.endpoint])
            let blocked = try bare.call("read", connection: removed)
            let meta = (blocked[T3QuestionCommands.metadataKey] as! [String: Any])["commands"] as! [String: [String: Any]]
            check(meta["command"]?["homeOrigin"] == nil, "Removed recorded route cannot stamp a guessed home")
        }
        run("actual restart seal") {
            let sealed = root(); try launch("child-terminal", sealed)
            let recovered = try QuestionPreferences(sealed).call("read"), meta = recovered[T3QuestionCommands.metadataKey] as! [String: Any]
            check((meta["tombstones"] as! [String: Any]).isEmpty, "A prior durable seal plus actual later process restart permits acknowledged tombstone collection")
            let orphan = root(); try launch("child-prepared", orphan)
            let cleaned = try QuestionPreferences(orphan).call("read")
            check(((cleaned[T3QuestionCommands.metadataKey] as! [String: Any])["prepared"] as! [String: Any]).isEmpty,
                  "Actual restart after prior seal retires an unbound prepared orphan")
            let unsealed = root(); try launch("child-unsealed", unsealed)
            let first = QuestionPreferences(unsealed), state = try first.call("read")
            check(!(state[T3QuestionCommands.recordsKey] as! [String: Any]).isEmpty, "Restart without a prior seal retains bound receipt ownership")
            check(!(try QuestionPreferences(unsealed).call("read")[T3QuestionCommands.recordsKey] as! [String: Any]).isEmpty, "Same-process coordinator recreation cannot collect bound ownership")
        }

        run("same actual process cannot acquire a new seal") {
            let location = root(), first = QuestionPreferences(location)
            let state = try first.call("prepare", ["record": record(), "catalog": ["q": "signature"], "generation": 1,
                "expectedOrigin": connection.endpoint, "expectedEnvironmentId": "env"])
            let meta = state[T3QuestionCommands.metadataKey] as! [String: Any]
            _ = try first.call("seal", ["epoch": meta["epoch"]!, "revision": meta["revision"]!])
            let recreated = QuestionPreferences(location), read = try recreated.call("read")
            check(((read[T3QuestionCommands.metadataKey] as! [String: Any])["prepared"] as! [String: Any])["command"] != nil,
                "A new helper in the same process cannot collect a freshly sealed prepared orphan")
            let again = try QuestionPreferences(location).call("read")
            check(((again[T3QuestionCommands.metadataKey] as! [String: Any])["prepared"] as! [String: Any])["command"] != nil,
                "Repeated same-process reconstruction preserves the same ineligible seal")
        }
        run("strict prepared binding and cross-file quarantine") {
            for mutation in ["missing-pin", "null-pin", "different-pin", "missing-sibling"] {
                let prefs = QuestionPreferences(root())
                var state = try prefs.call("prepare", ["record": record(), "catalog": ["q": "signature"], "generation": 1,
                    "expectedOrigin": connection.endpoint, "expectedEnvironmentId": "env"])
                if mutation == "missing-sibling" { state = try prepare(prefs); state.removeValue(forKey: T3QuestionCommands.metadataKey) }
                else {
                    var journal = state[T3QuestionCommands.metadataKey] as! [String: Any], prepared = journal["prepared"] as! [String: [String: Any]], entry = prepared["command"]!, meta = entry["metadata"] as! [String: Any]
                    if mutation == "missing-pin" { for key in ["pinId", "pinDigest", "requestIncarnation", "inventoryRevision", "questionCatalog"] { meta.removeValue(forKey: key) } }
                    if mutation == "null-pin" { meta["pinId"] = NSNull() }
                    if mutation == "different-pin" { meta["pinId"] = UUID().uuidString.lowercased() }
                    entry["metadata"] = meta; prepared["command"] = entry; journal["prepared"] = prepared; state[T3QuestionCommands.metadataKey] = journal
                }
                try prefs.disk.replace(T3QuestionCommands.encoded(state))
                refuses("Strict prepared/cross-file state rejects \(mutation) without force-cast crash") { _ = try prefs.call("read") }
                let projection = try JSONSerialization.jsonObject(with: Data(prefs.owner.preferencesRead().utf8)) as! [String: Any]
                check((projection["mobileQuestionRecovery"] as? [String: Any])?["blocked"] as? Bool == true,
                    "Corruption remains visible through ordinary preference read for \(mutation)")
            }
        }

        run("different exact bound receipt evidence is quarantined") {
            for mutation in ["record", "payload", "catalog", "incarnation"] {
                let prefs = QuestionPreferences(root()); var state = try prepare(prefs)
                var journal = state[T3QuestionCommands.metadataKey] as! [String: Any], commands = journal["commands"] as! [String: [String: Any]], metadata = commands["command"]!, records = state[T3QuestionCommands.recordsKey] as! [String: [String: Any]], record = records["command"]!
                if mutation == "record" { record["signature"] = "different-incarnation-item"; metadata["recordDigest"] = try T3QuestionCommands.recordDigest(record) }
                if mutation == "payload" {
                    var pending = record["pending"] as! [String: Any], payload = pending["payload"] as! [String: Any]; payload["answers"] = ["q": "different answer"]; pending["payload"] = payload; record["pending"] = pending
                    let bytes = try T3QuestionCommands.encoded(payload); metadata["payloadUTF8"] = String(decoding: bytes, as: UTF8.self); metadata["payloadDigest"] = t3QuestionDigest(bytes); metadata["recordDigest"] = try T3QuestionCommands.recordDigest(record)
                }
                if mutation == "catalog" { metadata["questionCatalog"] = ["q": "different signature"] }
                if mutation == "incarnation" { metadata["requestIncarnation"] = UUID().uuidString.lowercased() }
                commands["command"] = metadata; journal["commands"] = commands; records["command"] = record; state[T3QuestionCommands.metadataKey] = journal; state[T3QuestionCommands.recordsKey] = records
                state["pending"] = ["env": ["payload": ["commandId": "command", "type": "thread.rename"]], "ordinary": ["payload": ["commandId": "keep", "type": "runtime-request.respond"]]]
                try prefs.disk.replace(T3QuestionCommands.encoded(state))
                refuses("Different bound \(mutation) refuses authoritative recovery") { _ = try prefs.call("read") }
                check(try prefs.attachments.hasBoundPins(), "Different bound \(mutation) retains quarantined ownership")
                let projection = try JSONSerialization.jsonObject(with: Data(prefs.owner.preferencesRead().utf8)) as! [String: Any], pending = projection["pending"] as! [String: Any]
                check(pending["env"] == nil && pending["ordinary"] != nil, "Blocked projection filters quarantined native ID while preserving ordinary respond")
            }
        }
        run("retirement is exact and Stop retains local inventory") {
            let prefs = QuestionPreferences(root()), value = try prepare(prefs)
            let meta = ((value[T3QuestionCommands.metadataKey] as! [String: Any])["commands"] as! [String: [String: Any]])["command"]!
            let old = meta["requestIncarnation"] as! String
            let attempt = try issued(prefs, value, rpc: "1-1"), settled = try prefs.owner.complete(command: attempt.1, attempt: attempt.2, result: ["ok": true])
            refuses("Generic successful RPC does not prove the pending request consumed") { _ = try prefs.call("remove", marker(settled)) }
            _ = try clean(prefs, settled)
            let scope = try T3QuestionScope(origin: connection.home, environment: "env", thread: "thread", request: "request")
            let successor = try prefs.attachments.register(scope, questions: ["new": "later-signature"])
            check(successor != old, "Consumed native request permits a reused request ID with a changed catalog")
            let admission = try prefs.attachments.admit(request: successor, question: "new", signature: "later-signature", generation: 1, route: "route", mount: UUID(), fileLimit: 1024)
            let operation = try prefs.attachments.open(admission, kind: "files"), slot = try prefs.attachments.reserve(operation, ordinal: 0)
            try prefs.attachments.stage(slot, metadata: .init(kind: .file, name: "later.txt", mimeType: "text/plain"), produce: { Data("successor".utf8) })
            let items = try prefs.attachments.adopt(operation)
            check(items.count == 1, "Successor intake remains available after exact old request retirement")
            let stopPrefs = QuestionPreferences(root()), stop = try prepare(stopPrefs, type: "run.interrupt")
            let stopped = try issued(stopPrefs, stop, rpc: "1-1"), completed = try stopPrefs.owner.complete(command: stopped.1, attempt: stopped.2, result: ["ok": true])
            _ = try clean(stopPrefs, completed)
            let retained = try stopPrefs.attachments.register(scope, questions: ["q": "signature"])
            let stopMeta = ((stop[T3QuestionCommands.metadataKey] as! [String: Any])["commands"] as! [String: [String: Any]])["command"]!
            check(retained == stopMeta["requestIncarnation"] as? String, "Confirmed Stop never retires the still-pending native question request")
        }
        run("older uncertainty survives a later pre-handler refusal") {
            let prefs = QuestionPreferences(root()), first = try issued(prefs, prepare(prefs), rpc: "1-1")
            let uncertain = try prefs.owner.complete(command: first.1, attempt: first.2, result: ["ok": false])
            let second = try issued(prefs, uncertain, rpc: "1-9")
            let result = try prefs.owner.complete(command: second.1, attempt: second.2, result: ["ok": false, "_outboxNotAccepted": true])
            check(((result[T3QuestionCommands.recordsKey] as! [String: [String: Any]])["command"]?["phase"] as? String) == "uncertain",
                "A later proven nonacceptance cannot erase an unknown earlier command outcome")
        }

        run("bounded obsolete epochs and mutations") {
            let location = root()
            for _ in 0..<6 { try launch("child-terminal", location) }
            let prefs = QuestionPreferences(location); _ = try prefs.call("read")
            let state = try prefs.call("read"), meta = state[T3QuestionCommands.metadataKey] as! [String: Any]
            check((meta["tombstones"] as! [String: Any]).isEmpty, "Repeated actual restart collects only acknowledged prior-sealed tombstones")
            check((meta["seals"] as! [String: Any]).count <= 3, "Obsolete seals compact after exact pin/tombstone/mutation dependencies end")
            check((meta["mutations"] as! [String: Any]).count <= 12, "Obsolete sealed mutation identities compact without indefinite epoch growth")
        }
        let report: [String: Any] = ["checks": count, "failures": failures, "method": "Production command helper and production POSIX filesystem. Actual child process restart. UIKit module/transport execution is separate."]
        print(String(decoding: try JSONSerialization.data(withJSONObject: report, options: [.sortedKeys, .prettyPrinted]), as: UTF8.self))
        if !failures.isEmpty { throw T3QuestionCommandFailure(message: "\(failures.count) checks failed") }
    }
}
