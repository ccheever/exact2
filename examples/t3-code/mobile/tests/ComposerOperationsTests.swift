import Foundation

@main struct ComposerOperationsTests {
    static func main() throws {
        var checks = 0
        func check(_ value: Bool, _ name: String) { checks += 1; if !value { fatalError(name) } }
        func refuses(_ object: [String: Any]) {
            do { _ = try T3ComposerApplyRequest(object); check(false, "Malformed operation admitted") }
            catch let error as T3ComposerOperationFailure { check(error.kind == "arguments", "Arguments failure classified") }
            catch { check(false, "Unexpected refusal type") }
        }
        let identity = T3ComposerIdentity(owner: "o", editorId: "e", routeVisit: "r", renderEpoch: "g", mountId: "m")
        let command: [String: Any] = ["commandId": "c", "commandRevision": 1,
            "expected": ["eventCount": 1, "value": "🙂 /mo", "selection": ["start": 6, "end": 6]],
            "next": ["value": "🙂 /model ", "selection": ["start": 10, "end": 10], "tokensJson": "[]"]]
        let object: [String: Any] = ["op": "composerEditorApply", "generation": 1, "identity": identity.json, "command": command]
        let decoded = try T3ComposerApplyRequest(object)
        check(decoded.identity == identity && decoded.command.expected.selection.start == 6, "Exact mounted identity and UTF16 expected range decoded")
        check(decoded.command.sameRequest(try T3ComposerApplyRequest(object).command), "Exact request replay comparison")
        for key in ["owner", "editorId", "routeVisit", "renderEpoch", "mountId"] {
            var bad = object, missing = identity.json; missing[key] = ""; bad["identity"] = missing; refuses(bad)
        }
        for generation in [-1, 1.5, true, "1", NSNull(), T3ComposerProtocolState.maxCount + 1] as [Any] {
            var bad = object; bad["generation"] = generation; refuses(bad)
        }
        for key in ["op", "identity", "command", "generation"] { var bad = object; bad.removeValue(forKey: key); refuses(bad) }
        var wrong = object; wrong["op"] = "other"; refuses(wrong)
        for key in ["commandId", "commandRevision", "expected", "next"] {
            var bad = object, missing = command; missing.removeValue(forKey: key); bad["command"] = missing; refuses(bad)
        }
        check(try T3ComposerVoiceCapture.revision(nil) == nil, "Absent source revision remains distinguishable")
        for value in [0, 17, T3ComposerProtocolState.maxCount] { check(try T3ComposerVoiceCapture.revision(value) == value, "Valid explicit source revision") }
        for value in [-1, 1.5, true, "1", NSNull(), T3ComposerProtocolState.maxCount + 1] as [Any] {
            do { _ = try T3ComposerVoiceCapture.revision(value); check(false, "Invalid voice revision accepted") }
            catch { check(true, "Invalid voice revision refused") }
        }
        try finalSources(check)
        try handoffs(check)
        print("Composer operations: \(checks) checks passed")
    }

