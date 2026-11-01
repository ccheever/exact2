import CoreGraphics
import CoreText
import Foundation
import ImageIO
#if canImport(UIKit)
import UIKit
#else
import AppKit
#endif

/// The card's charts and its shared picture, drawn by hand on one opaque surface in a context whose
/// y axis points down (LLP 1079.000 §7.2). Shared by iOS and macOS.
enum FrameHealthCardDrawing {
    /// Opaque colors: the translucent system roles composited over the card's background ahead of
    /// time, as Exact's roles are on the card's web design.
    struct Palette {
        var background, label, secondary, separator, axis, blue, orange, green, gray: CGColor
        /// The status button's fill, hovered and pressed; an icon button's hovered and pressed fill
        var statusFill, statusHover, statusPressed, iconHover, iconPressed: CGColor
        /// The segmented control's track and chosen chip
        var track, chip: CGColor

        /// Composite resolved system colors, which may be translucent, over `background`.
        init(background: CGColor, label: CGColor, secondaryLabel: CGColor, separator: CGColor, fill: CGColor,
             secondaryFill: CGColor, tertiaryFill: CGColor, quaternaryFill: CGColor, blue: CGColor, orange: CGColor,
             green: CGColor, gray: CGColor, dark: Bool) {
            func over(_ color: CGColor, _ base: CGColor, alpha scale: CGFloat = 1) -> CGColor {
                let space = CGColorSpace(name: CGColorSpace.extendedSRGB)!
                guard let top = color.converted(to: space, intent: .defaultIntent, options: nil)?.components,
                      let bottom = base.converted(to: space, intent: .defaultIntent, options: nil)?.components,
                      top.count == 4, bottom.count == 4 else { return base }
                let a = top[3] * scale
                return CGColor(colorSpace: space, components: [0, 1, 2].map { top[$0] * a + bottom[$0] * (1 - a) } + [1])!
            }
            self.background = over(background, CGColor(gray: dark ? 0 : 1, alpha: 1))
            let ground = self.background
            self.label = over(label, ground)
            secondary = over(secondaryLabel, ground)
            self.separator = over(separator, ground)
            // A chart's zero line, a step stronger than its gridlines
            axis = over(label, ground, alpha: 0.25)
            self.blue = over(blue, ground); self.orange = over(orange, ground)
            self.green = over(green, ground); self.gray = over(gray, ground)
            statusFill = over(secondaryFill, ground)
            statusHover = over(quaternaryFill, statusFill)
            statusPressed = over(tertiaryFill, statusFill)
            iconHover = over(quaternaryFill, ground)
            iconPressed = over(tertiaryFill, ground)
            track = over(tertiaryFill, ground)
            chip = dark ? over(fill, track) : ground
        }
    }

    static func semibold(_ size: CGFloat) -> CTFont {
        #if canImport(UIKit)
        UIFont.systemFont(ofSize: size, weight: .semibold) as CTFont
        #else
        NSFont.systemFont(ofSize: size, weight: .semibold) as CTFont
        #endif
    }

    /// The charts' scales, from the request, the schedule and the reader's last 30 seconds.
    struct Input {
        var rates: FrameHealthCardText.Rates
        /// The frame-rate chart's top: the requested rate, or the display's maximum without one
        var rateTop: Double
        /// The frame-time chart's top: twice the target frame time
        var timeTop: Double

        init(reader: FrameHealthCardReader, rates: FrameHealthCardText.Rates, displayMaximumHz: Double) {
            self.rates = rates
            rateTop = rates.requestedHz > 0 ? rates.requestedHz : displayMaximumHz > 0 ? displayMaximumHz : 60
            // The target is what the app requests or, when it runs slower, its 30-second mean frame
            // time in whole refreshes; a frame twice as long reaches the top
            let thirty = reader.window(30)
            let period = rates.periodMs > 0 ? rates.periodMs : 1000 / rateTop
            let mean = thirty.frames == 0 ? 0 : Double(thirty.counts.activeNs) / Double(thirty.frames) / 1e6
            let average = mean == 0 ? 0 : max(1, (mean / period).rounded()) * period
            timeTop = 2 * max(1000 / rateTop, average)
        }
    }

    enum Align { case left, right }
    enum Rule { case grid, axis, dotted }

    static let labelFont = CTFontCreateUIFontForLanguage(.system, 10, nil)!
    static let titleFont = semibold(11)

