#if os(macOS)
import AppKit

/// Paragraph geometry in each scroll document's own coordinates. A scroll
/// changes the query rectangle, never the index. Layout/structure changes
/// discard it. Selection and copy keep their complete document order.
/// @ref LLP 1033 (long documents), LLP 1010 (native scrolling)
struct TextViewportIndex {
    private struct Entry {
        let node: NodeView
        let rect: NSRect
        var bottom: CGFloat
    }
    private struct Group {
        let document: NSView
        var entries: [Entry]
    }
    private var groups: [Group] = []

    init(_ paragraphs: [NodeView]) {
        var positions: [ObjectIdentifier: Int] = [:]
        for node in paragraphs {
            // A paragraph outside every scroll view (a heading over a split's
            // detail pane, a popover's text) is in view wherever its window
            // is: its window's content view stands in as its document.
            guard let document = node.enclosingScrollView?.documentView ?? node.window?.contentView else { continue }
            let key = ObjectIdentifier(document)
            let index = positions[key] ?? groups.count
            if index == groups.count { positions[key] = index; groups.append(Group(document: document, entries: [])) }
            let rect = node.convert(node.bounds, to: document)
            groups[index].entries.append(Entry(node: node, rect: rect, bottom: rect.maxY))
        }
        for i in groups.indices {
            groups[i].entries.sort { $0.rect.minY < $1.rect.minY }
            var bottom = -CGFloat.infinity
            for j in groups[i].entries.indices {
                bottom = max(bottom, groups[i].entries[j].rect.maxY)
                groups[i].entries[j].bottom = bottom
            }
        }
    }

    /// Paragraphs within `reach`, nearest each scroll document's visible rect
    /// first. A tall distant paragraph must not delay a short imminent one.
    func candidates(reach: CGFloat = 0) -> [NodeView] {
        var result: [(CGFloat, NodeView)] = []
        for group in groups {
            let shown = group.document.visibleRect
            guard !shown.isEmpty else { continue }
            let visible = shown.insetBy(dx: -reach, dy: -reach)
            // Prefix maxima include tall/overlapping paragraphs that start
            // before the viewport. Binary-searching only minY loses them.
            var lo = 0, hi = group.entries.count
            while lo < hi {
                let mid = lo + (hi - lo) / 2
                if group.entries[mid].bottom <= visible.minY { lo = mid + 1 }
                else { hi = mid }
            }
            var index = lo
            while index < group.entries.count && group.entries[index].rect.minY < visible.maxY {
                let entry = group.entries[index]
                if entry.rect.intersects(visible) {
                    let dx = max(0, shown.minX - entry.rect.maxX, entry.rect.minX - shown.maxX)
                    let dy = max(0, shown.minY - entry.rect.maxY, entry.rect.minY - shown.maxY)
                    result.append((max(dx, dy), entry.node))
                }
                index += 1
            }
        }
        return result.sorted {
            $0.0 == $1.0 ? $0.1.id < $1.1.id : $0.0 < $1.0
        }.map { $0.1 }
    }
}
#endif
