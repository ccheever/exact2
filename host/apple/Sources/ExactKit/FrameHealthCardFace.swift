import CoreGraphics
import CoreText
import Foundation

/// The card's face above its chart area, drawn by hand on one opaque surface so nothing in it
/// blends (LLP 1079.000 §7.2). iOS and macOS share this layout and drawing; each maps its own
/// touches or clicks to `Layout`'s regions and gives each region an accessibility element.
enum FrameHealthCardFace {
    enum Status { case live, paused, idle }
    enum Control: Equatable { case close, title, status, details, share, charts, table }

    /// What the face shows; the platform view fills it from the reader on each refresh.
    struct State {
        var status = Status.idle
        var request = ("No rate requested", "")
        var display = ""
        /// Two rows, each a window's name, frame rate in whole frames per second, and missed frames
        var rows = [("Last 0.5s", "—", "—"), ("Last 5s", "—", "—")]
        var pacing = ("—", "")
        /// The window pacing covers: "last 5s", or the span it actually covers
        var pacingWindow = "last 5s"
        var table = false
        /// Details shows in place of everything below the header
        var showsDetails = false
        var pressed: Control?
        var hovered: Control?
    }

    static let width: CGFloat = 320, height: CGFloat = 416
    /// Narrower, the header's title, status and buttons would overlap
    static let minimumWidth: CGFloat = 304
    /// Without the toolbar and chart area, where the card would not fit
    static let compactHeight: CGFloat = 176

    static let body = CTFontCreateUIFontForLanguage(.system, 12, nil)!
    static let digits = FrameHealthCardDrawing.tabular(body)
    static let strong = FrameHealthCardDrawing.tabular(FrameHealthCardDrawing.semibold(12))
    static let title = FrameHealthCardDrawing.semibold(15)

    /// Every region at a card width; sizes are points.
    struct Layout {
        let width: CGFloat
        /// Without the toolbar and chart area
        var compact = false
        /// With Details in place of everything below the header
        var showsDetails = false
        /// Close and Share sit in the card's top corners, and Details beside Share
        var close: CGRect { CGRect(x: 6, y: 6, width: 32, height: 32) }
        var share: CGRect { CGRect(x: width - 38, y: 6, width: 32, height: 32) }
        var details: CGRect { CGRect(x: width - 74, y: 6, width: 32, height: 32) }
        /// The title is the definitions button, with 4 points of fill on either side of its text
        var title: CGRect { CGRect(x: 40, y: 6, width: ceil(FrameHealthCardDrawing.width("Frame health", FrameHealthCardFace.title)) + 8, height: 32) }
        var status: CGRect { CGRect(x: title.maxX + 4, y: 8.5, width: 79, height: 27) }
        var segments: (CGRect, CGRect) {
            let charts = ceil(FrameHealthCardDrawing.width("Charts", body)) + 18, table = ceil(FrameHealthCardDrawing.width("Table", body)) + 18
            return (CGRect(x: 15, y: 175, width: charts, height: 21), CGRect(x: 15 + charts, y: 175, width: table, height: 21))
        }
        var track: CGRect { let (a, b) = segments; return a.union(b).insetBy(dx: -3, dy: -3) }
        var chartArea: CGRect { CGRect(x: 12, y: 206, width: width - 24, height: 200) }
        /// Rows that change at each refresh, through the toolbar, whose chosen segment can change
        var figures: CGRect { CGRect(x: 0, y: 44, width: width, height: 162) }
        /// Everything below the header, where Details shows; its rows scroll within it
        var detailsArea: CGRect { CGRect(x: 0, y: 44, width: width, height: (compact ? compactHeight : height) - 44) }

        /// The control under a point, with each button's 44-point touch target; where two targets
        /// overlap, the nearer button.
        func control(at point: CGPoint) -> Control? {
            func target(_ rect: CGRect) -> CGRect { rect.insetBy(dx: min(0, (rect.width - 44) / 2), dy: min(0, (rect.height - 44) / 2)) }
            func distance(_ rect: CGRect) -> CGFloat { max(rect.minX - point.x, point.x - rect.maxX, 0) + max(rect.minY - point.y, point.y - rect.maxY, 0) }
            var buttons = [(Control.close, close), (.title, title), (.status, status), (.details, details), (.share, share)]
            if !compact, !showsDetails { buttons += [(.charts, segments.0), (.table, segments.1)] }
            return buttons.filter { target($0.1).contains(point) }.min { distance($0.1) < distance($1.1) }?.0
        }
    }