    /// Draw the frame-rate chart (0), the frame-time chart (1), or both, 16 points apart, filling
    /// `rect`, with `lines` up to their newest frame. Without them, the plots are left to the card's
    /// layers (`FrameHealthChart`).
    static func charts(_ ctx: CGContext, _ rect: CGRect, _ input: Input, expanded: Int?, palette: Palette,
                       lines: (columns: [FrameHealthChart.Column], newest: Double)? = nil) {
        ctx.setFillColor(palette.background)
        ctx.fill(rect)
        for (chart, area, plot) in plots(rect, input, expanded: expanded) {
            text(ctx, chart == 0 ? "Frame rate" : "Frame time", x: area.minX, y: area.minY + 7, titleFont, palette.label, .left)
            for (label, value, _) in plot.labels {
                text(ctx, label, x: plot.left - 4, y: plot.y(value), labelFont, palette.secondary, .right)
            }
            if chart == 0 {
                text(ctx, "FPS (last 30s)", x: area.maxX, y: area.minY + 7, labelFont, palette.secondary, .right)
            } else {
                var x = area.maxX
                for (name, color, dotted) in [("deadline", palette.secondary, true), ("work", palette.orange, false),
                                              ("frame", palette.blue, false)] {
                    x -= width(name, labelFont)
                    text(ctx, name, x: x, y: area.minY + 7, labelFont, palette.secondary, .left)
                    stroke(ctx, from: CGPoint(x: x - 16, y: area.minY + 7.5), to: CGPoint(x: x - 4, y: area.minY + 7.5),
                        color: color, width: dotted ? 1 : 1.25, dotted: dotted)
                    x -= 26
                }
                text(ctx, "30s ago", x: plot.left, y: area.maxY - 6, labelFont, palette.secondary, .left)
                text(ctx, "now", x: plot.right, y: area.maxY - 6, labelFont, palette.secondary, .right)
            }
            if let lines {
                var plot = plot
                plot.now = lines.newest
                plotLines(ctx, chart: chart, plot: plot, columns: lines.columns, newest: lines.newest, palette: palette, margin: 0)
            }
        }
    }

    /// Each shown chart with the area it fills and its plot. The frame-rate chart runs from 0 to the
    /// request, its top dotted as the target, so an app on target draws along the top; the frame-time
    /// chart runs to twice the target frame time, its 0 ms rule unlabelled so it does not crowd the
    /// time labels under it. Value labels sit in a gutter sized to them.
    static func plots(_ rect: CGRect, _ input: Input, expanded: Int?) -> [(chart: Int, area: CGRect, plot: Plot)] {
        let areas: [(Int, CGRect)] = expanded.map { [($0, rect)] } ?? [
            (0, CGRect(x: rect.minX, y: rect.minY, width: rect.width, height: 80)),
            (1, CGRect(x: rect.minX, y: rect.minY + 96, width: rect.width, height: rect.height - 96))]
        return areas.map { chart, area in
            let top = chart == 0 ? input.rateTop : input.timeTop, parts = chart == 1 || expanded != nil ? 4 : 2
            let labels = (0...parts).map { i -> (String, Double, Rule) in
                if chart == 1 {
                    let value = top * Double(i) / 4
                    return (i == 0 ? "" : String(format: "%.1f", value) + (i == 4 ? "ms" : ""), value, i == 0 ? .axis : .grid)
                }
                let value = top * Double(parts - i) / Double(parts)
                let text = value == value.rounded() ? String(Int(value)) : String(format: "%.1f", value)
                return (text, value, i == 0 ? .dotted : value == 0 ? .axis : .grid)
            }
            let gutter = ceil(labels.map { width($0.0, labelFont) }.max() ?? 0) + 6
            let plot = Plot(left: area.minX + gutter, right: area.maxX - 2, top: area.minY + 22,
                bottom: area.maxY - (chart == 0 ? 6 : 14), maximum: top > 0 ? top : 1, now: 0, labels: labels)
            return (chart, area, plot)
        }
    }

