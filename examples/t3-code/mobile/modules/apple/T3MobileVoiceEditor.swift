#if os(iOS)
// @ref llp/1106.008-mobile-voice.decision.md#draft-and-selection
import UIKit

/// Reads the existing Exact editor; never takes its delegate, text or focus ownership.
final class T3MobileVoiceEditor {
    private struct Entry { weak var view: UITextView?; let owner: String; var revision: Int }
    private struct Selection { let owner: String; let text: String; let range: NSRange; let revision: Int }
    private var entries: [ObjectIdentifier: Entry] = [:]
    private var pending: Selection?

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
    func selection(owner: String, text: String) throws -> [String: Any] {
        let candidates = entries.values.filter { $0.owner == owner && $0.view?.window != nil && $0.view?.text == text }
        guard let view = candidates.first(where: { $0.view?.isFirstResponder == true })?.view ?? candidates.first?.view else {
            throw VoiceFailure("superseded", "The draft editor changed before voice input could start.")
        }
        let range = view.selectedRange, count = (text as NSString).length
        guard range.location <= count, range.length <= count - range.location else { throw VoiceFailure("voice", "The text selection is unavailable.") }
        return ["start": range.location, "end": range.location + range.length]
    }
    func stage(owner: String, text: String, start: Int, end: Int, revision: Int) {
        let length = (text as NSString).length, lower = max(0, min(length, start)), upper = max(0, min(length, end))
        pending = Selection(owner: owner, text: text, range: NSRange(location: lower, length: max(0, upper - lower)), revision: revision)
    }
    func destroy() { entries.removeAll(); pending = nil }
}
#endif
