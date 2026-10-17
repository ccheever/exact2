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
        print("Composer operations: \(checks) checks passed")
    }
}