    /// A plot's gridlines and lines, from tenth-of-a-second columns, where `ctx` draws over the
    /// caller's background: a card's layer redraws only the columns frames changed. On a layer, the
    /// plot is `margin` points wider at the right, where the frames after its last whole drawing go, and
    /// the layer reaches 8 points above it, for the carets. A column's lines reach back to the previous
    /// column with frames when its first frame continued that run; gaps are where a run ended.
    static func plotLines(_ ctx: CGContext, chart: Int, plot: Plot, columns: [FrameHealthChart.Column], newest: Double,
                          palette: Palette, margin: CGFloat) {
        let clip = ctx.boundingBoxOfClipPath
        let left = max(plot.left, clip.minX), right = min(plot.right + margin, clip.maxX)
        guard right > left else { return }
        for (_, value, rule) in plot.labels {
            let color = rule == .grid ? palette.separator : rule == .axis ? palette.axis : palette.secondary
            let line = CGFloat(Int(plot.y(value))) + 0.5
            stroke(ctx, from: CGPoint(x: left, y: line), to: CGPoint(x: right, y: line), color: color, width: 1,
                dotted: rule == .dotted, phase: plot.dash(left))
        }
        guard !columns.isEmpty, newest > 0 else { return }
        // The columns whose marks reach the region, and the nearest either side that holds frames, so a
        // line into it is whole
        let step = FrameHealthChart.columnSeconds, count = Int64(columns.count)
        let latest = Int64((newest / step).rounded(.down)), oldest = max(0, latest - count + 1)
        let held = { (i: Int64) in columns[Int(i % count)].index == i }
        var from = max(oldest, Int64((plot.time(left - overhang) / step).rounded(.down)) - 1)
        var to = min(latest, Int64((plot.time(right + overhang) / step).rounded(.down)) + 1)
        while from > oldest, !held(from) { from -= 1 }
        while to < latest, !held(to) { to += 1 }
        guard from <= to else { return }
        // Each held column with the span its lines take: from the previous held column's end when it
        // joins it, and otherwise from its own start
        var shown: [(CGFloat, CGFloat, FrameHealthChart.Column)] = [], end: CGFloat?
        for i in from...to where held(i) {
            let column = columns[Int(i % count)]
            let x0 = column.joined ? end ?? plot.x(Double(i) * step) : plot.x(Double(i) * step)
            let x1 = min(plot.x(Double(i + 1) * step), plot.x(newest))
            end = x1
            if x1 >= plot.left { shown.append((max(plot.left, x0), x1, column)) }
        }
        guard chart == 0 else {
            // Work draws last, so where both reach one height the cause stays visible
            steps(ctx, plot, shown, { $0.budget }, palette.secondary, dotted: true)
            steps(ctx, plot, shown, { $0.frame }, palette.blue)
            steps(ctx, plot, shown, { $0.budget > 0 ? $0.work : 0 }, palette.orange)
            return
        }
        // The moving average at each column's newest frame
        var pen = false
        ctx.setStrokeColor(palette.blue)
        ctx.setLineWidth(1.25)
        ctx.setLineJoin(.round)
        for (_, x1, column) in shown {
            guard column.fps > 0 else { pen = false; continue }
            let point = CGPoint(x: x1, y: plot.y(Double(column.fps)))
            if pen, column.joined { ctx.addLine(to: point) } else { ctx.move(to: point) }
            pen = true
        }
        ctx.strokePath()
    }

    /// How far a column's marks reach past its line: a caret reaches 3 points either side of the
    /// line's middle
    static let overhang: CGFloat = 3

    struct Plot {
        var left, right, top, bottom: CGFloat
        var maximum: Double
        /// The time at the plot's right edge
        var now: Double
        /// Each value label, its value and the rule drawn at it
        var labels: [(String, Double, Rule)]
        func y(_ value: Double) -> CGFloat { bottom - CGFloat(min(value, maximum) / maximum) * (bottom - top) }
        /// Thirty seconds across the plot, `now` at its right edge
        func x(_ at: Double) -> CGFloat { right - CGFloat((now - at) / 30) * (right - left) }
        func time(_ x: CGFloat) -> Double { now - Double((right - x) / (right - left)) * 30 }
        /// How far into its 5-point pattern a dotted line is at `x`, counted from the clock's origin, so
        /// dashes keep their place in time however much of the plot is drawn and wherever it starts
        func dash(_ x: CGFloat) -> CGFloat {
            let phase = (x - self.x(0)).truncatingRemainder(dividingBy: 5)
            return phase < 0 ? phase + 5 : phase
        }
    }

