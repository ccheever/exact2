// @ref LLP 1033: paint the same ordered inline runs the kernel measures.
import Foundation

extension NodeView {
    var isParagraph: Bool { kind == "text" && (superview as? NodeView)?.kind != "text" }

    func paragraphSpec() -> Spec {
        let align: Int
        switch style["text_align"] as? String {
        case "center": align = 1
        case "right": align = 2
        case "justify": align = 3
        default: align = 0
        }
        var runs: [Run] = []
        func collect(_ node: NodeView) {
            if let value = node.props["text"] {
                runs.append(Run(text: value, size: node.number("font_size", 16),
                    weight: Int(node.number("font_weight", 400)), family: Int(node.number("font_family")),
                    italic: (node.style["font_style"] as? String) == "italic",
                    lineHeight: node.number("line_height"), letterSpacing: node.number("letter_spacing"),
                    color: node.style["text_color"] as? [Double],
                    decoration: node.style["text_decoration_line"] as? String ?? "",
                    href: node.props["href"] ?? ""))
            } else {
                for child in node.container.subviews.compactMap({ $0 as? NodeView }) where child.kind == "text" { collect(child) }
            }
        }
        collect(self)
        return Spec(runs: runs, align: align, lineClamp: Int(number("line_clamp")),
                    color: style["text_color"] as? [Double] ?? [0, 0, 0, 255])
    }
}
