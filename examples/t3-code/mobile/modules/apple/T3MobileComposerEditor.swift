#if os(iOS)
// App-native adapter over the pinned T3 rich editor; no Exact textarea delegate replacement.
// @ref examples/t3-code/mobile/docs/1130.005-composer-and-transcript.decision.md#composer-focus-restoration
import UIKit

protocol T3MobileComposerEndpoint: AnyObject {
    var composerIdentity: T3ComposerIdentity? { get }
    var composerSnapshot: T3ComposerSnapshot? { get }
    var composerEventCount: Int { get }
    var composerMounted: Bool { get }
    func applyVoiceSelection(identity: T3ComposerIdentity, expectedText: String, start: Int, end: Int, revision: Int) -> Bool
}

private final class T3ComposerContainer: UIView {
    weak var editor: T3MobileOwnedComposerView?
    override func layoutSubviews() { super.layoutSubviews(); editor?.frame = bounds }
}

final class T3MobileComposerEditor: ExactNativeInstance, T3MobileComposerEndpoint {
    private struct Frozen {
        let operation: T3ComposerHandoffRequest.Operation
        let retireUndo: Bool
        let receipt: T3ComposerFinalizations.Receipt
    }
    private struct Resumed {
        let operation: T3ComposerHandoffRequest.Operation
        let receiptId: String
        let document: T3ComposerDocument
        let event: [String: Any]
        func matches(_ operation: T3ComposerHandoffRequest.Operation, _ receiptId: String, _ document: T3ComposerDocument) -> Bool {
            self.operation == operation && self.receiptId == receiptId && self.document.value == document.value
                && self.document.selection == document.selection && self.document.tokensJson == document.tokensJson
        }
    }
    private enum Handoff { case editing(Resumed?), frozen(Frozen) }
    private var handoffState = Handoff.editing(nil)
    private var frozen: Bool { if case .frozen = handoffState { return true }; return false }
    private let root = T3ComposerContainer()
    private weak var voice: T3MobileVoiceEditor?
    private weak var fileHolds: T3MobileComposerFileHolds?
    private weak var operations: T3MobileComposerOperations?
    private weak var paste: T3MobileComposerPaste?
    private var editor: T3MobileOwnedComposerView?
    private var state: T3ComposerProtocolState?
    private var control: T3ComposerControl?
    private var focusIntent: T3ComposerFocusIntent?
    private var presentation: T3ComposerPresentation?
    private var voiceRevision = 0
    private var alive = true
    private var applying = false
    private var voiceOwner = ""
    private var terminalCommand: T3ComposerReplace?
    private var terminalEnvelope: [String: Any]?
    override var view: UIView { root }
    override var focusTarget: UIView? { editor?.textView }
    var composerIdentity: T3ComposerIdentity? { state?.identity }
    var composerSnapshot: T3ComposerSnapshot? { editor?.sourceSnapshot() }
    var composerEventCount: Int { state?.count ?? 0 }
    var composerMounted: Bool {
        alive && !frozen && root.window != nil && state?.readyAcknowledged == true && control?.active == true
            && control?.editable == true && control?.readOnly == false && UIApplication.shared.applicationState == .active
    }

    var composerFileHoldEligible: Bool { alive && root.window != nil && control?.active == true && control?.readOnly == false }

    init(voice: T3MobileVoiceEditor, operations: T3MobileComposerOperations? = nil, fileHolds: T3MobileComposerFileHolds? = nil, paste: T3MobileComposerPaste? = nil, events: ExactNativeEvents) {
        self.voice = voice
        self.operations = operations
        self.fileHolds = fileHolds
        self.paste = paste
        super.init(events: events)
        root.isAccessibilityElement = false
    }

