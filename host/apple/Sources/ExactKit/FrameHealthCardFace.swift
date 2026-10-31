import CoreGraphics
import CoreText
import Foundation

/// The card's face above its chart area, drawn by hand on one opaque surface so nothing in it
/// blends (LLP 1079.000 §7.2). iOS and macOS share this layout and drawing; each maps its own
/// touches or clicks to `Layout`'s regions and gives each region an accessibility element.
enum FrameHealthCardFace {
    enum Status { case live, paused, idle }
    enum Control: Equatable { case close, title, status, copy, charts, table }

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
        var copied = false
        var pressed: Control?
        var hovered: Control?
    }

    static let width: CGFloat = 320, height: CGFloat = 416
    /// Narrower, the header's title, status and buttons would overlap
    static let minimumWidth: CGFloat = 280
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
        /// Close and Copy sit in the card's top corners
        var close: CGRect { CGRect(x: 6, y: 6, width: 32, height: 32) }
        var copy: CGRect { CGRect(x: width - 38, y: 6, width: 32, height: 32) }
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

        /// The control under a point, with each button's 44-point touch target.
        func control(at point: CGPoint) -> Control? {
            func target(_ rect: CGRect) -> CGRect { rect.insetBy(dx: min(0, (rect.width - 44) / 2), dy: min(0, (rect.height - 44) / 2)) }
            if target(copy).contains(point) { return .copy }
            if target(close).contains(point) { return .close }
            if target(title).contains(point) { return .title }
            if target(status).contains(point) { return .status }
            if compact { return nil }
            if target(segments.0).contains(point) { return .charts }
            if target(segments.1).contains(point) { return .table }
            return nil
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

    /// Draw the header, context, figures, pacing and, unless compact, the toolbar into `width` × 206 points.
    static func draw(_ ctx: CGContext, width: CGFloat, compact: Bool, state: State, palette p: FrameHealthCardDrawing.Palette) {
        let layout = Layout(width: width, compact: compact)
        ctx.setFillColor(p.background)
        ctx.fill(CGRect(x: 0, y: 0, width: width, height: 206))
        typealias D = FrameHealthCardDrawing
        for (control, rect) in [(Control.close, layout.close), (.title, layout.title), (.copy, layout.copy)]
        where state.pressed == control || state.hovered == control {
            ctx.setFillColor(state.pressed == control ? p.iconPressed : p.iconHover)
            ctx.addPath(CGPath(roundedRect: rect, cornerWidth: 8, cornerHeight: 8, transform: nil))
            ctx.fillPath()
        }
        Symbol.close.draw(ctx, in: layout.close.insetBy(dx: 6.6, dy: 6.6), color: p.secondary)
        D.text(ctx, "Frame health", x: layout.title.minX + 4, y: 22, title, p.label, .left)
        (state.copied ? Symbol.copied : Symbol.copy).draw(ctx, in: layout.copy.insetBy(dx: 6.6, dy: 6.6), color: p.secondary)
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

    private static func separator(_ ctx: CGContext, y: CGFloat, width: CGFloat, _ p: FrameHealthCardDrawing.Palette) {
        separator(ctx, y: y, from: 12, to: width - 12, p)
    }

    private static func separator(_ ctx: CGContext, y: CGFloat, from: CGFloat, to: CGFloat, _ p: FrameHealthCardDrawing.Palette) {
        ctx.setFillColor(p.separator)
        ctx.fill(CGRect(x: from, y: y, width: to - from, height: 1))
    }

    /// Exact's symbol roles as 24-unit paths, stroked at 1.85 with round caps and joins.
    enum Symbol {
        case live, paused, close, copy, copied

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
            case .copy:
                lines([(8, 7), (8, 3), (15, 3), (20, 8), (20, 17), (15, 17)])
                lines([(15, 3), (15, 8), (20, 8)])
                lines([(4, 7), (11, 7), (15, 11), (15, 21), (4, 21), (4, 7)])
                lines([(11, 7), (11, 11), (15, 11)])
            case .copied: lines([(5, 12), (10, 17), (19, 7)])
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

