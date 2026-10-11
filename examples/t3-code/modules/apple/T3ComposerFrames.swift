#if os(macOS)
import AppKit

/// Where the composer's popover anchors sit in the window (hook `t3-frame`,
/// `data-frame` names the anchor). Contract positions its menus from the
/// window edges, so a draft composer centred in the column opens them above
/// its own controls rather than at the bottom of the window. A move of the
/// anchor or of any view above it re-measures.
final class T3ComposerFrames {
    private let changed: (String) -> Void
    private final class Entry {
        weak var element: ExactElement?
        var observers: [NSObjectProtocol] = []
        init(_ element: ExactElement) { self.element = element }
    }
    private var entries: [String: Entry] = [:]
    /// name → [x, top, width, height] in the window content's top-left space, in points.
    private(set) var frames: [String: [Double]] = [:]

    init(changed: @escaping (String) -> Void = { _ in }) { self.changed = changed }

    var status: [String: Any] { ["frames": frames] }

    func install(_ element: ExactElement) {
        guard element.hatch == .t3Frame else { return }
        let name = element.data[.frame] ?? ""
        guard !name.isEmpty else { return }
        if entries[name]?.element === element { measure(); return }
        detach(name)
        let entry = Entry(element)
        var view = element.view
        while let current = view {
            current.postsFrameChangedNotifications = true
            entry.observers.append(NotificationCenter.default.addObserver(forName: NSView.frameDidChangeNotification,
                object: current, queue: .main) { [weak self] _ in self?.measure() })
            view = current.superview
        }
        entries[name] = entry
        measure()
    }

    func remove(_ element: ExactElement) {
        let ended = entries.filter { $0.value.element == nil || $0.value.element === element }.map(\.key)
        for name in ended { detach(name) }
        if !ended.isEmpty { measure() }
    }

    private func detach(_ name: String) {
        for observer in entries[name]?.observers ?? [] { NotificationCenter.default.removeObserver(observer) }
        entries[name] = nil
    }

    /// Internal so tests can drive it after moving a view.
    func measure() {
        var next: [String: [Double]] = [:]
        for (name, entry) in entries {
            guard let element = entry.element, element.isLive, let view = element.view,
                  let content = view.window?.contentView else { continue }
            let box = view.convert(view.bounds, to: content)
            let top = content.isFlipped ? box.minY : content.bounds.height - box.maxY
            let half = { (value: CGFloat) in (Double(value) * 2).rounded() / 2 }
            next[name] = [half(box.minX), half(top), half(box.width), half(box.height)]
        }
        guard next != frames else { return }
        frames = next
        changed("t3.status")
    }

    func destroy() {
        for name in Array(entries.keys) { detach(name) }
        frames = [:]
    }

    deinit { destroy() }
}
#endif
