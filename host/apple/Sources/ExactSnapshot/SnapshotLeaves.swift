// The leaves: a paragraph as one single-line `Text` per kernel line (D4),
// an image from the bundle or the app group, a symbol, and the platform's
// live time and progress (D6). Text and symbols are the accented group
// (D5): the glyphs take `widgetAccentable`; a raster image stays in the
// default group, desaturated unless the author says otherwise.
#if canImport(WidgetKit)
import SwiftUI
import WidgetKit
import ImageIO
import ExactCore

/// A text node: each kernel line is a `Text` that SwiftUI draws and never
/// re-breaks (`.lineLimit(1).fixedSize()`), placed at the line's origin;
/// a live `time` is the platform's text in the box the kernel reserved.
@available(iOS 17.0, macOS 14.0, watchOS 10.0, visionOS 26.0, *)
struct SnapshotTextView: View {
    let node: HeadlessSession.Node
    let environment: SnapshotEnvironment
    @Environment(\.colorScheme) private var scheme

    var body: some View {
        if let live = SnapshotLive.presentation(node, entryDate: environment.entryDate) {
            SnapshotLiveText(node: node, presentation: live).widgetAccentable()
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
                        .widgetAccentable()
                }
            }
        }
    }

    /// The line's runs, each in the font the kernel measured it in and its
    /// own colour or the paragraph's, concatenated into one `Text`. The
    /// text is WP1's `Line.text`: a clamped line's cut and "…" come from
    /// there. A run's background and `href` are refused by the profile.
    private func text(_ line: HeadlessSession.Line) -> Text {
        let dark = scheme == .dark
        let ink = SnapshotColor.style(SnapshotStyle(node).color("text_color"), dark: dark, context: .ink) ?? AnyShapeStyle(.primary)
        let segments = line.segments.isEmpty ? [HeadlessSession.Segment(run: 0, text: line.text)] : line.segments
        return segments.reduce(Text(verbatim: "")) { whole, segment in
            guard segment.run < node.runs.count else { return whole + Text(verbatim: segment.text) }
            let run = node.runs[segment.run]
            var part = Text(verbatim: segment.text)
                .font(Font(run.font))
                .foregroundStyle(SnapshotColor.style(ColorRow(run.color), dark: dark, context: .ink) ?? ink)
            if run.letterSpacing != 0 { part = part.tracking(run.letterSpacing) }
            if run.decoration.contains("underline") { part = part.underline() }
            if run.decoration.contains("line-through") { part = part.strikethrough() }
            return whole + part
        }
    }
}

/// D5: under the accented rendering mode, text and symbols are the accented
/// group (their glyphs take the tint); a raster image is the default
/// group, desaturated unless the author wrote `data-rendering="full-color"`.
package enum SnapshotAccent {
    package enum Group: Equatable { case accented, desaturated, fullColor }
    package static func group(_ node: HeadlessSession.Node) -> Group? {
        switch node.kind {
        case "text", "time": return .accented
        case "image":
            if (node.props["imageSource"] ?? "").hasPrefix("symbol:") { return .accented }
            return SnapshotData.words(node)["rendering"] == "full-color" ? .fullColor : .desaturated
        default: return nil
        }
    }
}

/// D6: `time` and `progress` as the platform's live views, which update
/// with no code running. A `time` has `datetime` (an instant: epoch
/// milliseconds, or ISO 8601 with or without fractional seconds) and
/// `format`: `relative` is `Text(date, style: .relative)`, `timer` is
/// `Text(date, style: .timer)` (counting on both sides of the instant),
/// `countdown` is `Text(timerInterval: entryDate...instant, countsDown:
/// true)` from the timeline entry's date, stopping at zero as the running
/// hosts do; the calendar formats do not tick and draw as the kernel's
/// text. A `progress` over an interval has `start` and `end` instants.
package enum SnapshotLive {
    package enum Presentation: Equatable {
        case relative(Date), timer(Date), countdown(ClosedRange<Date>)
    }

    private static let fractional: ISO8601DateFormatter = {
        let f = ISO8601DateFormatter(); f.formatOptions = [.withInternetDateTime, .withFractionalSeconds]; return f
    }()
    private static let whole = ISO8601DateFormatter()

    package static func instant(_ text: String?) -> Date? {
        guard let text, !text.isEmpty else { return nil }
        if let ms = Double(text) { return Date(timeIntervalSince1970: ms / 1000) }
        return fractional.date(from: text) ?? whole.date(from: text)
    }

    package static func presentation(_ node: HeadlessSession.Node, entryDate: Date) -> Presentation? {
        guard node.kind == "time" || node.props["semanticTag"] == "time", let date = instant(node.props["datetime"]) else { return nil }
        switch node.props["format"] ?? "" {
        case "relative": return .relative(date)
        case "timer": return .timer(date)
        case "countdown": return .countdown(min(entryDate, date)...date)
        default: return nil
        }
    }

    /// A progress's interval (the platform fills it toward `end`), else nil.
    package static func interval(_ node: HeadlessSession.Node) -> ClosedRange<Date>? {
        guard let start = instant(node.props["start"]), let end = instant(node.props["end"]), end > start else { return nil }
        return start...end
    }
}

