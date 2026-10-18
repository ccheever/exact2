// The leaves: a paragraph as one single-line `Text` per kernel line (D4),
// an image from the bundle or the app group, a symbol, and the platform's
// live time and progress (D6).
#if canImport(WidgetKit)
import SwiftUI
import WidgetKit
import ExactCore
#if canImport(UIKit)
import UIKit
#else
import AppKit
#endif

/// A text node: each kernel line is a `Text` that SwiftUI draws and never
/// re-breaks (`.lineLimit(1).fixedSize()`), placed at the line's origin;
/// a `time` is the platform's live text in the box the kernel reserved.
struct SnapshotTextView: View {
    let node: HeadlessSession.Node
    let environment: SnapshotEnvironment
    @Environment(\.colorScheme) private var scheme

    var body: some View {
        if let live = SnapshotLive.presentation(node) {
            SnapshotLiveText(node: node, presentation: live)
        } else {
            let box = node.contentBox
            let frames = node.lines.map { line in
                CGRect(x: box.minX + line.origin.x, y: box.minY + line.origin.y - line.ascent, width: max(0, line.width), height: line.ascent + line.descent)
            }
            FrameLayout(frames: frames, size: node.frame.size) {
                ForEach(Array(node.lines.enumerated()), id: \.offset) { _, line in
                    text(line)
                        .lineLimit(1)
                        .fixedSize()
                }
            }
        }
    }

    /// The line's runs, each in the font the kernel measured it in and its
    /// own colour or the paragraph's, concatenated into one `Text`.
    private func text(_ line: HeadlessSession.Line) -> Text {
        let dark = scheme == .dark
        let ink = SnapshotColor.style(SnapshotStyle(node).color("text_color"), dark: dark, context: .ink) ?? AnyShapeStyle(.primary)
        let segments = line.segments.isEmpty ? [HeadlessSession.Segment(run: 0, text: line.text)] : line.segments
        return segments.reduce(Text(verbatim: "")) { whole, segment in
            guard segment.run < node.runs.count else { return whole + Text(verbatim: segment.text) }
            let run = node.runs[segment.run]
            var part = Text(verbatim: run.hidden ? "" : segment.text)
                .font(Font(run.font))
                .foregroundStyle(SnapshotColor.style(ColorRow(run.color), dark: dark, context: .ink) ?? ink)
            if run.letterSpacing != 0 { part = part.tracking(run.letterSpacing) }
            if run.decoration.contains("underline") { part = part.underline() }
            if run.decoration.contains("line-through") { part = part.strikethrough() }
            return whole + part
        }
    }
}

/// D6: `time` and `progress` as the platform's live views, which update
/// with no code running. The props are WP3's shape (LLP 1126.000 §2): a
/// `time` has `datetime` (an instant: epoch milliseconds, or ISO 8601) and
/// `format`, one of `relative`, `timer`, `countdown`, `offset`, `date`,
/// `time`; a `progress` over an interval has `start` and `end` instants.
package enum SnapshotLive {
    package enum Presentation: Equatable {
        case relative(Date), timer(Date), offset(Date), date(Date), time(Date)
    }

    package static func instant(_ text: String?) -> Date? {
        guard let text, !text.isEmpty else { return nil }
        if let ms = Double(text) { return Date(timeIntervalSince1970: ms / 1000) }
        return ISO8601DateFormatter().date(from: text)
    }

    package static func presentation(_ node: HeadlessSession.Node) -> Presentation? {
        guard node.kind == "time" || node.props["semanticTag"] == "time", let date = instant(node.props["datetime"]) else { return nil }
        switch node.props["format"] ?? "" {
        case "relative": return .relative(date)
        case "timer", "countdown": return .timer(date)
        case "offset": return .offset(date)
        case "time": return .time(date)
        default: return .date(date)
        }
    }

    /// A progress's interval (the platform fills it toward `end`), else nil.
    package static func interval(_ node: HeadlessSession.Node) -> ClosedRange<Date>? {
        guard let start = instant(node.props["start"]), let end = instant(node.props["end"]), end > start else { return nil }
        return start...end
    }
}