    override func setProps(_ props: [String: String]) throws {
        guard alive, let json = props["configuration"] else { throw ExactNativeRefusal("Composer configuration is required.") }
        let next = try T3ComposerControl(json: json)
        if state == nil || state?.identity.sameAdmission(next.identity) != true {
            guard next.mountId.isEmpty else { throw ExactNativeRefusal("New composer admission must await ready.") }
            try mount(next)
            return
        }
        guard var state, let editor else { return }
        guard next.identity == state.identity else { return }
        let acknowledged = state.acknowledge(next)
        self.state = state; control = next
        if state.terminalID.isEmpty { terminalCommand = nil; terminalEnvelope = nil }
        configureInteraction(next, acknowledged: acknowledged)
        guard acknowledged else { return }
        if paste == nil { editor.textView.adoptPasteURIs(next.adoptedPasteURIs ?? []) }
        // Presentation can change independently, but every forced attributed rebuild is IME guarded.
        applying = true
        configurePresentation(next.presentation, editor: editor)
        applying = false
        guard !frozen else { return }
        if let command = next.command {
            _ = try? applyInvocation(identity: next.identity, command: command)
        } else if next.acknowledgedEventCount == self.state?.count, editor.textView.markedTextRange == nil {
            applying = true
            setDocument(next.document, editor: editor)
            applying = false
        }
        voice?.refresh(self, owner: voiceOwner, selectionRevision: next.voiceSelectionRevision ?? 0)
        applyFocus(next)
    }

    private func mount(_ next: T3ComposerControl) throws {
        retireEditor()
        state = .init(identity: next.identity); control = next
        do { try operations?.register(self) }
        catch { state = nil; control = nil; throw error }
        let editor = T3MobileOwnedComposerView(frame: root.bounds)
        self.editor = editor; root.editor = editor
        editor.textView.plainTextPaste = next.identity.independentNewTask
        root.addSubview(editor)
        editor.textView.accessibilityIdentifier = next.editorId
        editor.textView.accessibilityLabel = next.presentation?.placeholder
        editor.isAccessibilityElement = false
        applying = true
        configurePresentation(next.presentation, editor: editor)
        setDocument(next.document, editor: editor)
        editor.setEditable(false); editor.isUserInteractionEnabled = false
        applying = false
        bind(editor)
        voiceOwner = next.voiceOwner ?? next.owner
        voice?.register(self, owner: voiceOwner)
        fileHolds?.register(self)
        emit("ready", allowUnacknowledged: true)
    }

    private func bind(_ editor: T3MobileOwnedComposerView) {
        editor.textView.pasteAllowed = { [weak self, weak editor] in
            guard let self, let editor else { return false }
            return self.alive && !self.frozen && self.editor === editor && self.state?.readyAcknowledged == true && self.control?.active == true
                && self.control?.editable == true && self.control?.readOnly == false && self.root.window != nil
        }
        editor.onComposerChange = { [weak self, weak editor] _ in self?.receive("text", from: editor) }
        editor.onComposerSelectionChange = { [weak self, weak editor] _ in self?.receive("selection", from: editor) }
        editor.onComposerFocus = { [weak self, weak editor] _ in self?.receive("focus", from: editor) }
        editor.onComposerBlur = { [weak self, weak editor] _ in self?.receive("blur", from: editor) }
        editor.onComposerSubmit = { [weak self, weak editor] payload in self?.receive("submit", from: editor, extra: payload) }
        editor.onComposerPasteImages = { [weak self, weak editor] payload in self?.rich("pasteImages", from: editor, payload: payload) }
        editor.onComposerPasteContext = { [weak self, weak editor] payload in self?.rich("pasteContext", from: editor, payload: payload) }
        editor.onComposerPasteText = { [weak self, weak editor] payload in self?.rich("pasteText", from: editor, payload: payload) }
        editor.onComposerContextPress = { [weak self, weak editor] payload in self?.rich("contextPress", from: editor, payload: payload) }
        editor.onComposerContentSizeChange = { [weak self, weak editor] payload in self?.rich("contentSize", from: editor, payload: payload) }
    }