    /// A press on the card: anywhere on it, a button included, it drags once it moves 4 points, and
    /// one that does not move is a tap. Points are in the card's container.
    struct Drag {
        let start: CGPoint, origin: CGPoint, control: Control?
        var moved = false
        /// The latest point the press moved to, and when
        private var last: (CGPoint, TimeInterval)
        /// Points a second, into the latest point
        private var velocity = CGVector.zero
        init(at point: CGPoint, time: TimeInterval, origin: CGPoint, control: Control?) {
            (start, self.origin, self.control, last) = (point, origin, control, (point, time))
        }
        mutating func track(_ point: CGPoint, at time: TimeInterval) {
            let elapsed = time - last.1
            guard elapsed > 0, point != last.0 else { return }
            velocity = CGVector(dx: (point.x - last.0.x) / elapsed, dy: (point.y - last.0.y) / elapsed)
            last = (point, time)
        }
        /// The velocity a release at `time` carries: a press held still for 50 ms has none.
        func velocity(releasedAt time: TimeInterval) -> CGVector { time - last.1 > 0.05 ? .zero : velocity }
    }

    /// The spring that carries a released card to its corner: a 0.4 s response, damped to 0.8 of
    /// critical, with a unit mass
    static let spring = (stiffness: pow(2 * CGFloat.pi / 0.4, 2), damping: 2 * 0.8 * (2 * CGFloat.pi / 0.4))

    /// The corner of an area the card rests in, in that area's coordinates.
    struct Corner: Equatable {
        var maxX = false, maxY = false
        init(maxX: Bool = false, maxY: Bool = false) { (self.maxX, self.maxY) = (maxX, maxY) }
        /// The corner nearest where a card released at `origin` and `velocity` would come to rest
        /// under the platform's normal deceleration, as Picture in Picture chooses its corner
        /// ([Designing Fluid Interfaces](https://developer.apple.com/videos/play/wwdc2018/803/)).
        init(nearest origin: CGPoint, velocity: CGVector, size: CGSize, in area: CGRect) {
            // `UIScrollView.DecelerationRate.normal` keeps 0.998 of the velocity each millisecond
            let glide = 0.998 / (1 - 0.998) / 1000
            maxX = origin.x + size.width / 2 + velocity.dx * glide > area.midX
            maxY = origin.y + size.height / 2 + velocity.dy * glide > area.midY
        }
        func origin(of size: CGSize, in area: CGRect) -> CGPoint {
            CGPoint(x: maxX ? max(area.minX, area.maxX - size.width) : area.minX,
                y: maxY ? max(area.minY, area.maxY - size.height) : area.minY)
        }
    }

    /// Where a dragged card is drawn when the press would carry its origin to `origin`: there, within
    /// `area`, and past an edge only part of the way, following the press less the further it goes, as
    /// a scroll view resists past its content. On release the card springs back to a corner of `area`.
    static func resisted(_ origin: CGPoint, size: CGSize, in area: CGRect) -> CGPoint {
        func axis(_ value: CGFloat, _ low: CGFloat, _ high: CGFloat, _ span: CGFloat) -> CGFloat {
            let high = max(low, high)
            // Past an edge by `x`, the card goes (1 - 1 / (0.55 x / span + 1)) span, never a whole span
            func resist(_ x: CGFloat) -> CGFloat { (1 - 1 / (0.55 * x / span + 1)) * span }
            return value < low ? low - resist(low - value) : value > high ? high + resist(value - high) : value
        }
        return CGPoint(x: axis(origin.x, area.minX, area.maxX - size.width, area.width),
            y: axis(origin.y, area.minY, area.maxY - size.height, area.height))
    }