    /// One value per column as steps across the span it covers; a column without a value, or a
    /// missing one, leaves a gap. A value past the top is drawn at the top with a caret above it, so
    /// it does not read as the top's value.
    private static func steps(_ ctx: CGContext, _ plot: Plot, _ shown: [(CGFloat, CGFloat, FrameHealthChart.Column)],
                              _ value: (FrameHealthChart.Column) -> Float, _ color: CGColor, dotted: Bool = false) {
        ctx.setStrokeColor(color)
        ctx.setFillColor(color)
        ctx.setLineWidth(dotted ? 1 : 1.25)
        ctx.setLineJoin(.round)
        // The level being drawn: the height of the columns joined at it and where the last of them ends. A
        // level is one segment, since Core Graphics spaces a dotted line's dashes unevenly along many short ones.
        var carets: [CGFloat] = [], level: (y: CGFloat, end: CGFloat)?
        for (x0, x1, column) in shown {
            let v = Double(value(column)), y = plot.y(v), from = column.joined ? level?.y : nil
            if v > 0, from == y {
                level?.end = x1
            } else {
                if let level { ctx.addLine(to: CGPoint(x: level.end, y: level.y)) }
                level = nil
                guard v > 0 else { continue }
                if dotted {
                    // Each height of a dotted line is its own path, so its dashes keep their place in time after the
                    // step into it
                    ctx.strokePath()
                    let phase = (plot.dash(x0) - (from.map { abs(y - $0) } ?? 0)).truncatingRemainder(dividingBy: 5)
                    ctx.setLineDash(phase: phase < 0 ? phase + 5 : phase, lengths: [2, 3])
                    ctx.move(to: CGPoint(x: x0, y: from ?? y))
                    if from != nil { ctx.addLine(to: CGPoint(x: x0, y: y)) }
                } else if from != nil {
                    ctx.addLine(to: CGPoint(x: x0, y: y))
                } else {
                    ctx.move(to: CGPoint(x: x0, y: y))
                }
                level = (y, x1)
            }
            // A tolerance, so a value equal to the top through floating point is not taken for one past it
            if v - plot.maximum > 1e-6 { carets.append((x0 + x1) / 2) }
        }
        if let level { ctx.addLine(to: CGPoint(x: level.end, y: level.y)) }
        ctx.strokePath()
        ctx.setLineDash(phase: 0, lengths: [])
        for x in carets {
            ctx.move(to: CGPoint(x: x - 3, y: plot.top - 3))
            ctx.addLine(to: CGPoint(x: x + 3, y: plot.top - 3))
            ctx.addLine(to: CGPoint(x: x, y: plot.top - 7))
            ctx.closePath()
        }
        ctx.fillPath()
    }

    private static func stroke(_ ctx: CGContext, from: CGPoint, to: CGPoint, color: CGColor, width: CGFloat, dotted: Bool,
                               phase: CGFloat = 0) {
        ctx.setStrokeColor(color)
        ctx.setLineWidth(width)
        ctx.setLineDash(phase: phase, lengths: dotted ? [2, 3] : [])
        ctx.move(to: from)
        ctx.addLine(to: to)
        ctx.strokePath()
        ctx.setLineDash(phase: 0, lengths: [])
    }

    private static func line(_ string: String, _ font: CTFont, _ color: CGColor? = nil) -> CTLine {
        var attributes: [NSAttributedString.Key: Any] = [NSAttributedString.Key(kCTFontAttributeName as String): font]
        if let color { attributes[NSAttributedString.Key(kCTForegroundColorAttributeName as String)] = color }
        return CTLineCreateWithAttributedString(NSAttributedString(string: string, attributes: attributes))
    }

    /// A string broken into lines no wider than `width`, where Core Text suggests.
    static func lines(_ string: String, width: CGFloat, _ font: CTFont) -> [String] {
        let attributed = NSAttributedString(string: string, attributes: [NSAttributedString.Key(kCTFontAttributeName as String): font])
        let typesetter = CTTypesetterCreateWithAttributedString(attributed), text = string as NSString
        var start = 0, lines: [String] = []
        while start < text.length {
            let count = max(1, CTTypesetterSuggestLineBreak(typesetter, start, Double(width)))
            lines.append(text.substring(with: NSRange(location: start, length: count)).trimmingCharacters(in: .whitespaces))
            start += count
        }
        return lines
    }

    static func width(_ string: String, _ font: CTFont) -> CGFloat {
        CGFloat(CTLineGetTypographicBounds(line(string, font), nil, nil, nil))
    }

    /// Draw `string` with its middle at `y`, starting or ending at `x`. Wider than `limit`, it shrinks
    /// to fit, as a label that adjusts its font size to its width does.
    static func text(_ ctx: CGContext, _ string: String, x: CGFloat, y: CGFloat, _ font: CTFont, _ color: CGColor, _ align: Align,
                     limit: CGFloat = .infinity) {
        guard !string.isEmpty else { return }
        var line = line(string, font, color)
        var ascent: CGFloat = 0, descent: CGFloat = 0
        var width = CGFloat(CTLineGetTypographicBounds(line, &ascent, &descent, nil))
        // A font's metrics do not scale exactly with its size, so each smaller size is measured again
        var size = CTFontGetSize(font)
        for _ in 0..<4 where width > limit {
            size *= limit / width
            line = Self.line(string, CTFontCreateCopyWithAttributes(font, size, nil, nil), color)
            width = CGFloat(CTLineGetTypographicBounds(line, &ascent, &descent, nil))
        }
        ctx.saveGState()
        ctx.textMatrix = CGAffineTransform(scaleX: 1, y: -1)
        ctx.textPosition = CGPoint(x: align == .right ? x - width : x, y: y + (ascent - descent) / 2)
        CTLineDraw(line, ctx)
        ctx.restoreGState()
    }