    private static func finalSources(_ check: (Bool, String) -> Void) throws {
        func owner(_ fields: [Any]) throws -> String { String(decoding: try JSONSerialization.data(withJSONObject: fields), as: UTF8.self) }
        func identity(_ key: String, mount: String = UUID().uuidString, generation: Int = 3) throws -> T3ComposerIdentity {
            .init(owner: try owner(["ordinary", "https://t3.test", "environment", generation, key, "birth"]), editorId: "editor", routeVisit: "visit", renderEpoch: "epoch", mountId: mount)
        }
        let captured = try identity("new-task:one"), ledger = T3ComposerFinalizations()
        check(captured.independentNewTask, "Independent draft owner includes its incarnation")
        for fields in [["ordinary", "https://t3.test", "environment", 3, "environment:thread"],
                       ["queued-edit", "edit"], ["ordinary", "https://t3.test", "environment", 3, "new-task:pending-one", "birth"],
                       ["ordinary", "https://t3.test", "environment", 3, "new-task:one", ""],
                       ["ordinary", "https://t3.test", "environment", true, "new-task:one", "birth"],
                       ["ordinary", "https://t3.test", "environment", 3, "new-task:one", "birth", "extra"]] as [[Any]] {
            let foreign = T3ComposerIdentity(owner: try owner(fields), editorId: "e", routeVisit: "r", renderEpoch: "g", mountId: "m")
            check(!foreign.independentNewTask, "Foreign, pending and malformed owners stay outside independent finalization")
            try ledger.reserve(foreign)
        }
        try ledger.reserve(captured); try ledger.reserve(captured)
        check(ledger.pending.isEmpty, "Mount reservation is replayable and not a final source")
        var state = T3ComposerProtocolState(identity: captured, mountId: captured.mountId)
        let text = "🙂 @file final", snapshot = T3ComposerSnapshot(value: text, selection: .init(start: 3, end: 8), composing: true, focused: true)
        let finalEvent = state.event("retired", snapshot: snapshot, extra: ["pendingCommand": ["commandId": "retained-command"]])
        check(ledger.retire(captured, event: finalEvent), "Actual final source replaces its reserved slot")
        let receipt = ledger.pending[0]
        check(!ledger.retire(captured, event: ["value": "older"]), "Duplicate retirement cannot overwrite final source")
        check(receipt.event["value"] as? String == text && (receipt.event["selection"] as? [String: Int])?["end"] == 8
              && receipt.event["composing"] as? Bool == true && (receipt.event["pendingCommand"] as? [String: String])?["commandId"] == "retained-command",
              "Retained source preserves UTF16 selection, composition and unsettled command")
        check(ledger.pending[0].id == receipt.id && ledger.pending[0].event as NSDictionary == finalEvent as NSDictionary,
              "Reading without save acknowledgment leaves exact final source retryable")
        do { try ledger.reserve(try identity("new-task:one")); check(false, "Unpersisted draft remounted") }
        catch { check((error as? T3ComposerOperationFailure)?.kind == "busy", "Same draft stays blocked until its source is saved") }
        do { try ledger.reserve(try identity("new-task:one", generation: 4)); check(false, "Reconnect bypassed unsaved source") }
        catch { check((error as? T3ComposerOperationFailure)?.kind == "busy", "Transport generation cannot bypass stable draft reservation") }
        for index in 1..<T3ComposerFinalizations.capacity { try ledger.reserve(try identity("new-task:other-\(index)")) }
        let overflow = try identity("new-task:overflow")
        do { try ledger.reserve(overflow); check(false, "Unbounded editors admitted") }
        catch { check((error as? T3ComposerOperationFailure)?.kind == "busy" && ledger.pending[0].id == receipt.id, "Capacity refuses admission without evicting unsaved text") }
        check(!ledger.acknowledge(UUID().uuidString) && ledger.pending.count == 1, "Unknown acknowledgment removes no source")
        check(ledger.acknowledge(receipt.id) && !ledger.acknowledge(receipt.id), "Save acknowledgment is exact and replayable")
        try ledger.reserve(overflow)
        check(!ledger.retire(overflow, event: nil), "Unacknowledged seed releases its reservation without a save")
        try ledger.reserve(try identity("new-task:one"))
        ledger.destroy(); check(ledger.pending.isEmpty, "Module shutdown clears process-local receipts")

        let read: [String: Any] = ["op": "composerEditorFinalizations", "generation": 4, "action": "read"]
        check(try T3ComposerFinalizationRequest(read).action == .read, "Read operation has strict generation and action")
        var ack = read; ack["action"] = "acknowledge"; ack["receiptId"] = receipt.id
        check(try T3ComposerFinalizationRequest(ack).receiptId == receipt.id, "Acknowledgment names the opaque final source receipt")
        for replacement in [["generation": true], ["generation": -1], ["action": "other"], ["action": "acknowledge"], ["receiptId": receipt.id]] as [[String: Any]] {
            let bad = read.merging(replacement) { _, value in value }
            do { _ = try T3ComposerFinalizationRequest(bad); check(false, "Malformed final source request admitted") }
            catch { check((error as? T3ComposerOperationFailure)?.kind == "arguments", "Malformed final source request is refused") }
        }
    }

