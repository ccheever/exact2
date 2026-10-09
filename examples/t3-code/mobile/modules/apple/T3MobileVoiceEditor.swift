#if os(iOS)
// @ref llp/1109.008-mobile-voice.decision.md#draft-and-selection
import UIKit

/// Reads the existing Exact editor; never takes its delegate, text or focus ownership.
final class T3MobileVoiceEditor {
    private struct Entry { weak var view: UITextView?; let owner: String; var revision: Int }
    private struct Selection { let owner: String; let text: String; let range: NSRange; let revision: Int }
    private var entries: [ObjectIdentifier: Entry] = [:]
    private var pending: Selection?
    private struct RichEntry { weak var endpoint: (any T3MobileComposerEndpoint)?; let owner: String }
    private struct RichCapture { let identity: T3ComposerIdentity; let eventCount: Int }
    private struct RichSelection { let capture: RichCapture; let text: String; let start: Int; let end: Int; let revision: Int }
    private var rich: [ObjectIdentifier: RichEntry] = [:]
    private var richPending: [String: RichSelection] = [:]

    func register(_ endpoint: any T3MobileComposerEndpoint, owner: String) {
        rich[ObjectIdentifier(endpoint)] = RichEntry(endpoint: endpoint, owner: owner)
    }
    func unregister(_ endpoint: any T3MobileComposerEndpoint) {
        guard let entry = rich.removeValue(forKey: ObjectIdentifier(endpoint)) else { return }
        if richPending[entry.owner]?.capture.identity == endpoint.composerIdentity { richPending.removeValue(forKey: entry.owner) }
    }
    func refresh(_ endpoint: any T3MobileComposerEndpoint, owner: String, selectionRevision: Int) {
        guard let pending = richPending[owner], pending.revision == selectionRevision,
              endpoint.composerIdentity == pending.capture.identity else { return }
        // An ABA edit, caret move or new mount cannot consume an older voice selection.
        guard endpoint.composerEventCount == pending.capture.eventCount else { richPending.removeValue(forKey: owner); return }
        if endpoint.applyVoiceSelection(identity: pending.capture.identity, expectedText: pending.text,
            start: pending.start, end: pending.end, revision: pending.revision) {
            richPending.removeValue(forKey: owner)
        }
    }


    func configure(_ element: ExactElement, owner: String, selectionRevision: Int) {
        guard let view = element.platform as? UITextView else { return }
        let key = ObjectIdentifier(element)
        let prior = entries[key]
        entries[key] = Entry(view: view, owner: owner, revision: prior?.owner == owner ? prior!.revision : 0)
        guard element.isLive, let pending, pending.owner == owner, pending.revision == selectionRevision,
              entries[key]!.revision < selectionRevision, view.text == pending.text else { return }
        view.selectedRange = pending.range
        entries[key]?.revision = selectionRevision
        self.pending = nil
    }
    func end(_ element: ExactElement) { entries.removeValue(forKey: ObjectIdentifier(element)) }
    func selection(owner: String, text: String, optional: Bool = false, sourceRevision: Int? = nil) throws -> [String: Any] {
        guard !owner.isEmpty else { throw VoiceFailure("superseded", "The draft editor is unavailable.") }
        let endpoints = rich.values.filter { $0.owner == owner }.compactMap(\.endpoint)
        if !endpoints.isEmpty {
            guard let sourceRevision, sourceRevision >= 0, sourceRevision <= T3ComposerProtocolState.maxCount else {
                throw VoiceFailure("arguments", "A rich editor selection requires its source document revision.")
            }
            let candidates = endpoints.filter { $0.composerMounted && $0.composerSnapshot?.value == text && $0.composerSnapshot?.composing == false }
            guard let endpoint = candidates.first(where: { $0.composerSnapshot?.focused == true }) ?? (candidates.count == 1 ? candidates.first : nil),
                  let snapshot = endpoint.composerSnapshot, let identity = endpoint.composerIdentity else {
                throw VoiceFailure("superseded", "The rich draft editor changed before voice input could start.")
            }
            return ["start": snapshot.selection.start, "end": snapshot.selection.end,
                    "editorId": identity.editorId, "mountId": identity.mountId, "renderEpoch": identity.renderEpoch,
                    "routeVisit": identity.routeVisit, "eventCount": endpoint.composerEventCount,
                    "capture": T3ComposerVoiceCapture(identity: identity, eventCount: endpoint.composerEventCount, sourceRevision: sourceRevision).json]
        }
        let mounted = entries.values.filter { $0.owner == owner && $0.view?.window != nil }
        if optional && mounted.isEmpty { let end = (text as NSString).length; return ["start": end, "end": end] }
        let candidates = mounted.filter { $0.view?.text == text }
        guard let view = candidates.first(where: { $0.view?.isFirstResponder == true })?.view ?? candidates.first?.view else {
            throw VoiceFailure("superseded", "The draft editor changed before voice input could start.")
        }
        let range = view.selectedRange, count = (text as NSString).length
        guard range.location <= count, range.length <= count - range.location else { throw VoiceFailure("voice", "The text selection is unavailable.") }
        return ["start": range.location, "end": range.location + range.length]
    }
    @discardableResult
    func stageCaptured(owner: String, identity: T3ComposerIdentity, eventCount: Int, text: String, start: Int, end: Int, revision: Int) -> Bool {
        guard T3ComposerSelection(start: start, end: end).valid(text), revision > 0,
              richPending[owner].map({ $0.revision <= revision }) ?? true,
              rich.values.contains(where: { $0.owner == owner && $0.endpoint?.composerIdentity == identity && $0.endpoint?.composerEventCount == eventCount }) else { return false }
        richPending[owner] = RichSelection(capture: RichCapture(identity: identity, eventCount: eventCount), text: text, start: start, end: end, revision: revision)
        return true
    }
    @discardableResult
    func stage(owner: String, text: String, start: Int, end: Int, revision: Int) -> Bool {
        // Legacy callers have no invocation capture receipt. Never resolve a rich
        // editor through the latest owner slot; its runtime must use stageCaptured.
        if rich.values.contains(where: { $0.owner == owner && $0.endpoint != nil }) { return false }
        guard pending.map({ $0.revision <= revision }) ?? true else { return true }
        let length = (text as NSString).length, lower = max(0, min(length, start)), upper = max(0, min(length, end))
        pending = Selection(owner: owner, text: text, range: NSRange(location: lower, length: max(0, upper - lower)), revision: revision)
        return true
    }
    func destroy() { entries.removeAll(); pending = nil; rich.removeAll(); richPending.removeAll() }
}
#endif