    private func receive(_ kind: String, from sender: T3MobileOwnedComposerView?, extra: [String: Any] = [:]) {
        guard alive, !applying, !frozen, let sender, sender === editor, state?.readyAcknowledged == true else { return }
        if kind == "submit", control?.active != true || control?.editable != true || control?.readOnly == true || sender.textView.markedTextRange != nil { return }
        emit(kind, extra: extra)
    }
    private func emit(_ kind: String, extra: [String: Any] = [:], allowUnacknowledged: Bool = false) {
        guard alive, var state, let snapshot = composerSnapshot, allowUnacknowledged || state.readyAcknowledged else { return }
        let event = state.event(kind, snapshot: snapshot, extra: extra)
        self.state = state
        send(event, rich: false)
    }
    private func send(_ object: [String: Any], rich: Bool) {
        guard let bytes = try? JSONSerialization.data(withJSONObject: object) else { return }
        let json = String(decoding: bytes, as: UTF8.self)
        if rich || !["text", "selection"].contains(object["kind"] as? String ?? "") { events.message(json) }
        else { events.change(json) }
    }
    // Separate source-rich callback channel. Parent must integrate explicit handlers before activation.
    private func rich(_ kind: String, from sender: T3MobileOwnedComposerView?, payload: [String: Any]) {
        guard alive, !applying, !frozen, let sender, sender === editor, var state, state.readyAcknowledged,
              let snapshot = composerSnapshot, let identity = composerIdentity else { return }
        if kind != "contentSize", control?.active != true { return }
        if kind.hasPrefix("paste"), control?.editable != true || control?.readOnly == true || snapshot.composing || root.window == nil || UIApplication.shared.applicationState != .active { return }
        // A source paste can precede the JS observation of its last keystroke.
        // Capture one complete foundation event; both channels carry identical
        // source state and retained command outcome, never the inner view counter.
        let observation: [String: Any]?
        if ["pasteImages", "pasteText", "pasteContext"].contains(kind) {
            observation = state.event("selection", snapshot: snapshot)
            self.state = state
        } else { observation = nil }
        let richEventId = UUID().uuidString
        var data = payload
        if kind == "pasteImages", let paste, let uris = payload["uris"] as? [String] {
            do { data = ["files": try paste.issue(identity: identity, event: richEventId, uris: uris, take: sender.textView.claimPasteURIs)] }
            catch { data = ["files": [[String: String]](), "error": (error as? T3ComposerPasteError)?.message ?? "Pasted image ownership is unavailable."] }
        }
        if observation != nil {
            data.removeValue(forKey: "eventCount"); data.removeValue(forKey: "value"); data.removeValue(forKey: "selection")
        }
        var message: [String: Any] = ["owner": identity.owner, "editorId": identity.editorId, "routeVisit": identity.routeVisit,
            "renderEpoch": identity.renderEpoch, "mountId": identity.mountId, "eventCount": state.count,
            "kind": kind, "richEventId": richEventId, "value": snapshot.value,
            "selection": ["start": snapshot.selection.start, "end": snapshot.selection.end],
            "composing": snapshot.composing, "focused": snapshot.focused, "payload": data]
        if let observation { message["editorEvent"] = observation; send(observation, rich: false) }
        send(message, rich: true)
    }

