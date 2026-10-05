#if os(macOS)
import AppKit

/// The composer chips' hover details (composerContextPresentation.tsx and
/// ComposerPromptEditorTiptap.tsx, T3 Code, MIT; see LICENSE-T3), lane
/// r4-timeline: a mention's path, an attached file's or image's name and size
/// on two lines, a thread's "Open thread", and "This context is no longer
/// available. Remove it or attach it again." for a reference whose record is
/// gone. AppKit's tool-tip rects stand in for the reference's tooltip popup;
/// they follow each chip's drawn segments and are re-registered whenever the
/// styler restyles or the text view moves.
final class T3ComposerChipTips: NSObject, NSViewToolTipOwner {
    private weak var styler: T3ComposerStyler?
    private weak var view: NSTextView?
    private var tags: [NSView.ToolTipTag: String] = [:]

    init(styler: T3ComposerStyler) { self.styler = styler }

    /// Re-register one tool-tip rect per chip segment.
    func refresh() {
        guard let styler, let view = styler.view else { clear(); return }
        if view !== self.view { clear(); self.view = view }
        for tag in tags.keys { view.removeToolTip(tag) }
        tags.removeAll()
        let length = (view.string as NSString).length
        for chip in styler.chips where chip.end <= length {
            guard let text = tip(for: chip) else { continue }
            for rect in styler.segments(chip.range) where rect.width > 0 && rect.height > 0 {
                tags[view.addToolTip(rect, owner: self, userData: nil)] = text
            }
        }
    }

    private func clear() {
        if let view { for tag in tags.keys { view.removeToolTip(tag) } }
        tags.removeAll(); view = nil
    }

    /// What the reference's chip tooltip says for `chip`, or nil when it shows none.
    func tip(for chip: T3ComposerChip) -> String? {
        guard let styler else { return nil }
        switch chip.kind {
        case "mention": return chip.detail.isEmpty ? nil : chip.detail
        case "context":
            guard styler.chipResolved(chip) else { return "This context is no longer available. Remove it or attach it again." }
            switch styler.chipKind(chip) {
            case "thread": return "Open thread"
            case "file", "image", "video":
                let size = styler.chipSuffix(chip)
                return size.isEmpty ? chip.label : "\(chip.label)\n\(size)"
            default: return nil
            }
        default: return nil
        }
    }

    func view(_ view: NSView, stringForToolTip tag: NSView.ToolTipTag, point: NSPoint, userData data: UnsafeMutableRawPointer?) -> String {
        tags[tag] ?? ""
    }

    /// The tool tip registered over `point` (view coordinates), for tests.
    func registered(at point: NSPoint) -> String? {
        guard let styler, let view else { return nil }
        let length = (view.string as NSString).length
        for chip in styler.chips where chip.end <= length {
            if styler.segments(chip.range).contains(where: { $0.contains(point) }) { return tip(for: chip) }
        }
        return nil
    }
    var count: Int { tags.count }
}
#endif