struct SnapshotLiveText: View {
    let node: HeadlessSession.Node
    let presentation: SnapshotLive.Presentation
    @Environment(\.colorScheme) private var scheme

    var body: some View {
        let run = node.runs.first
        let style = SnapshotStyle(node)
        let ink = SnapshotColor.style(ColorRow(run?.color) ?? style.color("text_color"), dark: scheme == .dark, context: .ink) ?? AnyShapeStyle(.primary)
        let font = run.map { Font($0.font) } ?? .body
        let align: Alignment = {
            switch style.string("text_align") {
            case "center": return .center
            case "right", "end": return .trailing
            default: return .leading
            }
        }()
        text
            .font(font)
            .foregroundStyle(ink)
            .monospacedDigit()
            .lineLimit(1)
            .frame(width: node.contentBox.width, height: node.contentBox.height, alignment: align)
            .offset(x: node.contentBox.minX, y: node.contentBox.minY)
    }

    private var text: Text {
        switch presentation {
        case .relative(let date): return Text(date, style: .relative)
        case .timer(let date): return Text(date, style: .timer)
        case .offset(let date): return Text(date, style: .offset)
        case .date(let date): return Text(date, style: .date)
        case .time(let date): return Text(date, style: .time)
        }
    }
}

/// `progress`: over an interval the platform's timer bar; with a value the
/// determinate bar; else the activity indicator.
struct SnapshotProgressView: View {
    let node: HeadlessSession.Node

    var body: some View {
        Group {
            if let interval = SnapshotLive.interval(node) {
                ProgressView(timerInterval: interval, countsDown: node.props["format"] == "countdown") { EmptyView() } currentValueLabel: { EmptyView() }
            } else if let value = Double(node.props["value"] ?? "") {
                ProgressView(value: min(value, total), total: total)
            } else {
                ProgressView()
            }
        }
        .frame(width: node.contentBox.width, height: node.contentBox.height)
        .offset(x: node.contentBox.minX, y: node.contentBox.minY)
    }

    private var total: Double { max(Double(node.props["max"] ?? "") ?? 1, Double.leastNonzeroMagnitude) }
}

/// An image: a system symbol at its text's size and colour (the tint when
/// unsaid, as the kernel's `symbol` role says), or a raster from the entry's
/// assets, fitted as `object-fit` says. Never fetched in the render.
struct SnapshotImageView: View {
    let node: HeadlessSession.Node
    let environment: SnapshotEnvironment
    @Environment(\.colorScheme) private var scheme

    var body: some View {
        let box = node.contentBox
        Group {
            if (node.props["imageSource"] ?? "").hasPrefix("symbol:") {
                symbol
            } else if let image = raster {
                image.resizable()
                    .aspectRatio(contentMode: SnapshotStyle(node).string("object_fit") == "cover" ? .fill : .fit)
                    .frame(width: box.width, height: box.height)
                    .clipped()
            }
        }
        .frame(width: box.width, height: box.height)
        .offset(x: box.minX, y: box.minY)
    }

    @ViewBuilder
    private var symbol: some View {
        let name = node.props["symbolName"] ?? ""
        if !name.isEmpty {
            let style = SnapshotStyle(node)
            let points = style.number("font_size") ?? 17
            let weight = Int(style.number("font_weight") ?? 400)
            let ink = SnapshotColor.style(style.color("tint_color") ?? style.color("text_color"), dark: scheme == .dark, context: .ink) ?? AnyShapeStyle(.tint)
            Image(systemName: name)
                .font(.system(size: points, weight: Font.Weight(css: weight)))
                .foregroundStyle(ink)
        }
    }

    private var raster: Image? {
        guard let source = node.props["imageSource"], !source.isEmpty else { return nil }
        let url = source.hasPrefix("/") ? URL(fileURLWithPath: source)
            : environment.assetRoot.appending(path: source.hasPrefix("assets/") ? source : "assets/" + source)
        #if canImport(UIKit)
        return UIImage(contentsOfFile: url.path).map { Image(uiImage: $0) }
        #else
        return NSImage(contentsOf: url).map { Image(nsImage: $0) }
        #endif
    }
}
#endif
