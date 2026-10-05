// LLP 1021 D2, §5: where a painted popover sits against the invoker that
// opened it — CSS `position-area` with the invoker as its implicit anchor, in
// the subset the contract admits — the same on iOS's and macOS's top layers,
// and the point macOS pops a chooser's NSMenu up at.
import CoreGraphics

enum PositionArea {
    /// A box's resolved margins, in points.
    struct Margins: Equatable {
        var top: CGFloat = 0, right: CGFloat = 0, bottom: CGFloat = 0, left: CGFloat = 0
    }

    /// The row's value on a popover (`none` when it has none).
    static func of(_ popover: NodeView) -> String { popover.style["position_area"]?.string ?? "none" }
    /// The popover's margins as the kernel resolved them: points; `auto`
    /// (and a percentage, which the host never sees resolved) is 0, as CSS
    /// resolves an auto margin on a box placed by `position-area`.
    static func margins(of popover: NodeView) -> Margins {
        Margins(top: popover.number("margin_top"), right: popover.number("margin_right"),
                bottom: popover.number("margin_bottom"), left: popover.number("margin_left"))
    }

    /// The current option belongs to this opening, like the open state.
    struct History {
        var area = "", fallbacks = "", size = CGSize.zero, margins = Margins()
        var choice = 0
    }

    static func origin(_ area: String, anchor: CGRect, size: CGSize, margins: Margins = .init(),
                       fallbacks: String = "none", in bounds: CGRect) -> CGPoint {
        var history = History()
        return origin(area, anchor: anchor, size: size, margins: margins, fallbacks: fallbacks, history: &history, in: bounds)
    }

    /// Each attempt starts from the authored area and margins. Centred axes
    /// use CSS's anchor-center overflow alignment; an edge-aligned axis
    /// overflows unless an authored fallback fits (CSS Anchor Positioning §6).
    static func origin(_ area: String, anchor: CGRect, size: CGSize, margins: Margins = .init(),
                       fallbacks: String = "none", history: inout History, in bounds: CGRect) -> CGPoint {
        if history.area != area || history.fallbacks != fallbacks || history.size != size || history.margins != margins {
            history = History(area: area, fallbacks: fallbacks, size: size, margins: margins)
        }
        let axes = axes(area)
        func attempt(_ x: Int, _ y: Int, _ m: Margins) -> (CGPoint, CGRect) {
            let outer = CGSize(width: size.width + m.left + m.right, height: size.height + m.top + m.bottom)
            func at(_ side: Int, _ low: CGFloat, _ high: CGFloat, _ length: CGFloat, _ min: CGFloat, _ max: CGFloat) -> CGFloat {
                switch side {
                case -2: return low - length
                case -1: return high - length
                case 1: return low
                case 2: return high
                default: return Swift.max(min, Swift.min((low + high - length) / 2, max - length))
                }
            }
            var point = CGPoint(x: at(x, anchor.minX, anchor.maxX, outer.width, bounds.minX, bounds.maxX),
                                y: at(y, anchor.minY, anchor.maxY, outer.height, bounds.minY, bounds.maxY))
            // No authored area keeps D2's native default placement.
            if area == "none" {
                point.x = max(bounds.minX, min(point.x, bounds.maxX - outer.width))
                point.y = max(bounds.minY, min(point.y, bounds.maxY - outer.height))
            }
            return (CGPoint(x: point.x + m.left, y: point.y + m.top), CGRect(origin: point, size: outer))
        }
        let options = ["none"] + fallbacks.split(separator: ",").map(String.init)
        func selected(_ index: Int) -> (CGPoint, CGRect) {
            let words = options[index].split(whereSeparator: { $0.isWhitespace })
            var (x, y, m) = (axes.x, axes.y, margins)
            if words.contains("flip-block") { y = -y; swap(&m.top, &m.bottom) }
            if words.contains("flip-inline") { x = -x; swap(&m.left, &m.right) }
            return attempt(x, y, m)
        }
        let current = selected(history.choice)
        if bounds.contains(current.1) || area == "none" { return current.0 }
        for index in options.indices where index != history.choice {
            let candidate = selected(index)
            if bounds.contains(candidate.1) {
                history.choice = index
                return candidate.0
            }
        }
        return current.0
    }

    // -2/2: outside the anchor; -1/1: span toward that edge; 0: anchor-center.
    private static func axes(_ area: String) -> (x: Int, y: Int) {
        if area == "none" { return (1, 2) }
        var (x, y) = (0, 0)
        for word in area.split(separator: " ") {
            switch word {
            case "left": x = -2
            case "right": x = 2
            case "top": y = -2
            case "bottom": y = 2
            case "span-left": x = -1
            case "span-right": x = 1
            case "span-top": y = -1
            case "span-bottom": y = 1
            default: break
            }
        }
        return (x, y)
    }
}