    /// Draw the header, context, figures, pacing and, unless compact, the toolbar into `width` × 206 points.
    static func draw(_ ctx: CGContext, width: CGFloat, compact: Bool, state: State, palette p: FrameHealthCardDrawing.Palette) {
        let layout = Layout(width: width, compact: compact)
        ctx.setFillColor(p.background)
        ctx.fill(CGRect(x: 0, y: 0, width: width, height: 206))
        typealias D = FrameHealthCardDrawing
        for (control, rect) in [(Control.close, layout.close), (.title, layout.title), (.details, layout.details), (.share, layout.share)] {
            // Details keeps the status's fill while it shows, as a toggled button does
            let shown = control == .details && state.showsDetails
            let fill = state.pressed == control ? shown ? p.statusPressed : p.iconPressed
                : state.hovered == control ? shown ? p.statusHover : p.iconHover : shown ? p.statusFill : nil
            guard let fill else { continue }
            ctx.setFillColor(fill)
            ctx.addPath(CGPath(roundedRect: rect, cornerWidth: 8, cornerHeight: 8, transform: nil))
            ctx.fillPath()
        }
        Symbol.close.draw(ctx, in: layout.close.insetBy(dx: 6.6, dy: 6.6), color: p.secondary)
        D.text(ctx, "Frame health", x: layout.title.minX + 4, y: 22, title, p.label, .left)
        Symbol.details.draw(ctx, in: layout.details.insetBy(dx: 6.6, dy: 6.6), color: p.secondary)
        Symbol.share.draw(ctx, in: layout.share.insetBy(dx: 6.6, dy: 6.6), color: p.secondary)
        // The status is the pause button: its symbol and word say the state
        let status = layout.status
        ctx.setFillColor(state.pressed == .status ? p.statusPressed : state.hovered == .status ? p.statusHover : p.statusFill)
        ctx.addPath(CGPath(roundedRect: status, cornerWidth: status.height / 2, cornerHeight: status.height / 2, transform: nil))
        ctx.fillPath()
        let symbol = CGRect(x: status.minX + 9, y: status.midY - 6.6, width: 13.2, height: 13.2)
        switch state.status {
        case .paused: Symbol.paused.draw(ctx, in: symbol, color: p.gray)
        case .live: Symbol.live.draw(ctx, in: symbol, color: p.green)
        case .idle: Symbol.live.draw(ctx, in: symbol, color: p.gray)
        }
        let word = state.status == .paused ? "Paused" : state.status == .live ? "Live" : "Idle"
        D.text(ctx, word, x: symbol.maxX + 4.8, y: status.midY, strong, p.label, .left)
        guard !state.showsDetails else { return }
        // Context: the request and any system limit, then the display
        let request = state.request.0 + (state.request.1.isEmpty ? "" : " ")
        D.text(ctx, request, x: 12, y: 52, body, p.label, .left, limit: width - 24)
        if !state.request.1.isEmpty {
            D.text(ctx, "· " + state.request.1, x: 12 + D.width(request, body), y: 52, body, p.secondary, .left)
        }
        D.text(ctx, state.display, x: 12, y: 68, body, p.secondary, .left, limit: width - 24)
        separator(ctx, y: 80, width: width, p)
        // One row per window, digits right-aligned in reserved tabular columns so nothing moves
        let digits = D.width("000", strong), count = D.width("0000", strong)
        for (index, row) in state.rows.enumerated() {
            let y = 94 + CGFloat(index) * 18
            D.text(ctx, row.0, x: 12, y: y, body, p.secondary, .left)
            D.text(ctx, row.1, x: 78 + digits, y: y, strong, p.label, .right)
            D.text(ctx, "FPS", x: 78 + digits + 3.6, y: y, body, p.label, .left)
            D.text(ctx, row.2, x: 162 + count, y: y, strong, p.label, .right)
            D.text(ctx, "missed", x: 162 + count + 3.6, y: y, body, p.label, .left)
        }
        separator(ctx, y: 125, width: width, p)
        let label = "Pacing (\(state.pacingWindow))", verdict = 12 + D.width(label, body) + 3.6
        D.text(ctx, label, x: 12, y: 138, body, p.secondary, .left)
        D.text(ctx, state.pacing.0, x: verdict, y: 138, strong, p.label, .left, limit: width - 12 - verdict)
        D.text(ctx, state.pacing.1, x: 12, y: 154, Self.digits, p.label, .left, limit: width - 24)
        guard !compact else { return }
        // The segmented control: a track, and the chosen segment a chip of the card's ground
        let track = layout.track
        ctx.setFillColor(p.track)
        ctx.addPath(CGPath(roundedRect: track, cornerWidth: 8, cornerHeight: 8, transform: nil))
        ctx.fillPath()
        let (charts, table) = layout.segments
        for (rect, name, control, chosen) in [(charts, "Charts", Control.charts, !state.table), (table, "Table", .table, state.table)] {
            // The chosen segment is a chip of the card's ground; the other washes when hovered or pressed
            let fill = chosen ? p.chip : state.pressed == control ? p.statusPressed : state.hovered == control ? p.statusHover : nil
            if let fill {
                ctx.setFillColor(fill)
                ctx.addPath(CGPath(roundedRect: rect, cornerWidth: 6, cornerHeight: 6, transform: nil))
                ctx.fillPath()
            }
            let w = D.width(name, body)
            D.text(ctx, name, x: rect.midX - w / 2, y: rect.midY, body, chosen || state.hovered == control ? p.label : p.secondary, .left)
        }
    }

    /// The table: for the last 5s and 30s, the figures the face does not show, then the deadline.
    static func table(_ ctx: CGContext, _ rect: CGRect, columns: [String], rows: [(String, [String])], caption: String,
                      palette p: FrameHealthCardDrawing.Palette) {
        typealias D = FrameHealthCardDrawing
        ctx.setFillColor(p.background)
        ctx.fill(rect)
        let first = rect.width * 0.46, column = (rect.width - first) / CGFloat(max(1, columns.count))
        var y = rect.minY
        for (index, row) in ([("", columns)] + rows).enumerated() {
            D.text(ctx, row.0, x: rect.minX, y: y + 12.5, body, p.secondary, .left)
            for (i, value) in row.1.enumerated() {
                D.text(ctx, value, x: rect.minX + first + CGFloat(i + 1) * column, y: y + 12.5, index == 0 ? body : strong,
                    index == 0 ? p.secondary : p.label, .right)
            }
            y += 25
            separator(ctx, y: y - 1, from: rect.minX, to: rect.maxX, p)
        }
        D.text(ctx, caption, x: rect.minX, y: y + 6 + 8, body, p.secondary, .left, limit: rect.width)
    }

