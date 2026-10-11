#if os(macOS)
import AppKit

/// r5-composer: a ref list's "load the next page" signal (BranchPicker.tsx
/// maybeFetchNextBranchPage, paginatedBranches.ts shouldLoadNextBranchPageAfterScroll):
/// a scroll node hooked `t3-anchor` with `data-anchor="scroll:<name>"` counts each
/// scroll toward its end that lands within 96pt of it. One count per content
/// height, so a page that loads (and grows the list) arms the next one; the
/// TypeScript side fetches once per count it has not seen (r5-composer-paging.ts).
final class R5ComposerScroll {
    private final class Entry {
        weak var element: ExactElement?
        var observer: NSObjectProtocol?
        var resized: NSObjectProtocol?
        var previousTop: CGFloat?
        var previousContent: CGFloat = -1
        var previousHeight: CGFloat = -1
        var countedContent: CGFloat = -1
        init(_ element: ExactElement) { self.element = element }
    }
    private let changed: (String) -> Void
    private var entries: [String: Entry] = [:]
    private(set) var ends: [String: Int] = [:]
    static let distance: CGFloat = 96

    init(changed: @escaping (String) -> Void = { _ in }) { self.changed = changed }

    var status: [String: Any] { ["scrollEnds": ends] }

    func install(_ element: ExactElement) {
        guard element.hatch == .t3Anchor, let name = element.data[.anchor], name.hasPrefix("scroll:"),
              let scroll = element.scrollView else { return }
        if entries[name]?.element === element { return }
        detach(name)
        let entry = Entry(element)
        let clip = scroll.contentView
        clip.postsBoundsChangedNotifications = true
        entry.previousTop = nil
        entry.observer = NotificationCenter.default.addObserver(forName: NSView.boundsDidChangeNotification, object: clip, queue: .main) { [weak self] _ in
            self?.observe(name)
        }
        // The list's own growth (it mounting, a page arriving) re-bases the reading without counting.
        if let document = scroll.documentView {
            document.postsFrameChangedNotifications = true
            entry.resized = NotificationCenter.default.addObserver(forName: NSView.frameDidChangeNotification, object: document, queue: .main) { [weak self] _ in
                self?.rebase(name)
            }
        }
        entries[name] = entry
        rebase(name)
    }

    private func rebase(_ name: String) {
        guard let entry = entries[name], let scroll = entry.element?.scrollView, let document = scroll.documentView else { return }
        let clip = scroll.contentView
        entry.previousTop = clip.isFlipped ? clip.bounds.minY : document.frame.height - clip.bounds.maxY
        entry.previousContent = document.frame.height; entry.previousHeight = clip.bounds.height
    }

    func remove(_ element: ExactElement) {
        for (name, entry) in entries where entry.element == nil || entry.element === element { detach(name) }
    }

    func destroy() { for name in Array(entries.keys) { detach(name) } }

    private func detach(_ name: String) {
        if let observer = entries[name]?.observer { NotificationCenter.default.removeObserver(observer) }
        if let resized = entries[name]?.resized { NotificationCenter.default.removeObserver(resized) }
        entries[name] = nil
    }

    /// Internal so tests can drive it after scrolling a view.
    func observe(_ name: String) {
        guard let entry = entries[name], let scroll = entry.element?.scrollView, let document = scroll.documentView else { return }
        let clip = scroll.contentView
        let top = clip.isFlipped ? clip.bounds.minY : document.frame.height - clip.bounds.maxY
        let content = document.frame.height, height = clip.bounds.height
        let previous = entry.previousTop, stable = content == entry.previousContent && height == entry.previousHeight
        entry.previousTop = top; entry.previousContent = content; entry.previousHeight = height
        // Only a scroll moves the list: a layout pass that grows or resizes it (the list mounting, a page
        // arriving) is never a reader's scroll toward the end.
        guard let previous, stable, top > previous else { return }
        guard content - top - height <= Self.distance, content != entry.countedContent else { return }
        entry.countedContent = content
        ends[name, default: 0] += 1
        changed("t3.status")
    }
}
#endif