    func applyInvocation(identity: T3ComposerIdentity, command: T3ComposerReplace) throws -> [String: Any] {
        try command.validate()
        guard alive, var state, state.identity == identity, state.readyAcknowledged,
              let editor, let control, root.window != nil else {
            throw T3ComposerOperationFailure("superseded", "The captured editor is no longer mounted.")
        }
        guard !frozen else { throw T3ComposerOperationFailure("busy", "The editor source is held.") }
        if !state.terminalID.isEmpty {
            if let terminalCommand, terminalCommand.commandId == command.commandId {
                guard terminalCommand.sameRequest(command), let terminalEnvelope else {
                    throw T3ComposerOperationFailure("arguments", "A command identifier cannot name a different replacement.")
                }
                return terminalEnvelope
            }
            throw T3ComposerOperationFailure("busy", "Another replacement awaits acknowledgment.")
        }
        guard command.expected.eventCount <= state.count else {
            throw T3ComposerOperationFailure("superseded", "The captured editor revision is unavailable.")
        }
        let before = editor.sourceSnapshot()
        let active = control.active && control.editable && !control.readOnly && root.window != nil && UIApplication.shared.applicationState == .active
        let refusal = state.replacementRefusal(command, snapshot: before, active: active)
        if refusal == nil {
            applying = true
            editor.applyReplacement(value: command.next.value, selection: command.next.selection, tokensJson: command.next.tokensJson)
            applying = false
        }
        if let event = state.finish(command, applied: refusal == nil, reason: refusal ?? "", snapshot: editor.sourceSnapshot()) {
            self.state = state; terminalCommand = command; terminalEnvelope = event
            send(event, rich: false)
            return event
        }
        throw T3ComposerOperationFailure("superseded", "The replacement was already handled.")
    }
    func handoff(_ request: T3ComposerHandoffRequest) throws -> [String: Any] {
        guard alive, var state, state.identity == request.identity, state.readyAcknowledged,
              let editor, let control, let operations else {
            throw T3ComposerOperationFailure("superseded", "The captured editor is no longer mounted.")
        }
        switch request.action {
        case .freeze(let retireUndo):
            if case .frozen(let held) = handoffState {
                guard held.operation == request.operation, held.retireUndo == retireUndo else {
                    throw T3ComposerOperationFailure("busy", "Another editor handoff is active.")
                }
                return ["receipt": held.receipt.json]
            }
            if case .editing(let prior?) = handoffState,
               request.operation.revision <= prior.operation.revision || request.operation.id == prior.operation.id {
                throw T3ComposerOperationFailure("superseded", "The editor handoff was already completed.")
            }
            guard composerMounted, state.terminalID.isEmpty else {
                throw T3ComposerOperationFailure("busy", "Wait for the active editor command to finish.")
            }
            let snapshot = editor.sourceSnapshot()
            guard !snapshot.composing else { throw T3ComposerOperationFailure("busy", "Finish text composition before continuing.") }
            let event = state.event("frozen", snapshot: snapshot, extra: ["operationId": request.operation.id, "operationRevision": request.operation.revision])
            let receipt = try operations.freeze(self, event: event)
            self.state = state; handoffState = .frozen(.init(operation: request.operation, retireUndo: retireUndo, receipt: receipt))
            applying = true
            if retireUndo { editor.textView.undoManager?.removeAllActions() }
            configureInteraction(control, acknowledged: true)
            editor.blurEditor()
            applying = false
            send(event, rich: false)
            return ["receipt": receipt.json]
        case .resume(let receiptId, let document):
            if case .editing(let prior?) = handoffState, prior.matches(request.operation, receiptId, document) {
                return ["event": prior.event]
            }
            guard case .frozen(let held) = handoffState, held.operation == request.operation, held.receipt.id == receiptId else {
                throw T3ComposerOperationFailure("superseded", "The held editor changed before resuming.")
            }
            guard editor.textView.markedTextRange == nil else {
                throw T3ComposerOperationFailure("busy", "Finish text composition before resuming.")
            }
            try operations.resume(self, receiptId: receiptId)
            applying = true
            if held.retireUndo {
                setDocument(document, editor: editor)
                editor.textView.undoManager?.removeAllActions()
            } else if editor.sourceSnapshot().value == document.value, editor.matchesSourceTokens(document.tokensJson) {
                setDocument(document, editor: editor)
            } else if let selection = document.selection {
                editor.setReadOnly(false)
                editor.applyReplacement(value: document.value, selection: selection, tokensJson: document.tokensJson)
            }
            let event = state.event("resumed", snapshot: editor.sourceSnapshot(), extra: ["operationId": request.operation.id, "operationRevision": request.operation.revision, "receiptId": receiptId])
            self.state = state; handoffState = .editing(.init(operation: request.operation, receiptId: receiptId, document: document, event: event))
            configureInteraction(control, acknowledged: true)
            applying = false
            voice?.refresh(self, owner: voiceOwner, selectionRevision: control.voiceSelectionRevision ?? 0)
            applyFocus(control)
            send(event, rich: false)
            return ["event": event]
        }
    }
    private func setDocument(_ document: T3ComposerDocument, editor: T3MobileOwnedComposerView) {
        let selection: Any = document.selection.map { ["start": $0.start, "end": $0.end] } ?? NSNull()
        let value: [String: Any] = ["value": document.value, "selection": selection, "tokensJson": document.tokensJson,
            "mostRecentEventCount": editor.ownedEventCount, "isNativeEcho": document.isNativeEcho]
        if let bytes = try? JSONSerialization.data(withJSONObject: value) { editor.setControlledDocumentJson(String(decoding: bytes, as: UTF8.self)) }
    }
    private func configureInteraction(_ next: T3ComposerControl, acknowledged: Bool) {
        guard let editor else { return }
        let editable = acknowledged && next.active && next.editable && !frozen
        if !editable || next.readOnly { editor.textView.invalidatePendingPaste() }
        if editor.textView.isEditable != editable { editor.setEditable(editable) }
        editor.isUserInteractionEnabled = acknowledged && next.active && !frozen
        editor.setReadOnly(next.readOnly || frozen)
    }
    private func configurePresentation(_ p: T3ComposerPresentation?, editor: T3MobileOwnedComposerView) {
        guard let p, p != presentation else { return }
        let prior = presentation; presentation = p
        if let value = p.themeJson, value != prior?.themeJson { editor.setThemeJson(value) }
        if let value = p.placeholder, value != prior?.placeholder { editor.setPlaceholder(value) }
        if let value = p.fontFamily, value != prior?.fontFamily { editor.setFontFamily(value) }
        if let value = p.fontSize, value != prior?.fontSize { editor.setFontSize(value) }
        if let value = p.lineHeight, value != prior?.lineHeight { editor.setLineHeight(value) }
        if let value = p.contentInsetVertical, value != prior?.contentInsetVertical { editor.setContentInsetVertical(value) }
        if let value = p.scrollEnabled, value != prior?.scrollEnabled { editor.setScrollEnabled(value) }
        if let value = p.autoCorrect, value != prior?.autoCorrect { editor.setAutoCorrect(value) }
        if let value = p.spellCheck, value != prior?.spellCheck { editor.setSpellCheck(value) }
        if let value = p.enterBehavior, value != prior?.enterBehavior { editor.setEnterBehavior(value) }
        if let value = p.submitTitle, value != prior?.submitTitle { editor.setSubmitTitle(value) }
        if let value = p.alternateSubmitTitle, value != prior?.alternateSubmitTitle { editor.setAlternateSubmitTitle(value) }
        if let value = p.textPasteThresholdBytes, value != prior?.textPasteThresholdBytes { editor.setTextPasteThresholdBytes(value) }
        if let value = p.maxInputChars, value != prior?.maxInputChars { editor.setMaxInputChars(value) }
        if let value = p.clipboardFragment, value != prior?.clipboardFragment { editor.setClipboardFragment(value) }
    }
    private func applyFocus(_ next: T3ComposerControl) {
        guard !frozen else { return }
        guard focusIntent != next.focusIntent else { return }
        focusIntent = next.focusIntent
        guard let editor else { return }
        if next.focusIntent.operation == "blur" { editor.blurEditor(); return }
        guard next.focusIntent.operation == "focus", next.active, next.editable, !next.readOnly,
              UIApplication.shared.applicationState == .active, root.window != nil, root.bounds.width > 0, root.bounds.height > 0 else { return }
        var ancestor: UIView? = root
        while let current = ancestor {
            guard !current.isHidden, current.alpha > 0.02, current.isUserInteractionEnabled else { return }
            ancestor = current.superview
        }
        var responder: UIResponder? = root
        while let current = responder {
            if let controller = current as? UIViewController,
               controller.isBeingDismissed || controller.isBeingPresented || controller.transitionCoordinator != nil || controller.presentedViewController != nil { return }
            responder = current.next
        }
        if !editor.textView.isFirstResponder { editor.focusEditor() }
    }

