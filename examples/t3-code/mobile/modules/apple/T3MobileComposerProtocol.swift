// App-owned composer event/CAS boundary; source UTF16, never displayed chip offsets.
// @ref llp/1109.005-composer-and-transcript.decision.md#composer-focus-restoration
import Foundation

struct T3ComposerIdentity: Codable, Equatable {
    let owner: String
    let editorId: String
    let routeVisit: String
    let renderEpoch: String
    let mountId: String
    var admitted: Bool { !owner.isEmpty && !editorId.isEmpty && !routeVisit.isEmpty && !renderEpoch.isEmpty }
    func sameAdmission(_ other: Self) -> Bool {
        owner == other.owner && editorId == other.editorId && routeVisit == other.routeVisit && renderEpoch == other.renderEpoch
    }
    func mounted(_ mount: String) -> Self {
        .init(owner: owner, editorId: editorId, routeVisit: routeVisit, renderEpoch: renderEpoch, mountId: mount)
    }
}
struct T3ComposerSelection: Codable, Equatable {
    let start: Int
    let end: Int
    func valid(_ value: String) -> Bool { start >= 0 && end >= start && end <= value.utf16.count }
}
struct T3ComposerSnapshot: Equatable {
    let value: String
    let selection: T3ComposerSelection
    let composing: Bool
    let focused: Bool
}
struct T3ComposerDocument: Decodable {
    let value: String
    let selection: T3ComposerSelection?
    let tokensJson: String
    let isNativeEcho: Bool
    func validate() throws {
        guard selection?.valid(value) ?? true else { throw T3ComposerProtocolError.invalid("Invalid source selection") }
        try T3ComposerTokenValidator.validate(tokensJson, value: value)
    }
}
struct T3ComposerReplace: Decodable {
    struct Expected: Decodable { let eventCount: Int; let value: String; let selection: T3ComposerSelection }
    struct Next: Decodable { let value: String; let selection: T3ComposerSelection; let tokensJson: String }
    let commandId: String
    let commandRevision: Int
    let expected: Expected
    let next: Next
    func validate() throws {
        guard !commandId.isEmpty, commandRevision >= 0, commandRevision <= T3ComposerProtocolState.maxCount,
              expected.eventCount >= 0, expected.eventCount <= T3ComposerProtocolState.maxCount,
              expected.selection.valid(expected.value), next.selection.valid(next.value) else {
            throw T3ComposerProtocolError.invalid("Invalid replacement command")
        }
        try T3ComposerTokenValidator.validate(next.tokensJson, value: next.value)
    }
}
struct T3ComposerFocusIntent: Decodable, Equatable {
    let serial: String
    let attempt: Int
    let operation: String
}
struct T3ComposerPresentation: Decodable, Equatable {
    let themeJson: String?
    let placeholder: String?
    let fontFamily: String?
    let fontSize: Double?
    let lineHeight: Double?
    let contentInsetVertical: Double?
    let scrollEnabled: Bool?
    let autoCorrect: Bool?
    let spellCheck: Bool?
    let enterBehavior: String?
    let submitTitle: String?
    let alternateSubmitTitle: String?
    let textPasteThresholdBytes: Int?
    let maxInputChars: Int?
    let clipboardFragment: String?
}
struct T3ComposerControl: Decodable {
    let owner: String
    let editorId: String
    let routeVisit: String
    let renderEpoch: String
    let mountId: String
    let acknowledgedEventCount: Int
    let ackCommandId: String
    let document: T3ComposerDocument
    let active: Bool
    let editable: Bool
    let readOnly: Bool
    let focusIntent: T3ComposerFocusIntent
    let command: T3ComposerReplace?
    let presentation: T3ComposerPresentation?
    let voiceOwner: String?
    let voiceSelectionRevision: Int?
    let adoptedPasteURIs: [String]?
    var identity: T3ComposerIdentity { .init(owner: owner, editorId: editorId, routeVisit: routeVisit, renderEpoch: renderEpoch, mountId: mountId) }
    init(json: String) throws {
        self = try JSONDecoder().decode(Self.self, from: Data(json.utf8))
        guard identity.admitted, acknowledgedEventCount >= 0, acknowledgedEventCount <= T3ComposerProtocolState.maxCount,
              focusIntent.attempt >= 0, focusIntent.attempt <= 20,
              ["none", "focus", "blur"].contains(focusIntent.operation),
              focusIntent.operation == "none" || !focusIntent.serial.isEmpty else {
            throw T3ComposerProtocolError.invalid("Invalid composer admission")
        }
        try document.validate(); try command?.validate()
        if let p = presentation {
            for n in [p.fontSize, p.lineHeight, p.contentInsetVertical].compactMap({ $0 }) {
                guard n.isFinite, n >= 0, n <= 1000 else { throw T3ComposerProtocolError.invalid("Invalid composer dimensions") }
            }
            if let mode = p.enterBehavior, !["send", "newline"].contains(mode) { throw T3ComposerProtocolError.invalid("Invalid Enter behavior") }
            if let limit = p.maxInputChars, limit < 0 { throw T3ComposerProtocolError.invalid("Invalid input limit") }
            if let limit = p.textPasteThresholdBytes, limit < 0 { throw T3ComposerProtocolError.invalid("Invalid paste limit") }
        }
    }
}
enum T3ComposerProtocolError: Error { case invalid(String) }