    /// The shared picture: the context lines, both charts, every figure in a table whose columns are
    /// right-aligned in tabular digits, then Details' results. PNG data at twice the point size.
    static func picture(title: String, context: [String], input: Input, chart: FrameHealthChart, table: [[String]],
                        results: [String], notes: [String], palette: Palette) -> Data? {
        let body = tabular(CTFontCreateUIFontForLanguage(.system, 11, nil)!)
        let heading = semibold(15)
        let lineHeight: CGFloat = 18, gap: CGFloat = 16, margin: CGFloat = 16
        let nameWidth = table.dropFirst().map { width($0[0], body) }.max() ?? 0
        let column = table.flatMap { $0.dropFirst() }.map { width($0, body) }.max() ?? 0
        let columns = CGFloat(max(0, (table.first?.count ?? 1) - 1))
        let prose = (context + results + notes).map { width($0, body) }.max() ?? 0
        let size = CGSize(width: ceil(max(380 - 2 * margin, nameWidth + columns * (gap + column), prose)) + 2 * margin,
            height: 40 + CGFloat(context.count) * lineHeight + 12 + 80 + 16 + 104 + 16
                + CGFloat(table.count + results.count + notes.count) * lineHeight + 24)
        let scale: CGFloat = 2
        guard let space = CGColorSpace(name: CGColorSpace.sRGB),
              let ctx = CGContext(data: nil, width: Int(size.width * scale), height: Int(size.height * scale),
                  bitsPerComponent: 8, bytesPerRow: 0, space: space, bitmapInfo: CGImageAlphaInfo.noneSkipLast.rawValue)
        else { return nil }
        ctx.translateBy(x: 0, y: size.height * scale)
        ctx.scaleBy(x: scale, y: -scale)
        ctx.setFillColor(palette.background)
        ctx.fill(CGRect(origin: .zero, size: size))
        text(ctx, title, x: margin, y: 22, heading, palette.label, .left)
        var y: CGFloat = 40
        for line in context { y += lineHeight; text(ctx, line, x: margin, y: y - 4, body, palette.label, .left) }
        y += 12
        let chartWidth = size.width - 2 * margin
        charts(ctx, CGRect(x: margin, y: y, width: chartWidth, height: 200), input, expanded: nil, palette: palette,
            lines: (chart.columns, chart.newest))
        y += 80 + 16 + 104 + 16
        for (index, row) in table.enumerated() {
            y += lineHeight
            if index > 0 { text(ctx, row[0], x: margin, y: y - 4, body, palette.secondary, .left) }
            for (i, value) in row.dropFirst().enumerated() {
                text(ctx, value, x: margin + nameWidth + CGFloat(i + 1) * (gap + column), y: y - 4, body,
                    index == 0 ? palette.secondary : palette.label, .right)
            }
        }
        y += 6
        for result in results { y += lineHeight; text(ctx, result, x: margin, y: y - 4, body, palette.label, .left) }
        y += 6
        for note in notes { y += lineHeight; text(ctx, note, x: margin, y: y - 4, body, palette.secondary, .left) }
        guard let image = ctx.makeImage() else { return nil }
        let data = NSMutableData()
        guard let destination = CGImageDestinationCreateWithData(data, "public.png" as CFString, 1, nil) else { return nil }
        CGImageDestinationAddImage(destination, image, nil)
        return CGImageDestinationFinalize(destination) ? data as Data : nil
    }

    /// The font with tabular digits, so a column of figures lines up.
    static func tabular(_ font: CTFont) -> CTFont {
        let settings = [[kCTFontFeatureTypeIdentifierKey: kNumberSpacingType,
                         kCTFontFeatureSelectorIdentifierKey: kMonospacedNumbersSelector]]
        let descriptor = CTFontDescriptorCreateCopyWithAttributes(CTFontCopyFontDescriptor(font),
            [kCTFontFeatureSettingsAttribute: settings] as CFDictionary)
        return CTFontCreateWithFontDescriptor(descriptor, CTFontGetSize(font), nil)
    }
}