    func applyVoiceSelection(identity: T3ComposerIdentity, expectedText: String, start: Int, end: Int, revision: Int) -> Bool {
        guard alive, composerMounted, control?.active == true, control?.editable == true, control?.readOnly == false,
              state?.identity == identity, revision > voiceRevision,
              let editor, editor.sourceSnapshot().value == expectedText, editor.textView.markedTextRange == nil,
              T3ComposerSelection(start: start, end: end).valid(expectedText) else { return false }
        voiceRevision = revision
        applying = true; editor.setSelection(start: start, end: end); applying = false
        emit("selection")
        return true
    }
    private func retireEditor() {
        let finalEvent: [String: Any]?
        if var state, state.readyAcknowledged, state.identity.independentNewTask, let snapshot = composerSnapshot {
            finalEvent = state.event("retired", snapshot: snapshot)
        } else { finalEvent = nil }
        applying = true
        if let operations { operations.unregister(self, finalEvent: finalEvent, fileHolds: fileHolds) }
        else { fileHolds?.unregister(self) }
        if let identity = state?.identity { paste?.retire(identity) }
        voice?.unregister(self)
        editor?.destroyOwned(); editor?.removeFromSuperview(); editor = nil; root.editor = nil
        state = nil; control = nil; focusIntent = nil; presentation = nil; voiceRevision = 0; voiceOwner = ""
        terminalCommand = nil; terminalEnvelope = nil
        handoffState = .editing(nil)
        applying = false
    }
    override func destroy() { guard alive else { return }; alive = false; retireEditor() }
}
#endif