enum T3ComposerTokenValidator {
    private struct Token: Decodable { let type: String; let source: String; let label: String; let start: Int; let end: Int }
    static func validate(_ json: String, value: String) throws {
        let tokens = try JSONDecoder().decode([Token].self, from: Data(json.utf8)), text = value as NSString
        var previous = 0
        for token in tokens {
            guard token.start >= previous, token.end > token.start, token.end <= text.length,
                  !token.type.isEmpty, text.substring(with: NSRange(location: token.start, length: token.end - token.start)) == token.source else {
                throw T3ComposerProtocolError.invalid("Invalid rich token range")
            }
            previous = token.end
        }
    }
}

/// No UIKit, timers, handles or persistence. Each port owns exactly one state.
struct T3ComposerProtocolState {
    static let maxCount = 9_007_199_254_740_991
    let identity: T3ComposerIdentity
    private(set) var count = 0
    private(set) var readyAcknowledged = false
    private(set) var terminal: [String: Any]?
    private(set) var terminalID = ""
    private(set) var terminalCount = 0
    private var lastCommandRevision = -1
    private var lastCommandID = ""
    private var commandIDs = Set<String>()
    init(identity: T3ComposerIdentity, mountId: String = UUID().uuidString) { self.identity = identity.mounted(mountId) }
    mutating func acknowledge(_ control: T3ComposerControl) -> Bool {
        guard control.identity == identity, control.acknowledgedEventCount <= count else { return false }
        if control.acknowledgedEventCount >= 1 { readyAcknowledged = true }
        if !terminalID.isEmpty, control.ackCommandId == terminalID, control.acknowledgedEventCount >= terminalCount {
            terminal = nil; terminalID = ""; terminalCount = 0
        }
        return readyAcknowledged
    }
    mutating func event(_ kind: String, snapshot: T3ComposerSnapshot, extra: [String: Any] = [:]) -> [String: Any] {
        precondition(count < Self.maxCount)
        count += 1
        var value: [String: Any] = ["owner": identity.owner, "editorId": identity.editorId,
            "routeVisit": identity.routeVisit, "renderEpoch": identity.renderEpoch, "mountId": identity.mountId,
            "kind": kind, "eventCount": count, "value": snapshot.value,
            "selection": ["start": snapshot.selection.start, "end": snapshot.selection.end],
            "composing": snapshot.composing, "focused": snapshot.focused]
        value.merge(extra) { _, incoming in incoming }
        if let terminal { value["pendingCommand"] = terminal }
        return value
    }
    func replacementRefusal(_ command: T3ComposerReplace, snapshot: T3ComposerSnapshot, active: Bool) -> String? {
        if !readyAcknowledged { return "Editor has not been acknowledged." }
        if !terminalID.isEmpty { return "Another replacement awaits acknowledgment." }
        if command.commandRevision <= lastCommandRevision || commandIDs.contains(command.commandId) { return "Replacement was already handled." }
        if !active { return "Editor is not active or editable." }
        if snapshot.composing { return "Text composition is active." }
        if command.expected.eventCount != count || command.expected.value != snapshot.value || command.expected.selection != snapshot.selection {
            return "Editor changed before replacement."
        }
        return nil
    }
    /// Call only once for a previously unseen command. Existing terminal is immutable.
    mutating func finish(_ command: T3ComposerReplace, applied: Bool, reason: String, snapshot: T3ComposerSnapshot) -> [String: Any]? {
        guard terminalID.isEmpty, command.commandRevision > lastCommandRevision, !commandIDs.contains(command.commandId), command.expected.eventCount <= count else { return nil }
        lastCommandRevision = command.commandRevision; lastCommandID = command.commandId; commandIDs.insert(command.commandId)
        let kind = applied ? "commandApplied" : "commandRejected"
        var value = event(kind, snapshot: snapshot, extra: ["commandId": command.commandId, "commandRevision": command.commandRevision, "reason": reason])
        let outcome: [String: Any] = ["kind": kind, "commandId": command.commandId, "commandRevision": command.commandRevision,
            "eventCount": count, "value": snapshot.value, "selection": ["start": snapshot.selection.start, "end": snapshot.selection.end],
            "composing": snapshot.composing, "focused": snapshot.focused, "reason": reason]
        terminal = outcome; terminalID = command.commandId; terminalCount = count; value["pendingCommand"] = outcome
        return value
    }
}