/// The live text in the node's content box, in the font the kernel
/// measured the node's text in and its colour.
@available(iOS 17.0, macOS 14.0, watchOS 10.0, visionOS 26.0, *)
struct SnapshotLiveText: View {
    let node: HeadlessSession.Node
    let presentation: SnapshotLive.Presentation
    @Environment(\.colorScheme) private var scheme

    var body: some View {
        let run = node.runs.first
        let style = SnapshotStyle(node)
        let ink = SnapshotColor.style(ColorRow(run?.color) ?? style.color("text_color"), dark: scheme == .dark, context: .ink) ?? AnyShapeStyle(.primary)
        let align: Alignment = {
            switch style.string("text_align") {
            case "center": return .center
            case "right", "end": return .trailing
            default: return .leading
            }
        }()
        text
            .font(run.map { Font($0.font) } ?? .body)
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
        case .countdown(let interval): return Text(timerInterval: interval, countsDown: true)
        }
    }
}

/// `progress`: over an interval the platform's timer bar; with a value the
/// determinate bar; else the activity indicator.
@available(iOS 17.0, macOS 14.0, watchOS 10.0, visionOS 26.0, *)
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
/// unsaid, as the kernel's `symbol` role says), in the accented group; or a
/// raster from the entry's image roots, decoded as a bounded thumbnail at
/// the frame's pixel size, fitted as `object-fit` says and clipped to its
/// rounded content edge, in the default group and desaturated under the
/// accented mode unless `data-rendering="full-color"`. Never fetched in
/// the render; the profile requires both dimensions authored.
@available(iOS 17.0, macOS 14.0, watchOS 10.0, visionOS 26.0, *)
struct SnapshotImageView: View {
    let node: HeadlessSession.Node
    let environment: SnapshotEnvironment
    @Environment(\.colorScheme) private var scheme
    @Environment(\.displayScale) private var displayScale