    private static func handoffs(_ check: (Bool, String) -> Void) throws {
        let owner = String(decoding: try JSONSerialization.data(withJSONObject: ["ordinary", "https://t3.test", "environment", 3, "new-task:held", "birth"]), as: UTF8.self)
        let identity = T3ComposerIdentity(owner: owner, editorId: "editor", routeVisit: "visit", renderEpoch: "epoch", mountId: "mount")
        let operationId = UUID().uuidString
        let freeze: [String: Any] = ["op": "composerEditorHandoff", "generation": 3, "identity": identity.json,
            "action": "freeze", "operationId": operationId, "operationRevision": 1, "retireUndo": false]
        let request = try T3ComposerHandoffRequest(freeze)
        if case .freeze(let retireUndo) = request.action { check(!retireUndo, "Freeze carries explicit Undo retirement intent") }
        else { check(false, "Freeze decoded as another action") }
        let ledger = T3ComposerFinalizations(); try ledger.reserve(identity)
        let receipt = try ledger.freeze(identity, event: ["value": "🙂 held", "eventCount": 2])
        check(!receipt.retired && ledger.pending[0].id == receipt.id, "Frozen native source is readable before teardown")
        do { try ledger.resume(identity, receiptId: receipt.id); check(false, "Unsaved frozen source resumed") }
        catch { check((error as? T3ComposerOperationFailure)?.kind == "busy", "Resume requires explicit save acknowledgment") }
        check(ledger.acknowledge(receipt.id) && ledger.pending.isEmpty, "Save acknowledgment clears pending source while editor remains reserved")
        var reconnect = identity.json
        reconnect["owner"] = String(decoding: try JSONSerialization.data(withJSONObject: ["ordinary", "https://t3.test", "environment", 4, "new-task:held", "birth"]), as: UTF8.self)
        reconnect["mountId"] = "new-mount"
        let reconnected = try JSONDecoder().decode(T3ComposerIdentity.self, from: JSONSerialization.data(withJSONObject: reconnect))
        do { try ledger.reserve(reconnected); check(false, "Saved but still held editor replaced") }
        catch { check((error as? T3ComposerOperationFailure)?.kind == "busy", "Acknowledged held editor still excludes a second writer after reconnect") }
        do { try ledger.resume(identity, receiptId: UUID().uuidString); check(false, "Foreign receipt resumed") }
        catch { check((error as? T3ComposerOperationFailure)?.kind == "busy", "Resume names the exact saved receipt") }
        try ledger.resume(identity, receiptId: receipt.id)
        check(ledger.retire(identity, event: ["value": "new producer source"]), "Resumed writer captures its newer final source")
        check(ledger.pending[0].event["value"] as? String == "new producer source" && ledger.pending[0].retired, "Old held source cannot replace the resumed final source")
        _ = ledger.acknowledge(ledger.pending[0].id); try ledger.reserve(identity)
        let detached = try ledger.freeze(identity, event: ["value": "held before navigation"])
        check(ledger.retire(identity, event: ["value": "stale teardown"]), "Unsaved frozen source survives native destruction")
        check(ledger.pending[0].id == detached.id && ledger.pending[0].event["value"] as? String == "held before navigation" && ledger.pending[0].retired,
              "Detached receipt preserves the frozen event and marks retirement")
        _ = ledger.acknowledge(detached.id); try ledger.reserve(identity)
        let saved = try ledger.freeze(identity, event: ["value": "already saved"]); _ = ledger.acknowledge(saved.id)
        check(!ledger.retire(identity, event: ["value": "older native copy"]) && ledger.pending.isEmpty, "Destroying an acknowledged held editor cannot resave stale bytes after a producer")
        try ledger.reserve(reconnected)

        let document: [String: Any] = ["value": "🙂 next", "selection": ["start": 7, "end": 7], "tokensJson": "[]", "isNativeEcho": false]
        var resume = freeze; resume.removeValue(forKey: "retireUndo")
        resume["action"] = "resume"; resume["receiptId"] = saved.id; resume["document"] = document
        if case .resume(let id, let value) = try T3ComposerHandoffRequest(resume).action {
            check(id == saved.id && value.selection?.start == 7, "Resume decodes an explicit source document and UTF16 caret")
        } else { check(false, "Resume decoded as another action") }
        for replacement in [["generation": true], ["operationRevision": true], ["operationRevision": -1], ["operationId": "not-a-uuid"],
                            ["retireUndo": "false"], ["document": document], ["receiptId": saved.id], ["action": "other"]] as [[String: Any]] {
            let bad = freeze.merging(replacement) { _, value in value }
            do { _ = try T3ComposerHandoffRequest(bad); check(false, "Invalid freeze request admitted") }
            catch { check((error as? T3ComposerOperationFailure)?.kind == "arguments", "Invalid freeze request refused") }
        }
        for replacement in [["retireUndo": false], ["receiptId": ""], ["document": document.merging(["isNativeEcho": true]) { _, value in value }],
                            ["document": document.merging(["selection": NSNull()]) { _, value in value }],
                            ["document": document.merging(["tokensJson": "[{}]"]) { _, value in value }]] as [[String: Any]] {
            let bad = resume.merging(replacement) { _, value in value }
            do { _ = try T3ComposerHandoffRequest(bad); check(false, "Invalid resume request admitted") }
            catch { check((error as? T3ComposerOperationFailure)?.kind == "arguments", "Invalid resume request refused") }
        }
        var ordinary = freeze; ordinary["identity"] = T3ComposerIdentity(owner: "ordinary-thread", editorId: "e", routeVisit: "v", renderEpoch: "g", mountId: "m").json
        do { _ = try T3ComposerHandoffRequest(ordinary); check(false, "Thread editor entered NewTask handoff") }
        catch { check((error as? T3ComposerOperationFailure)?.kind == "arguments", "Ordinary editors stay outside NewTask handoff") }
    }
}
