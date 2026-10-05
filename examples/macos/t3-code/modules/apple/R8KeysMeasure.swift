#if os(macOS)
import AppKit

/// Lane r8-keys: where a hooked node sits in the window right now (hook `t3-measure`,
/// `data-frame` names it). Contract's `frame(id)` is the layout's position, which a
/// scrolled transcript leaves behind; a popup anchored to a node inside the transcript
/// (a table's Copy menu) asks for the drawn position at the moment it opens.
final class R8KeysMeasure {
    private final class Entry { weak var element: ExactElement?; init(_ element: ExactElement) { self.element = element } }
    private var entries: [String: Entry] = [:]

    func install(_ element: ExactElement) {
        guard element.hook == .t3Measure, let name = element.data[.frame], !name.isEmpty else { return }
        entries = entries.filter { $0.value.element != nil && $0.value.element !== element }
        entries[name] = Entry(element)
    }
    func remove(_ element: ExactElement) { entries = entries.filter { $0.value.element != nil && $0.value.element !== element } }

    /// [x, top, width, height] in the window content's top-left space and the content's size, or nil.
    func frame(_ name: String) -> [String: Double]? {
        guard let element = entries[name]?.element, element.isLive, let view = element.view, !view.isHiddenOrHasHiddenAncestor,
              let content = view.window?.contentView else { return nil }
        let box = view.convert(view.bounds, to: content)
        let top = content.isFlipped ? box.minY : content.bounds.height - box.maxY
        return ["x": Double(box.minX), "y": Double(top), "width": Double(box.width), "height": Double(box.height),
                "windowWidth": Double(content.bounds.width), "windowHeight": Double(content.bounds.height)]
    }

    /// `native.later({op: "r8MeasureFrame", name})`.
    func perform(_ request: [String: Any]) -> [String: Any] {
        let generation = request["generation"] as? Int ?? 0
        guard let name = request["name"] as? String, let frame = frame(name) else {
            return ["ok": false, "generation": generation, "error": ["kind": "Measure", "message": "That node is not on screen.", "uncertain": false]]
        }
        return ["ok": true, "generation": generation, "value": frame]
    }
}
#endif
