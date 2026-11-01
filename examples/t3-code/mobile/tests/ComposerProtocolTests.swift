// Exact Foundation protocol body; no network, UIKit or device required.
import Foundation

@main struct ComposerProtocolTests {
    static func main() throws {
        var checks = 0
        func check(_ condition: Bool, _ why: String) { checks += 1; if !condition { fatalError(why) } }
        func rejected(_ why: String, _ body: () throws -> Void) {
            do { try body(); check(false, why) } catch { check(true, why) }
        }
        func json(_ value: [String: Any]) throws -> String { String(decoding: try JSONSerialization.data(withJSONObject: value), as: UTF8.self) }
        let source = "🙂 /mo tail", selection = T3ComposerSelection(start: 6, end: 6)
        var object: [String: Any] = ["owner": "thread:A", "editorId": "editor:1", "routeVisit": "visit:1", "renderEpoch": "render:1", "mountId": "",
            "acknowledgedEventCount": 0, "ackCommandId": "", "active": true, "editable": true, "readOnly": false,
            "focusIntent": ["serial": "", "attempt": 0, "operation": "none"], "command": NSNull(),
            "document": ["value": source, "selection": ["start": selection.start, "end": selection.end], "tokensJson": "[]", "isNativeEcho": false]]
        let initial = try T3ComposerControl(json: json(object))
        var state = T3ComposerProtocolState(identity: initial.identity, mountId: "mount:1")
        var snapshot = T3ComposerSnapshot(value: source, selection: selection, composing: false, focused: true)
        let ready = state.event("ready", snapshot: snapshot)
        check(ready["mountId"] as? String == "mount:1" && state.count == 1, "ready captures incarnation/count")
        check(!state.acknowledge(initial), "empty initial mount never acknowledges")
        object["mountId"] = "mount:1"; object["acknowledgedEventCount"] = 1
        check(state.acknowledge(try T3ComposerControl(json: json(object))), "exact ready acknowledgment")
        for field in ["owner", "editorId", "routeVisit", "renderEpoch", "mountId"] {
            var stale = object; stale[field] = "stale"
            check(!state.acknowledge(try T3ComposerControl(json: json(stale))), "identity fences " + field)
        }
        func command(_ id: String, _ revision: Int, _ count: Int, _ suppliedValue: String? = nil, _ suppliedRange: T3ComposerSelection? = nil) throws -> T3ComposerReplace {
            let value = suppliedValue ?? source, range = suppliedRange ?? selection
            let input: [String: Any] = ["commandId": id, "commandRevision": revision,
                "expected": ["eventCount": count, "value": value, "selection": ["start": range.start, "end": range.end]],
                "next": ["value": "🙂 /model  tail", "selection": ["start": 10, "end": 10], "tokensJson": "[]"]]
            let result = try JSONDecoder().decode(T3ComposerReplace.self, from: Data(json(input).utf8)); try result.validate(); return result
        }
        let first = try command("c1", 1, 1)
        check(state.replacementRefusal(first, snapshot: snapshot, active: true) == nil, "current CAS accepted")
        check(state.replacementRefusal(first, snapshot: snapshot, active: false) != nil, "inactive CAS refused")
        check(state.replacementRefusal(first, snapshot: .init(value: source, selection: selection, composing: true, focused: true), active: true) != nil, "marked-text CAS refused")
        check(state.replacementRefusal(try command("old", 2, 0), snapshot: snapshot, active: true) != nil, "old event count refused")
        check(state.replacementRefusal(first, snapshot: .init(value: source, selection: .init(start: 0, end: 0), composing: false, focused: true), active: true) != nil, "moved caret refused")
        snapshot = .init(value: first.next.value, selection: first.next.selection, composing: false, focused: true)
        let terminal = state.finish(first, applied: true, reason: "", snapshot: snapshot)!
        check(terminal["kind"] as? String == "commandApplied" && state.count == 2, "CAS emits one terminal")
        check(state.terminalCount > first.expected.eventCount, "terminal count follows expected count")
        check(state.finish(first, applied: true, reason: "", snapshot: snapshot) == nil && state.count == 2, "duplicate cannot apply twice")
        let typed = T3ComposerSnapshot(value: "🙂 /model X tail", selection: .init(start: 11, end: 11), composing: true, focused: true)
        let latest = state.event("text", snapshot: typed)
        check(latest["value"] as? String == typed.value, "normal typing remains current during terminal wait")
        let pending = latest["pendingCommand"] as! [String: Any]
        check(pending["value"] as? String == first.next.value && pending["eventCount"] as? Int == 2, "immutable earlier terminal retained")
        check(latest["eventCount"] as! Int > pending["eventCount"] as! Int, "enclosing event strictly newer")
        object["acknowledgedEventCount"] = 3; object["ackCommandId"] = "wrong"
        check(state.acknowledge(try T3ComposerControl(json: json(object))) && state.terminalID == "c1", "wrong terminal ack retained")
        object["ackCommandId"] = "c1"
        check(state.acknowledge(try T3ComposerControl(json: json(object))) && state.terminalID.isEmpty, "matching terminal ack releases CAS only")
        let second = try command("c2", 2, 3, typed.value, typed.selection)
        check(state.replacementRefusal(second, snapshot: typed, active: true) != nil, "IME rejection while text remains editable")
        _ = state.finish(second, applied: false, reason: "Text composition is active.", snapshot: typed)
        let unmarked = state.event("selection", snapshot: .init(value: typed.value, selection: typed.selection, composing: false, focused: true))
        check(unmarked["composing"] as? Bool == false && state.terminalID == "c2", "same-value composition end flows with retained rejection")
        object["acknowledgedEventCount"] = state.count; object["ackCommandId"] = "c2"
        _ = state.acknowledge(try T3ComposerControl(json: json(object)))
        check(state.replacementRefusal(try command("c1", 3, state.count, typed.value, typed.selection), snapshot: typed, active: true) != nil, "c1 c2 c1 reuse refused")
        check(state.finish(try command("c1", 3, state.count), applied: false, reason: "", snapshot: typed) == nil, "reused ID no second terminal")
        let future = try command("future", 4, state.count + 1)
        check(state.finish(future, applied: false, reason: "", snapshot: typed) == nil, "future expected count cannot create malformed terminal")
        for bounds in [(-1, 0), (2, 1), (0, 999)] {
            var bad = object; bad["document"] = ["value": source, "selection": ["start": bounds.0, "end": bounds.1], "tokensJson": "[]", "isNativeEcho": false]
            rejected("bad selection \(bounds)") { _ = try T3ComposerControl(json: json(bad)) }
        }
        for badCount in [-1, 1.5, true, "1", T3ComposerProtocolState.maxCount + 1] as [Any] {
            var bad = object; bad["acknowledgedEventCount"] = badCount
            rejected("invalid acknowledgment count") { _ = try T3ComposerControl(json: json(bad)) }
        }
        try T3ComposerTokenValidator.validate("[{\"type\":\"skill\",\"source\":\"/mo\",\"label\":\"mo\",\"start\":3,\"end\":6}]", value: source)
        check(true, "UTF16 token after emoji")
        for tokens in ["[{\"type\":\"skill\",\"source\":\"wrong\",\"label\":\"x\",\"start\":3,\"end\":6}]",
                       "[{\"type\":\"skill\",\"source\":\"/mo\",\"label\":\"x\",\"start\":3,\"end\":6},{\"type\":\"skill\",\"source\":\"/mo\",\"label\":\"x\",\"start\":3,\"end\":6}]"] {
            rejected("mismatched/overlapping rich tokens") { try T3ComposerTokenValidator.validate(tokens, value: source) }
        }
        print("Composer protocol: \(checks) checks passed")
    }
}