    var body: some View {
        let box = node.contentBox
        let style = SnapshotStyle(node)
        Group {
            if (node.props["imageSource"] ?? "").hasPrefix("symbol:") {
                symbol.widgetAccentable()
            } else if let raster = SnapshotRasters.decode(node.props["imageSource"], roots: environment.imageRoots, pixels: CGSize(width: box.width * displayScale, height: box.height * displayScale)) {
                SnapshotRaster(image: Image(decorative: raster.image, scale: 1), natural: raster.natural, fit: style.string("object_fit") ?? "fill", box: box.size,
                               fullColor: SnapshotAccent.group(node) == .fullColor)
                    .clipShape(RoundedOutline(radii: SnapshotPaint(style: style, dark: scheme == .dark)
                        .contentRadii(outer: CGRect(origin: .zero, size: node.frame.size), inner: box)))
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
}

/// Raster decoding, bounded (D8: an extension's memory ceiling): ImageIO's
/// thumbnail at the frame's pixel size, never the full image, decoded
/// once per invocation and kept with the natural dimensions.
package enum SnapshotRasters {
    package struct Decoded {
        package let image: CGImage
        package let natural: CGSize
    }
    private static let cache = NSCache<NSString, Box>()
    private final class Box { let value: Decoded; init(_ value: Decoded) { self.value = value } }

    package static func decode(_ source: String?, roots: [URL], pixels: CGSize) -> Decoded? {
        guard let source, !source.isEmpty, let url = confined(source, roots: roots) else { return nil }
        return decode(url, pixels: pixels)
    }

    package static func decode(_ url: URL, pixels: CGSize) -> Decoded? {
        let longest = Int(max(1, max(pixels.width, pixels.height).rounded(.up)))
        let key = "\(url.path)|\(longest)" as NSString
        if let hit = cache.object(forKey: key) { return hit.value }
        guard let imageSource = CGImageSourceCreateWithURL(url as CFURL, nil),
              let properties = CGImageSourceCopyPropertiesAtIndex(imageSource, 0, nil) as? [CFString: Any],
              let width = (properties[kCGImagePropertyPixelWidth] as? NSNumber)?.doubleValue,
              let height = (properties[kCGImagePropertyPixelHeight] as? NSNumber)?.doubleValue else { return nil }
        // EXIF orientation 5–8 swap the sides; the thumbnail is made upright.
        let orientation = (properties[kCGImagePropertyOrientation] as? NSNumber)?.intValue ?? 1
        let natural = orientation >= 5 ? CGSize(width: height, height: width) : CGSize(width: width, height: height)
        let options: [CFString: Any] = [
            kCGImageSourceCreateThumbnailFromImageAlways: true,
            kCGImageSourceCreateThumbnailWithTransform: true,
            kCGImageSourceShouldCacheImmediately: true,
            kCGImageSourceThumbnailMaxPixelSize: longest,
        ]
        guard let image = CGImageSourceCreateThumbnailAtIndex(imageSource, 0, options as CFDictionary) else { return nil }
        let decoded = Decoded(image: image, natural: natural)
        cache.setObject(Box(decoded), forKey: key)
        return decoded
    }

    /// The file inside one of the entry's image roots (the bundle, the app
    /// group's generation): a name may carry `..`, so the resolved path
    /// must stay under a root.
    package static func confined(_ source: String, roots: [URL]) -> URL? {
        for root in roots {
            let base = root.standardizedFileURL.resolvingSymlinksInPath()
            let candidate: URL = source.hasPrefix("/") ? URL(fileURLWithPath: source)
                : base.appending(path: source.hasPrefix("assets/") ? source : "assets/" + source)
            let resolved = candidate.standardizedFileURL.resolvingSymlinksInPath()
            if resolved.path.hasPrefix(base.path + "/"), FileManager.default.fileExists(atPath: resolved.path) { return resolved }
        }
        return nil
    }
}

/// CSS `object-fit` over the image's natural size: `fill` stretches,
/// `contain` and `cover` keep the aspect, `none` keeps the natural size,
/// `scale-down` is `none` or `contain`, whichever is smaller. D5: under the
/// accented mode the raster is desaturated (the default group keeps its
/// detail) unless the author asked for full colour; the modifier is iOS
/// 18's, and older systems draw the default.
@available(iOS 17.0, macOS 14.0, watchOS 10.0, visionOS 26.0, *)
struct SnapshotRaster: View {
    let image: Image
    let natural: CGSize
    let fit: String
    let box: CGSize
    let fullColor: Bool

    private func treated(_ image: Image) -> AnyView {
        if #available(iOS 18.0, macOS 15.0, watchOS 11.0, visionOS 26.0, *) {
            return AnyView(image.widgetAccentedRenderingMode(fullColor ? .fullColor : .desaturated))
        }
        return AnyView(image)
    }

    var body: some View {
        let resizable = treated(image.resizable())
        switch fit {
        case "contain": resizable.aspectRatio(natural, contentMode: .fit).frame(width: box.width, height: box.height)
        case "cover": resizable.aspectRatio(natural, contentMode: .fill).frame(width: box.width, height: box.height).clipped()
        case "none": resizable.frame(width: natural.width, height: natural.height).frame(width: box.width, height: box.height).clipped()
        case "scale-down":
            if natural.width <= box.width && natural.height <= box.height {
                resizable.frame(width: natural.width, height: natural.height).frame(width: box.width, height: box.height)
            } else {
                resizable.aspectRatio(natural, contentMode: .fit).frame(width: box.width, height: box.height)
            }
        default: resizable.frame(width: box.width, height: box.height)
        }
    }
}
#endif
