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
        /// The anchor's scroll when its option was chosen (`place`): CSS's remembered scroll offset;
        /// and a scrolling page's own then, where that option is aligned against its start edges.
        var scroll: CGPoint?, pageScroll: CGPoint?
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
            let (x, y, m) = flipped(options[index], axes, margins, rtl: rtl)
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
        return overflowed(current.box, point: current.point, x: current.x, y: current.y, anchor: anchor, overflow: overflow, rtl: rtl, in: bounds)
    }

    /// Where a box sized in each option's region goes, and that region (LLP
    /// 1021 D2; the iOS lift and the Mac's painted popover). CSS sizes a box
    /// placed by `position-area` in its inset-modified containing block: the
    /// option's cell of the grid the anchor's edges make with the containing
    /// block's, whose outer lines reach the anchor where it is past them. An
    /// automatic width is `fit-content` there, min-content winning, and the
    /// height its content's. An option fits when its margin box lies inside
    /// its region (inside the containing block on an anchor-centred axis), so
    /// a box that narrows to fit keeps its place where one laid out once at
    /// the viewport's size would flip (Chrome, the plan's table). `measure`
    /// lays the box out in a region that size and answers its border box and
    /// margins as that layout resolved them (`Runtime.measureBlock`); each
    /// option flips those margins as it flips the area. The caller gives the
    /// chosen region as the box's block, so it is laid out at the size it was
    /// placed at. When no option fits, the history's option is aligned as
    /// `origin` aligns it. `margins` are the authored ones, which with the
    /// box's size in the whole block (which no option changes) keep the
    /// history as `origin` keeps it.
    ///
    /// `flip-start` exchanges the margins, not the width and height, and the
    /// box is laid out with its authored margins: its border box is the one
    /// its region gives with those (the plan's stated limit).
    ///
    /// `scroll` is how far the scrollers between the anchor and the block are
    /// scrolled (`Presenter.anchorScroll`). The chosen option keeps the scroll
    /// it was chosen at, as CSS remembers it (css-anchor-position-1 §5.2):
    /// its region and size are the anchor's there, and a scroll since moves
    /// the box with the anchor without sizing it again. It stays while that
    /// moved box fits the region the anchor makes now; when it doesn't, the
    /// other options are tried where the anchor is, and the one chosen
    /// remembers this scroll. `pageScroll` is an `.absolute` page's own
    /// scroll, remembered with it, so an option is aligned against the page's
    /// start edges where they were (its extent the page's now).
    static func place(_ area: String, anchor: CGRect, margins: Margins = .init(), fallbacks: String = "none",
                      overflow: Overflow = .fixed, rtl: Bool = false, scroll: CGPoint = .zero, pageScroll: CGPoint = .zero, history: inout History,
                      in bounds: CGRect, measure: (CGSize) -> Measured) -> Placement {
        // As `origin`'s, its size the box's in the whole block: a box that grows or shrinks starts again.
        let whole = measure(bounds.size).size
        if history.area != area || history.fallbacks != fallbacks || history.size != whole || history.margins != margins
            || history.mode != overflow.mode || history.rtl != rtl {
            history = History(area: area, fallbacks: fallbacks, size: whole, margins: margins, mode: overflow.mode, rtl: rtl)
        }
        if history.scroll == nil { history.pageScroll = pageScroll }
        let remembered = history.scroll ?? scroll
        let shift = CGPoint(x: scroll.x - remembered.x, y: scroll.y - remembered.y)
        // `none` is no fallback: the base alone, not the base twice.
        let tries = fallbacks.split(separator: ",").map { $0.trimmingCharacters(in: .whitespaces) }.filter { $0 != "none" }
        let axes = axes(area), options = ["none"] + tries
        struct Option { var placement: Placement, box: CGRect, x: Int, y: Int }
        func option(_ index: Int, at anchor: CGRect) -> Option {
            let (x, y, _) = flipped(options[index], axes, margins, rtl: rtl)
            let region = Self.region(x, y, anchor: anchor, in: bounds)
            let measured = measure(region.size), size = measured.size
            let m = flipped(options[index], axes, measured.margins, rtl: rtl).2
            let outer = CGSize(width: size.width + m.left + m.right, height: size.height + m.top + m.bottom)
            let corner = CGPoint(x: at(x, anchor.minX, anchor.maxX, outer.width, bounds.minX, bounds.maxX),
                                 y: at(y, anchor.minY, anchor.maxY, outer.height, bounds.minY, bounds.maxY))
            let origin = CGPoint(x: corner.x + m.left, y: corner.y + m.top)
            return Option(placement: Placement(origin: origin, size: size, region: region), box: CGRect(origin: corner, size: outer), x: x, y: y)
        }
        // A centred cell's box may overflow its cell, as anchor-center lets it: it fits inside the containing block.
        func fits(_ o: Option) -> Bool {
            let slack: CGFloat = 0.01
            func axis(_ side: Int, _ low: CGFloat, _ high: CGFloat, _ region: (CGFloat, CGFloat), _ block: (CGFloat, CGFloat)) -> Bool {
                let range = abs(side) == 3 ? block : region
                return low >= range.0 - slack && high <= range.1 + slack
            }
            let r = o.placement.region
            return axis(o.x, o.box.minX, o.box.maxX, (r.minX, r.maxX), (bounds.minX, bounds.maxX))
                && axis(o.y, o.box.minY, o.box.maxY, (r.minY, r.maxY), (bounds.minY, bounds.maxY))
        }
        // The chosen option where its scroll had the anchor; the scroll since moves it, last (§5.2).
        let then = anchor.offsetBy(dx: shift.x, dy: shift.y), current = option(history.choice, at: then)
        func moved(_ p: Placement) -> Placement {
            Placement(origin: CGPoint(x: p.origin.x - shift.x, y: p.origin.y - shift.y), size: p.size,
                      region: p.region.offsetBy(dx: -shift.x, dy: -shift.y))
        }
        var test = current
        test.box = current.box.offsetBy(dx: -shift.x, dy: -shift.y)
        test.placement.region = Self.region(current.x, current.y, anchor: anchor, in: bounds)
        history.scroll = remembered
        if fits(test) { return moved(current.placement) }
        for index in options.indices where index != history.choice {
            let candidate = option(index, at: anchor)
            if fits(candidate) {
                history.choice = index
                (history.scroll, history.pageScroll) = (scroll, pageScroll)
                return candidate.placement
            }
        }
        // Aligned where it was chosen: a scrolling page's start edges where its scroll had them then.
        var placed = current.placement, limit = overflow
        if case .absolute(let page) = overflow, let then = history.pageScroll {
            limit = .absolute(page: page.offsetBy(dx: pageScroll.x - then.x, dy: pageScroll.y - then.y))
        }
        placed.origin = overflowed(current.box, point: placed.origin, x: current.x, y: current.y, anchor: then, overflow: limit, rtl: rtl, in: bounds)
        return moved(placed)
    }

    /// What `place`'s `measure` answers for a region: the box's border box,
    /// and its margins as the layout resolved them (a percentage is the
    /// region's).
    struct Measured: Equatable {
        var size: CGSize, margins = Margins()
    }

    /// Where `place` put a box: its border box's origin and size, and the
    /// region (the option's inset-modified containing block) it was sized in.
    struct Placement: Equatable {
        var origin: CGPoint, size: CGSize, region: CGRect
    }

    /// An option's region: on each axis the grid's band its side names, the
    /// outer lines reaching the anchor where it is past the block (§3.1.1).
    static func region(_ x: Int, _ y: Int, anchor: CGRect, in block: CGRect) -> CGRect {
        func band(_ side: Int, _ low: CGFloat, _ high: CGFloat, _ start: CGFloat, _ end: CGFloat) -> (CGFloat, CGFloat) {
            let (min, max) = (Swift.min(start, low), Swift.max(end, high))
            switch side {
            case -2: return (min, low)
            case -1: return (min, high)
            case 1: return (low, max)
            case 2: return (high, max)
            case 3, -3: return (low, high)
            default: return (min, max)
            }
        }
        let h = band(x, anchor.minX, anchor.maxX, block.minX, block.maxX)
        let v = band(y, anchor.minY, anchor.maxY, block.minY, block.maxY)
        return CGRect(x: h.0, y: v.0, width: max(0, h.1 - h.0), height: max(0, v.1 - v.0))
    }

    /// Where a box of `length` starts on one axis: outside the anchor (±2),
    /// spanning from its edge (±1), or anchor-centred inside the block (0, ±3).
    private static func at(_ side: Int, _ low: CGFloat, _ high: CGFloat, _ length: CGFloat, _ min: CGFloat, _ max: CGFloat) -> CGFloat {
        switch side {
        case -2: return low - length
        case -1: return high - length
        case 1: return low
        case 2: return high
        default: return Swift.max(min, Swift.min((low + high - length) / 2, max - length))
        }
    }

    /// An option's axes and margins: each flip transforms what the one before
    /// it left (§6.1). `flip-start` mirrors across the diagonal from the
    /// start-start corner (top-left, or top-right in a right-to-left document).
    private static func flipped(_ option: String, _ axes: (x: Int, y: Int), _ margins: Margins, rtl: Bool) -> (Int, Int, Margins) {
        var (x, y, m) = (axes.x, axes.y, margins)
        for word in option.split(whereSeparator: { $0.isWhitespace }) {
            switch word {
            case "flip-block": y = -y; swap(&m.top, &m.bottom)
            case "flip-inline": x = -x; swap(&m.left, &m.right)
            case "flip-start" where rtl:
                (x, y) = (-y, -x); (m.top, m.right, m.bottom, m.left) = (m.right, m.top, m.left, m.bottom)
            case "flip-start": swap(&x, &y); (m.top, m.left, m.bottom, m.right) = (m.left, m.top, m.right, m.bottom)
            default: break
            }
        }
        return (x, y, m)
    }

    /// The history's option when none fits: CSS Box Alignment §4.4.1.2
    /// shifts it into its overflow limit rect, the containing block with the
    /// option's area, the start edge winning when it is too big.
    private static func overflowed(_ box: CGRect, point: CGPoint, x side: Int, y down: Int, anchor: CGRect,
                                   overflow: Overflow, rtl: Bool, in bounds: CGRect) -> CGPoint {
        let block: CGRect
        switch overflow {
        case .platform: return point
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
        let x = shifted(box.minX, box.width, limit(side, anchor.minX, anchor.maxX, (block.minX, block.maxX), startHigh: rtl), startHigh: rtl)
        let y = shifted(box.minY, box.height, limit(down, anchor.minY, anchor.maxY, (block.minY, block.maxY), startHigh: false), startHigh: false)
        return CGPoint(x: point.x + x - box.minX, y: point.y + y - box.minY)
    }

    /// How `popover` overflows: an authored `position` (the row is sent only
    /// when set) computes to absolute in the top layer (CSS Positioned Layout 4,
    /// "Top Layer Styling"); otherwise it is the UA's `fixed`.
    static func overflow(of popover: NodeView, page: @autoclosure () -> CGRect) -> Overflow {
        popover.style["position_type"] == nil ? .fixed : .absolute(page: page())
    }

    /// Where `popover`'s authored `left` and `top` put its border box, from
    /// the viewport's top-left (`clientX`/`clientY`'s space, LLP 1021 D2):
    /// past its margin, as CSS places a fixed box by its insets; an axis with
    /// none at its margin. Nil when it writes neither, and its invoker places
    /// it; nil too under an authored `position-area`, which CSS lets win (the
    /// insets then offset the box in its cell, which no host here does). A
    /// percentage is of the viewport, `viewport` big.
    static func inset(of popover: NodeView, viewport: CGSize) -> CGPoint? {
        guard of(popover) == "none" else { return nil }
        let left = popover.style["left"]?.length(basis: viewport.width)
        let top = popover.style["top"]?.length(basis: viewport.height)
        guard left != nil || top != nil else { return nil }
        let m = margins(of: popover)
        return CGPoint(x: (left ?? 0) + m.left, y: (top ?? 0) + m.top)
    }

    /// The top-left of a box `size` big asked for at `point` in `bounds`:
    /// there, or on an axis where that leaves too little room the other way
    /// from the point, as the web's sheet opens one (neutral.css's
    /// `--exact-inset-start` and `-up`, a context menu's too); held in
    /// `bounds` where neither way fits, as a painted layer is in the window.
    static func at(_ point: CGPoint, size: CGSize, in bounds: CGRect) -> CGPoint {
        func axis(_ at: CGFloat, _ length: CGFloat, _ low: CGFloat, _ high: CGFloat) -> CGFloat {
            if at + length <= high { return max(low, at) }
            if at - length >= low { return at - length }
            return max(low, min(at, high - length))
        }
        return CGPoint(x: axis(point.x, size.width, bounds.minX, bounds.maxX),
                       y: axis(point.y, size.height, bounds.minY, bounds.maxY))
    }


    // -2/2: outside the anchor; -1/1: span toward that edge; 0: the axis
    // unsaid, spanning the block (`bottom` is `bottom span-all`), and 3 the
    // anchor's own cell (`center`), each anchor-centred.
    private static func axes(_ area: String) -> (x: Int, y: Int) {
        if area == "none" { return (1, 2) }
        if area == "center" { return (3, 3) }
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
