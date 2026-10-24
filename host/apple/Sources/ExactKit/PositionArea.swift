// LLP 1021 D2, §5: where a painted popover sits against the invoker that
// opened it — CSS `position-area` with the invoker as its implicit anchor, in
// the subset the contract admits — the same on iOS's and macOS's top layers,
// and the point macOS pops a chooser's NSMenu up at.
import CoreGraphics
#if os(macOS)
import AppKit
#endif

enum PositionArea {
    /// A box's resolved margins, in points.
    struct Margins: Equatable {
        var top: CGFloat = 0, right: CGFloat = 0, bottom: CGFloat = 0, left: CGFloat = 0
        #if os(macOS)
        /// The margin facing an invoker the box sits beyond `edge` of, in a
        /// flipped view (`minY` above it, `maxY` below, `maxX` right, `minX` left).
        func gap(_ edge: NSRectEdge) -> CGFloat {
            edge == .minY ? bottom : edge == .maxY ? top : edge == .maxX ? left : right
        }
        #endif
    }

    /// The fallbacks the web's UA sheet gives a popover (`neutral.css`:
    /// flip-block, flip-inline, then both).
    static let defaultFallbacks = "flip-block, flip-inline, flip-block flip-inline"

    /// The area and fallbacks a painted popover is placed by (theme2
    /// 2026-10-23 20:05): its author's, and where its author wrote no area the
    /// web's UA default, `block-end span-inline-end` with flips (`neutral.css`),
    /// so the same contract opens alike on the web, iOS and the Mac and a
    /// popover near an edge flips rather than covering its invoker. An authored
    /// `position-try-fallbacks` (`none` included) replaces only the flips. A
    /// chooser's AppKit menu keeps `of` and its own placement.
    static func painted(area: String?, fallbacks: String?, rtl: Bool) -> (area: String, fallbacks: String) {
        guard let area else { return (rtl ? "bottom span-left" : "bottom span-right", fallbacks ?? defaultFallbacks) }
        return (area, fallbacks ?? "none")
    }
    static func painted(_ popover: NodeView, rtl: Bool) -> (area: String, fallbacks: String) {
        painted(area: popover.style["position_area"]?.string, fallbacks: popover.style["position_try_fallbacks"]?.string, rtl: rtl)
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


    /// Where a box goes when no option fits: CSS Box Alignment §4.4.1.2 shifts
    /// it into its overflow limit rect, its containing block together with
    /// the area it was given, the start edge winning when it is too big.
    enum Overflow: Equatable {
        /// The platform keeps it on screen (an `NSMenu`'s launch point).
        case platform
        /// The UA's `position: fixed`: held inside the window.
        case fixed
        /// An authored `position`, which the top layer makes absolute. Its
        /// containing block scrolls, so only the start edges of `page` (the
        /// page's scrollable overflow, in the layer's coordinates) hold it.
        case absolute(page: CGRect)

        var mode: Int { switch self { case .platform: 0; case .fixed: 1; case .absolute: 2 } }
    }

    /// The current option belongs to this opening, like the open state.
    struct History {
        var area = "", fallbacks = "", size = CGSize.zero, margins = Margins(), mode = 1, rtl = false
        var choice = 0
    }

    static func origin(_ area: String, anchor: CGRect, size: CGSize, margins: Margins = .init(),
                       fallbacks: String = "none", overflow: Overflow = .fixed, rtl: Bool = false,
                       in bounds: CGRect) -> CGPoint {
        var history = History()
        return origin(area, anchor: anchor, size: size, margins: margins, fallbacks: fallbacks, overflow: overflow,
                      rtl: rtl, history: &history, in: bounds)
    }

    /// Each attempt starts from the authored area and margins. Centred axes
    /// use CSS's anchor-center overflow alignment. `rtl` is the document's
    /// direction, which decides `flip-start`'s diagonal and the start edges.
    static func origin(_ area: String, anchor: CGRect, size: CGSize, margins: Margins = .init(),
                       fallbacks: String = "none", overflow: Overflow = .fixed, rtl: Bool = false,
                       history: inout History, in bounds: CGRect) -> CGPoint {
        // A change of `position` or direction is a fallback-sensitive change (§6.5.1).
        if history.area != area || history.fallbacks != fallbacks || history.size != size || history.margins != margins
            || history.mode != overflow.mode || history.rtl != rtl {
            history = History(area: area, fallbacks: fallbacks, size: size, margins: margins, mode: overflow.mode, rtl: rtl)
        }
        let axes = axes(area)
        struct Option { var point: CGPoint, box: CGRect, x: Int, y: Int }
        func attempt(_ x: Int, _ y: Int, _ m: Margins) -> Option {
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
            return Option(point: CGPoint(x: point.x + m.left, y: point.y + m.top), box: CGRect(origin: point, size: outer), x: x, y: y)
        }
        let options = ["none"] + fallbacks.split(separator: ",").map(String.init)
        // Each flip transforms what the one before it left (§6.1).
        // `flip-start` mirrors across the diagonal from the start-start
        // corner (top-left, or top-right in a right-to-left document); the
        // width and height it would also exchange are the laid-out size here.
        func selected(_ index: Int) -> Option {
            var (x, y, m) = (axes.x, axes.y, margins)
            for word in options[index].split(whereSeparator: { $0.isWhitespace }) {
                switch word {
                case "flip-block": y = -y; swap(&m.top, &m.bottom)
                case "flip-inline": x = -x; swap(&m.left, &m.right)
                case "flip-start" where rtl:
                    (x, y) = (-y, -x); (m.top, m.right, m.bottom, m.left) = (m.right, m.top, m.left, m.bottom)
                case "flip-start": swap(&x, &y); (m.top, m.left, m.bottom, m.right) = (m.left, m.top, m.right, m.bottom)
                default: break
                }
            }
            return attempt(x, y, m)
        }
        let current = selected(history.choice)
        if bounds.contains(current.box) || area == "none" { return current.point }
        for index in options.indices where index != history.choice {
            let candidate = selected(index)
            if bounds.contains(candidate.box) {
                history.choice = index
                return candidate.point
            }
        }
        let block: CGRect
        switch overflow {
        case .platform: return current.point
        case .fixed: block = bounds
        case .absolute(let page): block = page
        }
        // One axis of the limit: the containing block with the option's area,
        // whose grid's outer lines reach the anchor where it is past them (§3.1.1).
        func limit(_ side: Int, _ low: CGFloat, _ high: CGFloat, _ block: (CGFloat, CGFloat), startHigh: Bool) -> (CGFloat, CGFloat) {
            let (min, max) = (Swift.min(block.0, low), Swift.max(block.1, high))
            let area: (CGFloat, CGFloat) = switch side {
            case -2: (min, low)
            case -1: (min, high)
            case 1: (low, max)
            case 2: (high, max)
            default: (min, max)
            }
            var range = (Swift.min(block.0, area.0), Swift.max(block.1, area.1))
            if case .absolute = overflow {
                if startHigh { range.0 = -.infinity } else { range.1 = .infinity }
            }
            return range
        }
        func shifted(_ at: CGFloat, _ length: CGFloat, _ range: (CGFloat, CGFloat), startHigh: Bool) -> CGFloat {
            startHigh ? min(range.1 - length, max(at, range.0)) : max(range.0, min(at, range.1 - length))
        }
        let box = current.box
        let x = shifted(box.minX, box.width, limit(current.x, anchor.minX, anchor.maxX, (block.minX, block.maxX), startHigh: rtl), startHigh: rtl)
        let y = shifted(box.minY, box.height, limit(current.y, anchor.minY, anchor.maxY, (block.minY, block.maxY), startHigh: false), startHigh: false)
        return CGPoint(x: current.point.x + x - box.minX, y: current.point.y + y - box.minY)
    }

    /// How `popover` overflows: an authored `position` (the row is sent only
    /// when set) computes to absolute in the top layer (CSS Positioned Layout 4,
    /// "Top Layer Styling"); otherwise it is the UA's `fixed`.
    static func overflow(of popover: NodeView, page: @autoclosure () -> CGRect) -> Overflow {
        popover.style["position_type"] == nil ? .fixed : .absolute(page: page())
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
