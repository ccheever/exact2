import Foundation
import QuartzCore

/// The card's charts, fed by each frame the app draws (LLP 1079.000 §7.2). The frame-rate chart
/// plots a moving average over the last half second, and the frame-time chart each frame's own frame
/// time and work, kept as 30 seconds of tenth-of-a-second columns. Each plot's gridlines and lines
/// are one opaque layer, wider than the plot by five seconds of axis. The frame hooks only store each
/// frame; `present()`, after them, moves the layers left by whole device pixels and redraws only the
/// columns that changed, and every five seconds the layers are drawn whole again. Without frames
/// nothing moves or draws. Shared by iOS and macOS.
final class FrameHealthChart {
    /// A tenth of a second of frames.
    struct Column {
        /// Which tenth of a second, counted from the clock's origin; -1 for none
        var index: Int64 = -1
        /// The frame rate's moving average at the column's newest frame; zero without one
        var fps: Float = 0
        /// Milliseconds: the longest frame time, and the slowest work with that frame's budget
        var frame: Float = 0, work: Float = 0, budget: Float = 0
        /// Its first frame continued a run from an earlier column, so its lines reach back to that
        /// column's across any empty columns between
        var joined = false
    }

    static let columnSeconds = 0.1, span = 30.0, reach = 5.0, average = 0.5
    /// Held only while the card reads, from `start()` to `stop()`
    private(set) var columns: [Column] = []
    /// The newest frame's timestamp, at the plots' right edges
    private(set) var newest = 0.0
    /// This run's frame timestamps over the last half second, a ring of `count` from `first`, oldest
    /// first: the moving average's intervals lie between them
    private(set) var times: [Double] = []
    private var first = 0, count = 0
    /// The columns frames changed since the layers last showed them
    private var pending: ClosedRange<Int64>?

    private final class Lines: CALayer {
        weak var chart: FrameHealthChart?
        var which = 0
        override func draw(in ctx: CGContext) { chart?.draw(which, ctx) }
    }
    private let windows = [CALayer(), CALayer()], lines = [Lines(), Lines()]
    /// What the layers show: each shown plot, the scale from its points to the host's, the palette,
    /// and the newest frame's time when they were last drawn whole, at the plots' right edges
    private var plots: [Int: FrameHealthCardDrawing.Plot] = [:]
    private var scale: CGFloat = 1, palette: FrameHealthCardDrawing.Palette?, base = 0.0

    init(in host: CALayer) {
        // No implicit animations: a new picture, place or size shows at once, never faded or stretched from the old
        let still: [String: CAAction] = ["contents": NSNull(), "position": NSNull(), "bounds": NSNull(), "hidden": NSNull()]
        for (index, (window, layer)) in zip(windows, lines).enumerated() {
            window.actions = still
            window.masksToBounds = true
            window.isHidden = true
            layer.actions = still
            layer.isOpaque = true
            layer.anchorPoint = .zero
            layer.chart = self
            layer.which = index
            window.addSublayer(layer)
            host.addSublayer(window)
        }
    }

    // MARK: Frames

    /// Hold 30 seconds of columns and half a second of frames, as the card starts reading.
    func start() {
        guard columns.isEmpty else { return }
        columns = [Column](repeating: Column(), count: Int(Self.span / Self.columnSeconds))
        times = [Double](repeating: 0, count: 256)
    }

    /// Free the frames and the layers' pictures, as the card stops reading.
    func stop() {
        (columns, times, first, count, newest, pending, plots) = ([], [], 0, 0, 0, nil, [:])
        for (window, layer) in zip(windows, lines) {
            window.isHidden = true
            layer.contents = nil
        }
    }

    /// A frame started at `start`; a frame whose work finished in its callback gives `finished`. The
    /// frame hooks call it, so it only stores the frame.
    func frame(at start: Double, finished: Double? = nil, deadline: Double = 0) {
        guard !columns.isEmpty else { return }
        guard start > newest else { endRun(); return }
        let index = Self.index(start), at = slot(index)
        let before = newest > 0 ? Self.index(newest) : index
        // A column 30 seconds old gives its place to this one
        if columns[at].index != index { columns[at] = Column(index: index, joined: count > 0) }
        if count > 0 {
            let previous = times[(first + count - 1) % times.count]
            if count == times.count { (first, count) = ((first + 1) % times.count, count - 1) }
            times[(first + count) % times.count] = start
            count += 1
            // The intervals that ended within the last half second, and at least the newest
            while count > 2, times[(first + 1) % times.count] <= start - Self.average {
                (first, count) = ((first + 1) % times.count, count - 1)
            }
            columns[at].frame = max(columns[at].frame, Float((start - previous) * 1000))
            columns[at].fps = Float(Double(count - 1) / (start - times[first]))
        } else {
            (first, count) = (0, 1)
            times[0] = start
        }
        newest = start
        // Redraw from the line into the previous frame's column: once a later frame begins, that column's lines
        // end at its edge, not at its frame
        changed(lineStart(before)...index)
        if let finished { work(start: start, deadline: deadline, finished: finished) }
    }

    /// A frame's work finished, after its callback or within it.
    func work(start: Double, deadline: Double, finished: Double) {
        guard !columns.isEmpty else { return }
        let index = Self.index(start), at = slot(index)
        guard columns[at].index == index, Float((finished - start) * 1000) > columns[at].work else { return }
        columns[at].work = Float((finished - start) * 1000)
        columns[at].budget = Float((deadline - start) * 1000)
        changed(lineStart(index)...index)
    }

