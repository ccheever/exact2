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
#if canImport(UIKit)
import UIKit
#else
import AppKit
#endif

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
            let part = Text(verbatim: segment.text)
                .font(Font(run.font))
                .foregroundStyle(SnapshotColor.style(ColorRow(run.color), dark: dark, context: .ink) ?? ink)
            return whole + SnapshotTextPaint.styled(part, run: run, node: node)
        }
    }
}

/// The paint a run's text takes beyond its font and colour: its tracking
/// and its decoration, the run's own added to the node's `text-decoration`
/// (CSS Text Decoration 3 §2.1: an inline's lines add to its ancestors').
package enum SnapshotTextPaint {
    package static func decoration(_ run: HeadlessSession.Run, node: HeadlessSession.Node) -> String {
        [node.style["text_decoration_line"]?.string ?? "", run.decoration].filter { !$0.isEmpty }.joined(separator: " ")
    }
    static func styled(_ text: Text, run: HeadlessSession.Run, node: HeadlessSession.Node) -> Text {
        var part = text
        if run.letterSpacing != 0 { part = part.tracking(run.letterSpacing) }
        let decoration = decoration(run, node: node)
        if decoration.contains("underline") { part = part.underline() }
        if decoration.contains("line-through") { part = part.strikethrough() }
        return part
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

    /// `ISO8601DateFormatter` is not thread-safe and WidgetKit archives
    /// entries concurrently: a formatter per call.
    package static func instant(_ text: String?) -> Date? {
        guard let text, !text.isEmpty else { return nil }
        if let ms = Double(text) { return Date(timeIntervalSince1970: ms / 1000) }
        let fractional = ISO8601DateFormatter()
        fractional.formatOptions = [.withInternetDateTime, .withFractionalSeconds]
        return fractional.date(from: text) ?? ISO8601DateFormatter().date(from: text)
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
/// measured the node's text in, its colour, tracking and decoration.
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
        let base = text.font(run.map { Font($0.font) } ?? .body).foregroundStyle(ink).monospacedDigit()
        (run.map { SnapshotTextPaint.styled(base, run: $0, node: node) } ?? base)
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
/// raster from the entry's image roots or the app group, decoded as a
/// bounded thumbnail, in the default group and desaturated under the
/// accented mode unless `data-rendering="full-color"`. Both fit the content
/// box as `object-fit` says and clip to its rounded edge. Never fetched in
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
        let fit = style.string("object_fit") ?? "fill"
        let clip = RoundedOutline(radii: SnapshotPaint(style: style, dark: scheme == .dark).contentRadii(outer: CGRect(origin: .zero, size: node.frame.size), inner: box))
        Group {
            if (node.props["imageSource"] ?? "").hasPrefix("symbol:") {
                symbol(fit: fit).clipShape(clip).widgetAccentable()
            } else if let raster = environment.rasters.decode(node.props["imageSource"], roots: environment.imageRoots, appGroup: environment.appGroup,
                                                                box: box.size, fit: fit, scale: displayScale) {
                SnapshotRaster(image: Image(decorative: raster.image, scale: 1), natural: raster.natural, fit: fit, box: box.size,
                               fullColor: SnapshotAccent.group(node) == .fullColor)
                    .clipShape(clip)
            }
        }
        .frame(width: box.width, height: box.height)
        .offset(x: box.minX, y: box.minY)
    }

    /// The symbol at its natural size (the glyph's at the text's size and
    /// weight, `SymbolMeasure.size`), fitted as a raster is: the vector
    /// keeps its aspect under `fill` and `contain` as the running host
    /// draws it (`xMidYMid meet`), covers under `cover`.
    @ViewBuilder
    private func symbol(fit: String) -> some View {
        let name = node.props["symbolName"] ?? ""
        if !name.isEmpty {
            let style = SnapshotStyle(node)
            let points = style.number("font_size") ?? 17
            let weight = style.number("font_weight") ?? 400
            let ink = SnapshotColor.style(style.color("tint_color") ?? style.color("text_color"), dark: scheme == .dark, context: .ink) ?? AnyShapeStyle(.tint)
            let natural = SymbolMeasure.size(name, points: points, weight: weight) ?? CGSize(width: points, height: points)
            let image = Image(systemName: name).resizable()
            let box = node.contentBox.size
            Group {
                switch fit {
                case "cover": image.aspectRatio(natural, contentMode: .fill).frame(width: box.width, height: box.height).clipped()
                case "none", "scale-down" where natural.width <= box.width && natural.height <= box.height:
                    image.frame(width: natural.width, height: natural.height).frame(width: box.width, height: box.height)
                default: image.aspectRatio(natural, contentMode: .fit).frame(width: box.width, height: box.height)
                }
            }
            .font(.system(size: points, weight: Font.Weight(css: Int(weight))))
            .foregroundStyle(ink)
        }
    }
}

/// Raster decoding, bounded (D8: an extension's memory ceiling): ImageIO's
/// thumbnail at the pixels the box shows, never the full image, kept for
/// the invocation under a decoded-byte budget (the oldest evicted past it;
/// an image larger than the budget is never retained) and released with
/// it. `cover` shows a window of the image, so its thumbnail is the image
/// at the cover scale; any decode is bounded by `pixelBudget` in area,
/// scaled to fit it. `none` would need the image at its own scale, which
/// the profile refuses (drawn as `scale-down` meanwhile).
public final class SnapshotRasters {
    public struct Decoded {
        public let image: CGImage
        public let natural: CGSize
    }
    private struct Kept { let key: String; let value: Decoded; let bytes: Int }
    private var kept: [Kept] = []
    private var total = 0
    private let lock = NSLock()
    /// The decoded bytes kept at once.
    public let budget: Int
    /// The most pixels one decode may have; a larger one scales to fit.
    public let pixelBudget: Int
    /// How many decodes ran: a test's view of hits and evictions.
    package private(set) var decodes = 0

    public init(budget: Int = 16 << 20, pixelBudget: Int = 4_000_000) {
        self.budget = budget
        self.pixelBudget = pixelBudget
    }

    package var keptKeys: [String] { lock.lock(); defer { lock.unlock() }; return kept.map(\.key) }

    public func decode(_ source: String?, roots: [URL], appGroup: URL?, box: CGSize, fit: String, scale: CGFloat) -> Decoded? {
        guard let source, !source.isEmpty, let url = Self.resolve(source, roots: roots, appGroup: appGroup) else { return nil }
        return decode(url, box: box, fit: fit, scale: scale)
    }

    /// The decode a box needs: its own pixels for `fill`, `contain` and
    /// `scale-down`; for `cover`, the image scaled so the shorter cover
    /// dimension fills the box, which is the window shown; either scaled
    /// down to the pixel budget's area.
    public func decode(_ url: URL, box: CGSize, fit: String, scale: CGFloat) -> Decoded? {
        let pixels = CGSize(width: box.width * scale, height: box.height * scale)
        guard let natural = Self.natural(url), natural.width > 0, natural.height > 0 else { return nil }
        var size: CGSize
        if fit == "cover" {
            let cover = max(pixels.width / natural.width, pixels.height / natural.height)
            size = CGSize(width: natural.width * cover, height: natural.height * cover)
        } else {
            let contain = min(pixels.width / natural.width, pixels.height / natural.height, 1)
            size = CGSize(width: natural.width * contain, height: natural.height * contain)
            if fit == "fill" { size = CGSize(width: min(pixels.width, natural.width), height: min(pixels.height, natural.height)) }
        }
        let area = size.width * size.height
        if area > CGFloat(pixelBudget) {
            let factor = (CGFloat(pixelBudget) / area).squareRoot()
            size = CGSize(width: size.width * factor, height: size.height * factor)
        }
        return decode(url, longest: Int(max(1, max(size.width, size.height).rounded(.up))), natural: natural)
    }

    private func decode(_ url: URL, longest: Int, natural: CGSize) -> Decoded? {
        let key = "\(url.path)|\(longest)"
        lock.lock()
        if let hit = kept.first(where: { $0.key == key }) { lock.unlock(); return hit.value }
        lock.unlock()
        guard let imageSource = CGImageSourceCreateWithURL(url as CFURL, nil) else { return nil }
        let options: [CFString: Any] = [
            kCGImageSourceCreateThumbnailFromImageAlways: true,
            kCGImageSourceCreateThumbnailWithTransform: true,
            kCGImageSourceShouldCacheImmediately: true,
            kCGImageSourceThumbnailMaxPixelSize: longest,
        ]
        guard let image = CGImageSourceCreateThumbnailAtIndex(imageSource, 0, options as CFDictionary) else { return nil }
        let decoded = Decoded(image: image, natural: natural)
        let bytes = image.bytesPerRow * image.height
        lock.lock()
        decodes += 1
        if bytes <= budget {
            while total + bytes > budget, !kept.isEmpty { total -= kept.removeFirst().bytes }
            kept.append(Kept(key: key, value: decoded, bytes: bytes))
            total += bytes
        }
        lock.unlock()
        return decoded
    }

    /// The image's pixel size, upright (EXIF orientations 5–8 swap the sides).
    public static func natural(_ url: URL) -> CGSize? {
        guard let imageSource = CGImageSourceCreateWithURL(url as CFURL, nil),
              let properties = CGImageSourceCopyPropertiesAtIndex(imageSource, 0, nil) as? [CFString: Any],
              let width = (properties[kCGImagePropertyPixelWidth] as? NSNumber)?.doubleValue,
              let height = (properties[kCGImagePropertyPixelHeight] as? NSNumber)?.doubleValue else { return nil }
        let orientation = (properties[kCGImagePropertyOrientation] as? NSNumber)?.intValue ?? 1
        return orientation >= 5 ? CGSize(width: height, height: width) : CGSize(width: width, height: height)
    }

    /// The file a source names: `app:/data/…`, `app:/cache/…` and
    /// `app:/tmp/…` under the app group's container (its `data`, `cache`
    /// and `temporary` directories, the running host's names), any other
    /// name under one of the bundle roots; `..` and a root alone are
    /// refused, and the resolved path must stay under its root.
    public static func resolve(_ source: String, roots: [URL], appGroup: URL?) -> URL? {
        if source.hasPrefix("app:/") {
            let parts = source.dropFirst(5).split(separator: "/", omittingEmptySubsequences: false).map(String.init)
            let directories = ["data": "data", "cache": "cache", "tmp": "temporary"]
            guard let appGroup, parts.count >= 2, let directory = directories[parts[0]],
                  !parts.dropFirst().contains(where: { $0.isEmpty || $0 == "." || $0 == ".." }) else { return nil }
            let root = appGroup.appending(path: directory)
            return confined(parts.dropFirst().reduce(root) { $0.appending(path: $1) }, under: root)
        }
        for root in roots {
            let candidate: URL = source.hasPrefix("/") ? URL(fileURLWithPath: source)
                : root.appending(path: source.hasPrefix("assets/") ? source : "assets/" + source)
            if let url = confined(candidate, under: root) { return url }
        }
        return nil
    }

    private static func confined(_ candidate: URL, under root: URL) -> URL? {
        let base = root.standardizedFileURL.resolvingSymlinksInPath()
        let resolved = candidate.standardizedFileURL.resolvingSymlinksInPath()
        guard resolved.path.hasPrefix(base.path + "/"), FileManager.default.fileExists(atPath: resolved.path) else { return nil }
        return resolved
    }
}

/// CSS `object-fit` over the image's natural size: `fill` stretches,
/// `contain` and `cover` keep the aspect, `scale-down` keeps the natural
/// size when it fits, else contains; `none` is the profile's to refuse and
/// draws as `scale-down` here. D5: under the accented mode the raster is
/// desaturated (the default group keeps its detail) unless the author
/// asked for full colour; the modifier is iOS 18's, and older systems draw
/// the default.
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
        case "none", "scale-down":
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
