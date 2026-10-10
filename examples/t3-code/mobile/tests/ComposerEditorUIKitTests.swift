#if os(iOS)
// Runs against the actual app-native bodies in an isolated UIKit application.
import UIKit

final class ComposerEditorUIKitTests {
    private(set) var checks: [[String: Any]] = []
    private let host: UIView
    private let voiceOwner = T3MobileVoice(agent: true, audioSession: T3MobileAudioSession(), changed: { _ in })
    private var voice: T3MobileVoiceEditor { voiceOwner.editor }
    private let operations = T3MobileComposerOperations()
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
    private func emitter() -> ExactNativeEvents {
        ExactNativeEvents(fn: { context, _, _, bytes, length in
            guard let context, let bytes else { return }
            let owner = Unmanaged<ComposerEditorUIKitTests>.fromOpaque(context).takeUnretainedValue()
            if let object = try? JSONSerialization.jsonObject(with: Data(bytes: bytes, count: Int(length))) as? [String: Any] { owner.events.append(object) }
        }, ctx: Unmanaged.passUnretained(self).toOpaque(), nonce: 1)
    }
    private func makePort(_ source: String, selection: Int, tokens: String = "[]", epoch: String = UUID().uuidString) throws -> (T3MobileComposerEditor, [String: Any]) {
        let port = T3MobileComposerEditor(voice: voice, operations: operations, events: emitter())
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
        for (name, body) in [("rich mapping", readyAndRichMapping), ("command/composition", commandAndComposition), ("owner/voice", ownerAndVoice),
                             ("native invocation", invocation), ("display scale", displayScale)] {
            do { try body() } catch { check(false, "\(name) threw: \(error)") }
        }
        do { try await pasteCancellation() } catch { check(false, "paste cancellation threw: \(error)") }
        do { try await voiceCapture() } catch { check(false, "voice capture threw: \(error)") }
        do { try await richObservation() } catch { check(false, "rich observation threw: \(error)") }
        do { try bundledIcons() } catch { check(false, "bundled icons threw: \(error)") }
        do { try registryLifetime() } catch { check(false, "registry lifetime threw: \(error)") }
        for port in instances { port.destroy(); port.view.removeFromSuperview() }
        voiceOwner.destroy(); operations.destroy()
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
        let selection = try voice.selection(owner: owner, text: "🙂 middle", sourceRevision: 1)
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

    private func invocation() throws {
        var (port, control) = try makePort("/mo tail", selection: 3)
        try acknowledge(port, &control)
        let text = editor(port).textView
        text.becomeFirstResponder()
        let request: [String: Any] = ["op": "composerEditorApply", "generation": 7, "identity": port.composerIdentity!.json,
            "command": command(port, id: "operation-1", revision: 1, value: "/model  tail", caret: 7)]
        let reply = operations.perform(request), value = reply["value"] as? [String: Any], terminal = value?["event"] as? [String: Any]
        check(reply["ok"] as? Bool == true && terminal?["kind"] as? String == "commandApplied", "Operation returns actual applied terminal synchronously")
        check(terminal as NSDictionary? == event("commandApplied") as NSDictionary?, "Operation reply and emitted terminal envelopes are identical")
        let count = port.composerEventCount, snapshot = port.composerSnapshot
        let replay = operations.perform(request)
        check(replay as NSDictionary == reply as NSDictionary && port.composerEventCount == count && port.composerSnapshot == snapshot, "Retained command replay returns immutable envelope without a second edit/event")
        var changed = request, changedCommand = request["command"] as! [String: Any]
        changedCommand["next"] = ["value": "different", "selection": ["start": 0, "end": 0], "tokensJson": "[]"]
        changed["command"] = changedCommand
        check((operations.perform(changed)["error"] as? [String: Any])?["kind"] as? String == "arguments", "Same command id cannot replay a different payload")
        var other = request
        other["command"] = command(port, id: "operation-2", revision: 2, value: "busy", caret: 0)
        check((operations.perform(other)["error"] as? [String: Any])?["kind"] as? String == "busy", "Different command waits for retained terminal acknowledgment")
        text.insertText("X"); editor(port).textViewDidChange(text)
        let typed = port.composerSnapshot!, typedCount = port.composerEventCount
        let afterTyping = operations.perform(request)
        check(afterTyping as NSDictionary == reply as NSDictionary && port.composerSnapshot == typed && port.composerEventCount == typedCount, "Replay after newer native typing neither rewinds text nor changes terminal")
        try acknowledge(port, &control, terminal: "operation-1")
        check((operations.perform(request)["error"] as? [String: Any])?["kind"] as? String == "superseded", "Already acknowledged command id cannot apply again")
        var wrong = request, wrongIdentity = port.composerIdentity!.json
        wrongIdentity["mountId"] = "retired"; wrong["identity"] = wrongIdentity
        check((operations.perform(wrong)["error"] as? [String: Any])?["kind"] as? String == "superseded", "Wrong mount cannot resolve through newer same owner")
        var ime = request
        ime["command"] = command(port, id: "operation-ime", revision: 3, value: "must not apply", caret: 0)
        text.setMarkedText("に", selectedRange: NSRange(location: 1, length: 0))
        let rejection = operations.perform(ime)
        let rejectionEvent = (rejection["value"] as? [String: Any])?["event"] as? [String: Any]
        check(rejection["ok"] as? Bool == true && rejectionEvent?["kind"] as? String == "commandRejected" && text.markedTextRange != nil && text.isEditable,
              "Invocation during composition returns terminal rejection and preserves marked text")
        text.unmarkText(); try acknowledge(port, &control, terminal: "operation-ime")
        var detached = request
        detached["command"] = command(port, id: "operation-detached", revision: 4, value: "detached", caret: 0)
        port.view.removeFromSuperview()
        check((operations.perform(detached)["error"] as? [String: Any])?["kind"] as? String == "superseded", "Detached port cannot be edited by invocation")
        host.addSubview(port.view); port.destroy()
        check((operations.perform(detached)["error"] as? [String: Any])?["kind"] as? String == "superseded", "Destroyed port unregisters its invocation identity")
        port.view.removeFromSuperview()
    }

    @MainActor private func voiceCapture() async throws {
        var (port, control) = try makePort("🙂 voice", selection: 3)
        try acknowledge(port, &control)
        let owner = control["owner"] as! String
        func invoke(_ action: String, _ fields: [String: Any]) async -> [String: Any] {
            var request = fields; request["action"] = action; request["generation"] = 5
            return await withCheckedContinuation { continuation in voiceOwner.perform(request) { continuation.resume(returning: $0) } }
        }
        let first = await invoke("selection", ["owner": owner, "text": "🙂 voice", "sourceRevision": 101])
        let firstCapture = (first["value"] as? [String: Any])?["capture"] as? [String: Any]
        editor(port).textView.selectedRange = NSRange(location: 2, length: 0)
        editor(port).textViewDidChangeSelection(editor(port).textView)
        let second = await invoke("selection", ["owner": owner, "text": "🙂 voice", "sourceRevision": 202])
        let secondCapture = (second["value"] as? [String: Any])?["capture"] as? [String: Any]
        check(firstCapture?["sourceRevision"] as? Int == 101 && secondCapture?["sourceRevision"] as? Int == 202,
              "Actual voice operation returns independent per-invocation source revision receipts")
        check((firstCapture?["eventCount"] as? Int ?? -1) < (secondCapture?["eventCount"] as? Int ?? -1)
              && (firstCapture?["identity"] as? [String: Any])?["mountId"] as? String == port.composerIdentity?.mountId,
              "Later same-owner capture cannot mutate prior count or identity")
        let noRevision = await invoke("selection", ["owner": owner, "text": "🙂 voice"])
        check((noRevision["error"] as? [String: Any])?["kind"] as? String == "arguments", "Rich voice selection refuses missing source revision")
        let badRevision = await invoke("selection", ["owner": owner, "text": "🙂 voice", "sourceRevision": true])
        check((badRevision["error"] as? [String: Any])?["kind"] as? String == "arguments", "Rich voice rejects boolean revision at actual operation boundary")
        let legacy = await invoke("selection-commit", ["owner": owner, "text": "🙂 voice", "start": 0, "end": 0, "revision": 1])
        check((legacy["error"] as? [String: Any])?["kind"] as? String == "superseded" && port.composerSnapshot?.selection.start == 2,
              "Legacy rich selection-commit refuses instead of reporting a false successful move")
        let plain = await invoke("selection-commit", ["owner": "unmigrated-plain", "text": "plain", "start": 1, "end": 1, "revision": 1])
        check(plain["ok"] as? Bool == true, "Plain textarea selection-commit remains compatible")
        control["active"] = false; try apply(port, control)
        let inactive = await invoke("selection", ["owner": owner, "text": "🙂 voice", "sourceRevision": 203])
        check((inactive["error"] as? [String: Any])?["kind"] as? String == "superseded", "Inactive same-owner port cannot provide a voice capture")
        port.destroy(); port.view.removeFromSuperview()
    }

    private func displayScale() {
        let facts = T3LayoutFacts(events: emitter())
        facts.view.frame = CGRect(x: 0, y: 0, width: 1, height: 1)
        host.addSubview(facts.view); facts.view.setNeedsLayout(); facts.view.layoutIfNeeded()
        let reported = events.last(where: { $0["displayScale"] != nil })?["displayScale"] as? Double
        check(reported == host.window.map { Double($0.screen.scale) } && (reported ?? 0) > 0,
              "Layout fact reports actual attached-window display scale")
        facts.destroy(); facts.view.removeFromSuperview()
    }

    private func registryLifetime() throws {
        weak var weakPort: T3MobileComposerEditor?
        do {
            let transient = T3MobileComposerEditor(voice: voice, operations: operations, events: emitter())
            operations.register(transient); weakPort = transient
        }
        check(weakPort == nil, "Operation registry does not retain a native editor")
        var (port, control) = try makePort("live", selection: 4)
        try acknowledge(port, &control)
        let request: [String: Any] = ["op": "composerEditorApply", "generation": 1, "identity": port.composerIdentity!.json,
            "command": command(port, id: "closed-session", revision: 1, value: "never", caret: 0)]
        operations.destroy()
        let result = operations.perform(request)
        check((result["error"] as? [String: Any])?["kind"] as? String == "superseded" && port.composerSnapshot?.value == "live",
              "Session registry destruction refuses invocation without changing a surviving port")
        port.destroy(); port.view.removeFromSuperview()
    }

    @MainActor private func richObservation() async throws {
        let pasteboard = UIPasteboard.general, savedItems = UIPasteboard.general.items
        defer { pasteboard.items = savedItems }
        var (port, control) = try makePort("🙂 /mo", selection: 6)
        try acknowledge(port, &control)
        let view = editor(port), text = view.textView
        text.becomeFirstResponder()
        let request: [String: Any] = ["op": "composerEditorApply", "generation": 1, "identity": port.composerIdentity!.json,
            "command": command(port, id: "rich-pending-command", revision: 1, value: "🙂 /model ", caret: 10)]
        let terminal = ((operations.perform(request)["value"] as? [String: Any])?["event"] as? [String: Any])!
        view.setTextPasteThresholdBytes(5)
        pasteboard.string = "intercepted text"
        let beforeText = port.composerEventCount
        text.paste(nil)
        let first = event("pasteText")!, firstObservation = first["editorEvent"] as? [String: Any]
        check(firstObservation?["kind"] as? String == "selection" && port.composerEventCount == beforeText + 1,
              "Actual intercepted text paste advances exactly one foundation observation")
        check(firstObservation as NSDictionary? == events[events.count - 2] as NSDictionary?, "Foundation paste observation is emitted immediately before identical embedded event")
        check(first["eventCount"] as? Int == firstObservation?["eventCount"] as? Int
              && first["value"] as? String == firstObservation?["value"] as? String
              && first["selection"] as? NSDictionary == firstObservation?["selection"] as? NSDictionary,
              "Rich outer source/count/selection match embedded foundation event")
        check(firstObservation?["pendingCommand"] as? NSDictionary == terminal["pendingCommand"] as? NSDictionary,
              "Paste observation preserves the exact earlier command terminal")
        check((first["payload"] as? [String: Any])?["text"] as? String == "intercepted text" && port.composerSnapshot?.value == "🙂 /model ",
              "Intercepted paste exposes payload without inserting text before TS decision")
        check(["eventCount", "value", "selection"].allSatisfy { (first["payload"] as? [String: Any])?[$0] == nil },
              "Paste data strips inner source-view counter and redundant document fields")
        pasteboard.items = [["public.utf8-plain-text": "context text", T3ComposerClipboard.fragmentType: Data("{\"version\":1,\"records\":[]}".utf8)]]
        text.paste(nil)
        let context = event("pasteContext")!, contextObservation = context["editorEvent"] as? [String: Any]
        check(contextObservation?["kind"] as? String == "selection" && contextObservation as NSDictionary? == events[events.count - 2] as NSDictionary?,
              "Actual custom context paste embeds its complete prior foundation observation")
        check(context["richEventId"] as? String != first["richEventId"] as? String && context["mountId"] as? String == port.composerIdentity?.mountId,
              "Two intercepted pastes keep unique request IDs on their captured mount")
        let beforeSize = port.composerEventCount
        view.onComposerContentSizeChange(["width": 100, "height": 40])
        check(port.composerEventCount == beforeSize && event("contentSize")?["editorEvent"] == nil, "Content size cannot advance document revision or produce a paste observation")
        text.setMarkedText("に", selectedRange: NSRange(location: 1, length: 0))
        let richBeforeIME = events.filter { ($0["kind"] as? String)?.hasPrefix("paste") == true }.count
        text.paste(nil)
        check(events.filter { ($0["kind"] as? String)?.hasPrefix("paste") == true }.count == richBeforeIME && text.markedTextRange != nil,
              "Real paste remains rejected during marked composition")
        text.unmarkText()
        pasteboard.items = []
        pasteboard.image = T3ComposerBundledIcon.image("t3-bundled-icon:typescript")
        let beforeImage = port.composerEventCount
        text.paste(nil)
        for _ in 0..<30 where event("pasteImages") == nil { try await Task.sleep(nanoseconds: 10_000_000) }
        let images = event("pasteImages"), imageObservation = images?["editorEvent"] as? [String: Any]
        check(images != nil && imageObservation?["kind"] as? String == "selection" && (imageObservation?["eventCount"] as? Int ?? 0) > beforeImage,
              "Actual asynchronous UIImage paste carries a new complete foundation observation")
        check(imageObservation?["pendingCommand"] as? NSDictionary == terminal["pendingCommand"] as? NSDictionary,
              "Asynchronous image paste keeps pending command outcome")
        let uris = (images?["payload"] as? [String: Any])?["uris"] as? [String] ?? []
        check(!uris.isEmpty && uris.allSatisfy { URL(string: $0).map { FileManager.default.fileExists(atPath: $0.path) } == true },
              "Image message retains real owned temporary PNG bytes")
        port.destroy(); port.view.removeFromSuperview()
        check(uris.allSatisfy { URL(string: $0).map { !FileManager.default.fileExists(atPath: $0.path) } == true },
              "Unadopted image lease cleanup is preserved after rich observation")
    }

    private func bundledIcons() throws {
        let bundle = Bundle.main.resourceURL!
        let typescriptURL = bundle.appendingPathComponent("assets/file-icons/pierre_typescript.png")
        let jsonURL = bundle.appendingPathComponent("assets/file-icons/pierre_json.png")
        let typescript = UIImage(contentsOfFile: typescriptURL.path), jsonImage = UIImage(contentsOfFile: jsonURL.path)
        check(typescript != nil && jsonImage != nil, "Fixture contains real pinned Pierre PNG assets")
        check(T3ComposerBundledIcon.image("t3-bundled-icon:typescript")?.pngData() == typescript?.pngData(),
              "Restricted identifier resolves and decodes the actual bundled Pierre asset")
        for invalid in ["", "../typescript", "%2e%2e", "//host/typescript", "typescript?x", "typescript#x", "https://example.com/icon", "TypeScript", "type.script"] {
            check(T3ComposerBundledIcon.relativePath("t3-bundled-icon:" + invalid) == nil, "Bundled icon rejects path/scheme escape: " + invalid)
        }
        check(T3ComposerBundledIcon.image("t3-bundled-icon:fixture-missing-key") == nil, "Missing bundled icon returns no invented asset")
        let temporary = FileManager.default.temporaryDirectory.appendingPathComponent("icon-root-" + UUID().uuidString, isDirectory: true)
        defer { try? FileManager.default.removeItem(at: temporary) }
        let folder = temporary.appendingPathComponent("assets/file-icons", isDirectory: true)
        try FileManager.default.createDirectory(at: folder, withIntermediateDirectories: true)
        try FileManager.default.copyItem(at: jsonURL, to: folder.appendingPathComponent("pierre_typescript.png"))
        check(T3ComposerBundledIcon.image("t3-bundled-icon:typescript", roots: [temporary, bundle])?.pngData() == jsonImage?.pngData(),
              "Existing development-root precedence wins over bundled file")
        check(T3ComposerBundledIcon.image("t3-bundled-icon:json", roots: [temporary, bundle])?.pngData() == jsonImage?.pngData(),
              "Missing development icon falls through to bundle")
        func chip(_ identifier: String) throws -> UIImage? {
            let document = T3MobileOwnedComposerView(frame: CGRect(x: 0, y: 0, width: 400, height: 80))
            defer { document.destroyOwned() }
            let token: [String: Any] = ["type": "mention", "source": "@file", "label": "file", "start": 0, "end": 5, "iconUri": identifier]
            document.setControlledDocumentJson(try json(["value": "@file", "selection": ["start": 5, "end": 5],
                "tokensJson": json([token]), "mostRecentEventCount": 0, "isNativeEcho": false]))
            return (document.textView.textStorage.attribute(.attachment, at: 0, effectiveRange: nil) as? ComposerTextAttachment)?.image
        }
        let rendered = try chip("t3-bundled-icon:typescript"), fallback = try chip("t3-bundled-icon:fixture-missing-key")
        check(rendered != nil && fallback != nil && rendered?.pngData() != fallback?.pngData(),
              "Actual rich attachment raster uses Pierre pixels rather than its fallback glyph")
    }
}
#endif
