#if os(iOS)
// Runs against the actual app-native bodies in an isolated UIKit application.
import UIKit

final class ComposerEditorUIKitTests {
    private(set) var checks: [[String: Any]] = []
    private let host: UIView
    private let voice = T3MobileVoiceEditor()
    private var instances: [T3MobileComposerEditor] = []
    private var events: [[String: Any]] = []
    init(host: UIView) { self.host = host }
    private func check(_ value: Bool, _ name: String) {
        checks.append(["name": name, "pass": value])
        let file = FileManager.default.urls(for: .documentDirectory, in: .userDomainMask)[0].appendingPathComponent("partial-checks.json")
        if let bytes = try? JSONSerialization.data(withJSONObject: checks, options: [.prettyPrinted]) { try? bytes.write(to: file) }
    }
    private func json(_ object: Any) throws -> String { String(decoding: try JSONSerialization.data(withJSONObject: object), as: UTF8.self) }
    private func event(_ kind: String) -> [String: Any]? { events.last(where: { $0["kind"] as? String == kind }) }
    private func makePort(_ source: String, selection: Int, tokens: String = "[]", epoch: String = UUID().uuidString) throws -> (T3MobileComposerEditor, [String: Any]) {
        let emitter = ExactNativeEvents(fn: { context, _, _, bytes, length in
            guard let context, let bytes else { return }
            let owner = Unmanaged<ComposerEditorUIKitTests>.fromOpaque(context).takeUnretainedValue()
            if let object = try? JSONSerialization.jsonObject(with: Data(bytes: bytes, count: Int(length))) as? [String: Any] { owner.events.append(object) }
        }, ctx: Unmanaged.passUnretained(self).toOpaque(), nonce: 1)
        let port = T3MobileComposerEditor(voice: voice, events: emitter)
        instances.append(port)
        port.view.frame = CGRect(x: 20, y: 180, width: 600, height: 240)
        host.addSubview(port.view)
        let control: [String: Any] = ["owner": "fixture:" + epoch, "editorId": "composer:" + epoch,
            "routeVisit": "route:" + epoch, "renderEpoch": epoch, "mountId": "",
            "acknowledgedEventCount": 0, "ackCommandId": "", "active": true, "editable": true, "readOnly": false,
            "focusIntent": ["serial": "", "attempt": 0, "operation": "none"], "command": NSNull(),
            "document": ["value": source, "selection": ["start": selection, "end": selection], "tokensJson": tokens, "isNativeEcho": false],
            "presentation": ["placeholder": "Fixture composer", "fontSize": 16, "lineHeight": 22]]
        try apply(port, control)
        port.view.setNeedsLayout(); port.view.layoutIfNeeded()
        return (port, control)
    }
    private func editor(_ port: T3MobileComposerEditor) -> T3MobileOwnedComposerView { port.view.subviews.compactMap { $0 as? T3MobileOwnedComposerView }.first! }
    private func apply(_ port: T3MobileComposerEditor, _ control: [String: Any]) throws { try port.setProps(["configuration": json(control)]) }
    private func acknowledge(_ port: T3MobileComposerEditor, _ control: inout [String: Any], terminal: String = "") throws {
        control["mountId"] = port.composerIdentity!.mountId
        control["acknowledgedEventCount"] = port.composerEventCount
        control["ackCommandId"] = terminal
        if let snapshot = port.composerSnapshot {
            var document = control["document"] as! [String: Any]
            document["value"] = snapshot.value
            document["selection"] = ["start": snapshot.selection.start, "end": snapshot.selection.end]
            document["isNativeEcho"] = true
            control["document"] = document
        }
        try apply(port, control)
    }
    private func command(_ port: T3MobileComposerEditor, id: String, revision: Int, value: String, caret: Int) -> [String: Any] {
        let current = port.composerSnapshot!
        return ["commandId": id, "commandRevision": revision,
            "expected": ["eventCount": port.composerEventCount, "value": current.value,
                "selection": ["start": current.selection.start, "end": current.selection.end]],
            "next": ["value": value, "selection": ["start": caret, "end": caret], "tokensJson": "[]"]]
    }
    @MainActor func run() async -> [String: Any] {
        for (name, body) in [("rich mapping", readyAndRichMapping), ("command/composition", commandAndComposition), ("owner/voice", ownerAndVoice)] {
            do { try body() } catch { check(false, "\(name) threw: \(error)") }
        }
        do { try await pasteCancellation() } catch { check(false, "paste cancellation threw: \(error)") }
        for port in instances { port.destroy(); port.view.removeFromSuperview() }
        voice.destroy()
        return ["checks": checks, "events": events, "passed": checks.filter { $0["pass"] as? Bool == true }.count,
            "failed": checks.filter { $0["pass"] as? Bool == false }.count,
            "pid": ProcessInfo.processInfo.processIdentifier, "platform": UIDevice.current.systemVersion,
            "method": "Actual production Swift bodies; public UIKit in isolated simulator app. No framework runner/root integration."]
    }
    private func readyAndRichMapping() throws {
        let source = "🙂 @file tail"
        let tokens = try json([["type": "file", "source": "@file", "label": "file", "start": 3, "end": 8]])
        var (port, control) = try makePort(source, selection: 8, tokens: tokens)
        let view = editor(port), text = view.textView
        check(port.composerEventCount == 1 && event("ready")?["value"] as? String == source, "Ready captures initialized source")
        check(!text.isEditable && !view.isUserInteractionEnabled, "Ready gates editing before exact echo")
        check(text.textStorage.length == 9 && text.textStorage.attribute(.attachment, at: 3, effectiveRange: nil) is ComposerTextAttachment, "Rich source becomes one actual UIKit attachment")
        check(port.composerSnapshot?.selection.start == 8 && text.selectedRange.location == 4, "Source UTF16 caret maps after chip")
        try acknowledge(port, &control)
        check(text.isEditable && view.isUserInteractionEnabled, "Ready echo enables editor")
        control["focusIntent"] = ["serial": "fixture-focus", "attempt": 0, "operation": "focus"]
        try apply(port, control)
        check(text.isFirstResponder && event("focus")?["mountId"] as? String == port.composerIdentity?.mountId, "Explicit active focus captures mount identity")
        text.selectedRange = NSRange(location: 2, length: 0)
        view.textViewDidChangeSelection(text)
        check(port.composerSnapshot?.selection.start == 2 && event("selection")?["selection"] as? [String: Int] == ["start": 2, "end": 2], "Pure caret move emits real UTF16 selection")
        text.textStorage.insert(NSAttributedString(string: "X"), at: 0)
        text.selectedRange = NSRange(location: 1, length: 0)
        view.textViewDidChange(text)
        let before = port.composerSnapshot
        view.setFontSize(18)
        check(port.composerSnapshot == before && text.textStorage.attribute(.attachment, at: 4, effectiveRange: nil) is ComposerTextAttachment, "Typing before chip then forced typography rebuild preserves source and caret")
        port.destroy(); port.view.removeFromSuperview()
    }
    private func commandAndComposition() throws {
        var (port, control) = try makePort("/mo tail", selection: 3)
        try acknowledge(port, &control)
        let view = editor(port), text = view.textView
        text.becomeFirstResponder()
        let expected = port.composerEventCount
        control["command"] = command(port, id: "command-1", revision: 1, value: "/model  tail", caret: 7)
        try apply(port, control)
        let applied = event("commandApplied")
        check(port.composerSnapshot?.value == "/model  tail" && port.composerSnapshot?.selection.start == 7, "CAS applies source and UTF16 selection atomically")
        check((applied?["eventCount"] as? Int ?? 0) > expected && applied?["commandId"] as? String == "command-1", "CAS emits terminal after expected count")
        let appliedCount = port.composerEventCount
        try apply(port, control)
        check(port.composerEventCount == appliedCount, "Retained terminal prevents duplicate CAS")
        text.insertText("X")
        view.textViewDidChange(text)
        check(text.isEditable && event("text")?["pendingCommand"] as? NSDictionary == applied?["pendingCommand"] as? NSDictionary, "Typing remains enabled and retains immutable terminal")
        let newer = port.composerSnapshot!.value
        control["command"] = NSNull()
        try acknowledge(port, &control, terminal: "command-1")
        check(port.composerSnapshot?.value == newer, "Terminal ACK never restores older terminal value")
        let stale = command(port, id: "command-ime", revision: 2, value: "must not replace", caret: 0)
        text.setMarkedText("に", selectedRange: NSRange(location: 1, length: 0))
        check(text.markedTextRange != nil && port.composerSnapshot?.composing == true, "Actual UITextView marked text observed")
        control["command"] = stale
        let markedValue = port.composerSnapshot!.value
        try apply(port, control)
        check(event("commandRejected")?["commandId"] as? String == "command-ime" && text.isEditable && text.markedTextRange != nil, "Rejected CAS leaves UIKit composition editable and marked")
        view.setFontSize(23)
        check(text.markedTextRange != nil && port.composerSnapshot?.value == markedValue, "Typography rebuild deferred during marked text")
        text.unmarkText()
        check(port.composerSnapshot?.composing == false && event("selection")?["composing"] as? Bool == false, "Unmark emits composing=false without requiring text mutation")
        check(event("selection")?["pendingCommand"] as? [String: Any] != nil, "Composition completion carries unacknowledged terminal")
        control["command"] = NSNull(); try acknowledge(port, &control, terminal: "command-ime")
        control["editable"] = false; try apply(port, control)
        // UIKit may legitimately emit blur when editability changes. Only the
        // already-dispatched submit below must leave the post-update count alone.
        let count = port.composerEventCount
        view.onComposerSubmit(["alternate": true])
        check(port.composerEventCount == count, "Already-dispatched submit rejected when editable=false")
        port.destroy(); port.view.removeFromSuperview()
    }
    private func ownerAndVoice() throws {
        var (port, control) = try makePort("🙂 middle", selection: 3)
        try acknowledge(port, &control)
        let view = editor(port), identity = port.composerIdentity!, owner = control["owner"] as! String
        let count = port.composerEventCount
        let selection = try voice.selection(owner: owner, text: "🙂 middle")
        check(selection["start"] as? Int == 3 && selection["mountId"] as? String == identity.mountId, "Voice capture contains actual source selection and incarnation")
        check(voice.stageCaptured(owner: owner, identity: identity, eventCount: count, text: "🙂 middle", start: 2, end: 2, revision: 1), "Voice stage accepts exact invocation capture")
        voice.refresh(port, owner: owner, selectionRevision: 1)
        check(port.composerSnapshot?.selection.start == 2, "Voice receipt restores source caret")
        check(!voice.stageCaptured(owner: owner, identity: identity, eventCount: count, text: "🙂 middle", start: 0, end: 0, revision: 2), "Stale voice capture refused after selection event")
        view.textView.becomeFirstResponder()
        view.applyReplacement(value: "replacement", selection: .init(start: 4, end: 4), tokensJson: "[]")
        let oldUndo = view.textView.undoManager
        check(oldUndo?.canUndo == true, "CAS replacement registers actual UIKit undo")
        control["renderEpoch"] = "next-admission"; control["mountId"] = ""; control["acknowledgedEventCount"] = 0
        control["document"] = ["value": "new owner", "selection": ["start": 0, "end": 0], "tokensJson": "[]", "isNativeEcho": false]
        try apply(port, control)
        check(editor(port) !== view && oldUndo?.canUndo != true, "Admission change replaces UITextView and clears old undo")
        let before = port.composerEventCount
        view.onComposerChange([:]); view.onComposerSubmit([:])
        check(port.composerEventCount == before && port.composerSnapshot?.value == "new owner", "Old native callbacks cannot write a new admission")
        check(!port.applyVoiceSelection(identity: identity, expectedText: "new owner", start: 2, end: 2, revision: 9), "Old mount voice receipt cannot target replacement owner")
        port.destroy(); port.view.removeFromSuperview()
    }
    @MainActor private func pasteCancellation() async throws {
        var (port, control) = try makePort("paste", selection: 5)
        try acknowledge(port, &control)
        let text = editor(port).textView
        var supply: ((UIImage?, Error?) -> Void)?
        let provider = NSItemProvider()
        provider.registerObject(ofClass: UIImage.self, visibility: .all) { completion in
            supply = completion
            return Progress(totalUnitCount: 1)
        }
        text.loadImages(from: [provider])
        for _ in 0..<20 where supply == nil { try await Task.sleep(nanoseconds: 10_000_000) }
        check(supply != nil, "Real NSItemProvider image load is pending")
        control["active"] = false; try apply(port, control)
        let prior = events.filter { $0["kind"] as? String == "pasteImages" }.count
        let image = UIGraphicsImageRenderer(size: CGSize(width: 2, height: 2)).image { context in UIColor.red.setFill(); context.fill(CGRect(x: 0, y: 0, width: 2, height: 2)) }
        supply?(image, nil)
        try await Task.sleep(nanoseconds: 50_000_000)
        check(events.filter { $0["kind"] as? String == "pasteImages" }.count == prior, "Inactive admission cancels pending provider paste")
        port.destroy(); port.view.removeFromSuperview()
    }
}
#endif