    /// The run ended, as at an idle callback or a stop: the next frame bounds no interval.
    func endRun() { (first, count) = (0, 0) }

    /// Forget every frame, for a new generation.
    func reset() {
        endRun()
        for i in columns.indices { columns[i] = Column() }
        (newest, pending) = (0, nil)
        redraw()
    }

    private static func index(_ time: Double) -> Int64 { Int64((time / columnSeconds).rounded(.down)) }

    private func slot(_ index: Int64) -> Int { Int(index % Int64(columns.count)) }

    /// Where column `index`'s lines begin: at the nearest earlier column that holds frames, within the
    /// 30 seconds kept, when it joins it, and otherwise at itself.
    private func lineStart(_ index: Int64) -> Int64 {
        guard columns[slot(index)].index == index, columns[slot(index)].joined else { return index }
        var i = index - 1
        while i > max(0, index - Int64(columns.count)) {
            if columns[slot(i)].index == i { return i }
            i -= 1
        }
        return index
    }

    private func changed(_ range: ClosedRange<Int64>) {
        pending = pending.map { min($0.lowerBound, range.lowerBound)...max($0.upperBound, range.upperBound) } ?? range
    }

    // MARK: Layers

    /// Show what frames changed since the last call: move each layer left by the time they advanced, in
    /// whole device pixels, so its lines stay as sharp as they were drawn, and draw again the part that
    /// shows the columns they changed and the marks that reach past them. Once the layers have moved
    /// the five seconds they reach, draw them whole. Called after the frame hooks.
    func present() {
        guard let range = pending else { return }
        pending = nil
        guard !plots.isEmpty else { return }
        guard newest - base <= Self.reach else { redraw(); return }
        let overhang = FrameHealthCardDrawing.overhang
        for (chart, plot) in plots {
            let layer = lines[chart], pixel = layer.contentsScale
            layer.position.x = -(CGFloat((newest - base) / Self.span) * (plot.right - plot.left) * scale * pixel).rounded(.down) / pixel
            var at = plot
            at.now = base
            let from = at.x(Double(range.lowerBound) * Self.columnSeconds) - overhang
            let to = at.x(Double(range.upperBound + 1) * Self.columnSeconds) + overhang
            layer.setNeedsDisplay(CGRect(x: (from - plot.left) * scale, y: 0, width: (to - from) * scale, height: layer.bounds.height))
        }
    }

    /// Place the shown plots, in the card's points, which `scale` maps to the host's. A change of
    /// place, scale or palette draws the layers whole.
    func configure(_ shown: [(chart: Int, area: CGRect, plot: FrameHealthCardDrawing.Plot)], scale: CGFloat,
                   contentsScale: CGFloat, palette: FrameHealthCardDrawing.Palette, force: Bool = false) {
        let next = Dictionary(uniqueKeysWithValues: shown.map { ($0.chart, $0.plot) })
        let same = !force && scale == self.scale && next.count == plots.count && next.allSatisfy { chart, plot in
            plots[chart].map { $0.left == plot.left && $0.right == plot.right && $0.top == plot.top && $0.bottom == plot.bottom
                && $0.maximum == plot.maximum } ?? false
        }
        guard !same else { return }
        (plots, self.scale, self.palette) = (next, scale, palette)
        for chart in 0..<2 {
            guard let plot = plots[chart] else { windows[chart].isHidden = true; continue }
            let width = plot.right - plot.left, top = plot.top - 8
            windows[chart].isHidden = false
            windows[chart].frame = CGRect(x: plot.left * scale, y: top * scale, width: width * scale, height: (plot.bottom + 1 - top) * scale)
            lines[chart].frame = CGRect(x: 0, y: 0, width: (width + margin(plot)) * scale, height: windows[chart].bounds.height)
            lines[chart].contentsScale = contentsScale
        }
        redraw()
    }

    /// The axis five seconds take, which the layer reaches past its plot.
    private func margin(_ plot: FrameHealthCardDrawing.Plot) -> CGFloat { (plot.right - plot.left) * CGFloat(Self.reach / Self.span) }

    /// Draw the layers whole from the newest frame, at the plots' right edges.
    private func redraw() {
        base = newest
        for (chart, _) in plots {
            lines[chart].position.x = 0
            lines[chart].setNeedsDisplay()
        }
    }

    fileprivate func draw(_ chart: Int, _ ctx: CGContext) {
        guard var plot = plots[chart], let palette else { return }
        plot.now = base
        // Paint the whole region, as an opaque layer must, including its last pixel column past the plot's margin
        ctx.setFillColor(palette.background)
        ctx.fill(ctx.boundingBoxOfClipPath)
        // In the plot's points with y pointing down, whichever way the layer's context points
        if ctx.ctm.d > 0 { ctx.translateBy(x: 0, y: lines[chart].bounds.height); ctx.scaleBy(x: 1, y: -1) }
        ctx.scaleBy(x: scale, y: scale)
        ctx.translateBy(x: -plot.left, y: -(plot.top - 8))
        FrameHealthCardDrawing.plotLines(ctx, chart: chart, plot: plot, columns: columns, newest: newest, palette: palette,
            margin: margin(plot))
    }
}