    /// Details' rows, then its caption, from the top of a surface `width` points wide: a result with its
    /// reason under it takes 36 points, a count 25.
    static func details(_ ctx: CGContext, width: CGFloat, rows: [FrameHealthCardText.Detail], caption: String,
                        palette p: FrameHealthCardDrawing.Palette) {
        typealias D = FrameHealthCardDrawing
        var y: CGFloat = 0
        for row in rows {
            let height = height(row), right = width - 12
            D.text(ctx, row.name, x: 12, y: y + height / 2, body, p.secondary, .left)
            let name = 12 + D.width(row.name, body) + 12
            if let reason = row.reason {
                D.text(ctx, row.value, x: right, y: y + 11, strong, p.label, .right, limit: right - name)
                D.text(ctx, reason, x: right, y: y + 25, body, p.secondary, .right, limit: right - name)
            } else {
                D.text(ctx, row.value, x: right, y: y + height / 2, strong, p.label, .right, limit: right - name)
            }
            y += height
            separator(ctx, y: y - 1, width: width, p)
        }
        for line in D.lines(caption, width: width - 24, body) {
            y += 16
            D.text(ctx, line, x: 12, y: y, body, p.secondary, .left)
        }
    }

    /// The height Details' rows and caption take, which scroll where it is more than the area's.
    static func detailsHeight(width: CGFloat, rows: [FrameHealthCardText.Detail], caption: String) -> CGFloat {
        rows.reduce(0) { $0 + height($1) } + CGFloat(FrameHealthCardDrawing.lines(caption, width: width - 24, body).count) * 16 + 10
    }

    private static func height(_ row: FrameHealthCardText.Detail) -> CGFloat { row.reason == nil ? 25 : 36 }

    private static func separator(_ ctx: CGContext, y: CGFloat, width: CGFloat, _ p: FrameHealthCardDrawing.Palette) {
        separator(ctx, y: y, from: 12, to: width - 12, p)
    }

    private static func separator(_ ctx: CGContext, y: CGFloat, from: CGFloat, to: CGFloat, _ p: FrameHealthCardDrawing.Palette) {
        ctx.setFillColor(p.separator)
        ctx.fill(CGRect(x: from, y: y, width: to - from, height: 1))
    }

    /// Exact's symbol roles as 24-unit paths, stroked at 1.85 with round caps and joins.
    enum Symbol {
        case live, paused, close, details, share

        func draw(_ ctx: CGContext, in rect: CGRect, color: CGColor) {
            let path = CGMutablePath()
            func lines(_ points: [(CGFloat, CGFloat)]) {
                path.move(to: CGPoint(x: points[0].0, y: points[0].1))
                for point in points.dropFirst() { path.addLine(to: CGPoint(x: point.0, y: point.1)) }
            }
            var solid = false
            switch self {
            case .live: lines([(2, 12), (6, 12), (9, 4), (15, 20), (18, 12), (22, 12)])
            case .paused:
                solid = true
                path.addRect(CGRect(x: 6, y: 4, width: 4, height: 16))
                path.addRect(CGRect(x: 14, y: 4, width: 4, height: 16))
            case .close:
                lines([(6, 6), (18, 18)])
                lines([(18, 6), (6, 18)])
            case .details:
                // Rows of text with a bullet before each, as the system's list.bullet
                for y: CGFloat in [6, 12, 18] {
                    lines([(5, y), (5.01, y)])
                    lines([(9, y), (20, y)])
                }
            case .share:
                // A box open at the top and an arrow up out of it, as the system's square.and.arrow.up
                lines([(8, 9), (5, 9), (5, 21), (19, 21), (19, 9), (16, 9)])
                lines([(12, 3), (12, 15)])
                lines([(8, 7), (12, 3), (16, 7)])
            }
            ctx.saveGState()
            ctx.translateBy(x: rect.minX, y: rect.minY)
            ctx.scaleBy(x: rect.width / 24, y: rect.height / 24)
            ctx.addPath(path)
            if solid {
                ctx.setFillColor(color)
                ctx.fillPath()
            } else {
                ctx.setStrokeColor(color)
                ctx.setLineWidth(1.85)
                ctx.setLineCap(.round)
                ctx.setLineJoin(.round)
                ctx.strokePath()
            }
            ctx.restoreGState()
        }
    }
}

