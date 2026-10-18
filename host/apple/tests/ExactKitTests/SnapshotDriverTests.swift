// The snapshot driver (LLP 1126.000 WP4) over real entry plans, rendered
// in-process with `ImageRenderer` as the hand-built baseline
// (`spike/1126-baseline`, `Host/Renderer.swift`) rendered itself: the same
// family frames, content margins, container stand-in and mode emulation,
// so the two sets of PNGs compare. The 3x PNGs go to `EXACT_WP4_OUT` for
// the eye; the comparison is at 1x, cropped to the content region
// (docs/testing.md), against `EXACT_WP4_BASELINE`'s `out/`, and bounded.
import XCTest
import SwiftUI
import ImageIO
import UniformTypeIdentifiers
import ExactCore
import ExactSnapshot
#if canImport(WidgetKit)
import WidgetKit
#endif

#if canImport(WidgetKit)
enum SnapshotStyleRows {
    static func lineClamp(_ node: HeadlessSession.Node) -> Int { (node.style["line_clamp"] as? NSNumber)?.intValue ?? 0 }
}

@MainActor
@available(iOS 17.0, macOS 14.0, visionOS 26.0, *)
final class SnapshotDriverTests: XCTestCase {
    enum Mode: String, CaseIterable { case fullColor, accented, vibrant }
    enum Content: String { case answered, placeholder, locked }
    struct Family {
        let name: String
        let family: SnapshotEnvironment.Family
        let size: CGSize
        let margins: CGFloat
        let corner: CGFloat
        var insets: EdgeInsets { EdgeInsets(top: margins, leading: margins, bottom: margins, trailing: margins) }
        var content: CGRect { CGRect(x: margins, y: margins, width: size.width - 2 * margins, height: size.height - 2 * margins) }
        /// The baseline's frames: the HIG's sizes for the iPhone 17 Pro's
        /// class, 16 pt margins and a 22 pt corner on the Home Screen.
        static let all = [
            Family(name: "small", family: .systemSmall, size: CGSize(width: 158, height: 158), margins: 16, corner: 22),
            Family(name: "medium", family: .systemMedium, size: CGSize(width: 338, height: 158), margins: 16, corner: 22),
            Family(name: "rectangular", family: .accessoryRectangular, size: CGSize(width: 160, height: 72), margins: 0, corner: 0),
        ]
    }
    static let types: [(name: String, size: DynamicTypeSize)] = [("L", .large), ("XXXL", .xxxLarge)]

    private static func plan(_ key: String) throws -> Data {
        let path = try XCTUnwrap(ProcessInfo.processInfo.environment["EXACT_\(key)_PLAN"], "EXACT_\(key)_PLAN: build.mjs --test compiles it")
        return try Data(contentsOf: URL(fileURLWithPath: path))
    }
    private static let out: URL = {
        let dir = ProcessInfo.processInfo.environment["EXACT_WP4_OUT"].map { URL(fileURLWithPath: $0) }
            ?? FileManager.default.temporaryDirectory.appending(path: "exact-wp4-\(ProcessInfo.processInfo.processIdentifier)")
        try? FileManager.default.createDirectory(at: dir, withIntermediateDirectories: true)
        return dir
    }()
    private static let baseline = ProcessInfo.processInfo.environment["EXACT_WP4_BASELINE"].map { URL(fileURLWithPath: $0) }

    // MARK: The stand-in for WidgetKit's container (the baseline's `WidgetContainer`)

    /// The frame, the container background (kept in full colour, removed in
    /// the accented and vibrant modes), a 35% grey backdrop where the
    /// system shows tinted glass or wallpaper, desaturation and a dark
    /// scheme where the system composites in white. The content margins
    /// are the driver's own (`SnapshotEnvironment.contentMargins`).
    struct StandIn<Inner: View>: View {
        let family: Family
        let mode: Mode
        let scheme: ColorScheme
        let type: DynamicTypeSize
        let content: Content
        let inner: Inner

        var body: some View {
            ZStack {
                if mode == .fullColor {
                    if family.margins > 0 {
                        RoundedRectangle(cornerRadius: family.corner, style: .continuous).fill(.background)
                    } else {
                        Rectangle().fill(.background)
                    }
                } else {
                    Color(white: 0.35)
                }
                inner.redacted(reason: content == .placeholder ? .placeholder : [])
            }
            .frame(width: family.size.width, height: family.size.height)
            .saturation(mode == .fullColor ? 1 : 0)
            .environment(\.colorScheme, mode == .fullColor ? scheme : .dark)
            .environment(\.widgetRenderingMode, mode == .fullColor ? .fullColor : mode == .accented ? .accented : .vibrant)
            .environment(\.dynamicTypeSize, type)
            .environment(\.redactionReasons, content == .locked ? .privacy : [])
        }
    }

    private func render<V: View>(_ view: V, size: CGSize, scale: CGFloat = 3) -> CGImage? {
        let renderer = ImageRenderer(content: view.frame(width: size.width, height: size.height))
        renderer.scale = scale
        renderer.proposedSize = ProposedViewSize(size)
        return renderer.cgImage
    }

    @discardableResult
    private func write(_ image: CGImage?, _ name: String) throws -> URL {
        let url = Self.out.appending(path: "\(name).png")
        let image = try XCTUnwrap(image, name)
        let destination = try XCTUnwrap(CGImageDestinationCreateWithURL(url as CFURL, UTType.png.identifier as CFString, 1, nil))
        CGImageDestinationAddImage(destination, image, nil)
        XCTAssertTrue(CGImageDestinationFinalize(destination), name)
        return url
    }

    /// RGBA8 pixels of an image, optionally drawn at another size (1x).
    private func pixels(_ image: CGImage, width: Int? = nil, height: Int? = nil) throws -> (width: Int, height: Int, data: [UInt8]) {
        let w = width ?? image.width, h = height ?? image.height
        var data = [UInt8](repeating: 0, count: w * h * 4)
        let context = try XCTUnwrap(CGContext(data: &data, width: w, height: h, bitsPerComponent: 8, bytesPerRow: w * 4,
                                              space: CGColorSpace(name: CGColorSpace.sRGB)!, bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue))
        context.interpolationQuality = .high
        context.draw(image, in: CGRect(x: 0, y: 0, width: w, height: h))
        return (w, h, data)
    }

    /// How many pixels in `rect` (points, y down) of a 1x image differ from
    /// `background` by more than 32 in some channel: the ink a leaf drew.
    private func ink(_ image: CGImage, in rect: CGRect, background: [UInt8] = [255, 255, 255]) throws -> Int {
        let p = try pixels(image)
        var count = 0
        let (y0, y1) = (max(0, Int(rect.minY)), min(p.height, Int(rect.maxY.rounded(.up))))
        let (x0, x1) = (max(0, Int(rect.minX)), min(p.width, Int(rect.maxX.rounded(.up))))
        guard y0 < y1, x0 < x1 else { return 0 }
        for y in y0..<y1 {
            for x in x0..<x1 {
                let at = (y * p.width + x) * 4
                if (0..<3).contains(where: { abs(Int(p.data[at + $0]) - Int(background[$0])) > 32 }) { count += 1 }
            }
        }
        return count
    }

    private func color(_ image: CGImage, x: Int, y: Int) throws -> [Int] {
        let p = try pixels(image)
        let at = (y * p.width + x) * 4
        return [Int(p.data[at]), Int(p.data[at + 1]), Int(p.data[at + 2])]
    }

    /// A 1x render of an entry alone on white in the light scheme, and its tree.
    private func plain(_ session: HeadlessSession, family: Family, environment: SnapshotEnvironment) throws -> (CGImage, HeadlessSession.Snapshot) {
        let view = SnapshotDriver.view(session: session, displaySize: family.size, environment: environment)
            .environment(\.colorScheme, .light).background(.white)
        let image = try XCTUnwrap(render(view, size: family.size, scale: 1))
        return (image, try session.snapshot())
    }

    private func environment(_ family: Family, content: Content = .answered, type: DynamicTypeSize = .large, link: URL? = nil) -> SnapshotEnvironment {
        // The iPhone's ramp on every platform: the baseline's frames are the iPhone 17 Pro's.
        SnapshotEnvironment(family: family.family, placeholder: content == .placeholder, linkBase: link,
                            contentMargins: family.insets, rootFontSize: SnapshotDriver.bodySize(for: type))
    }

    /// One entry through the matrix: each family, mode, scheme and type
    /// size (`contents` beyond `answered` in full colour and vibrant at the
    /// default size, as the baseline's matrix runs).
    private func renderMatrix(session: HeadlessSession, family: Family, prefix: String, contents: [Content], link: URL?) throws -> [String] {
        var names: [String] = []
        for content in contents {
            let modes: [Mode] = content == .answered ? Mode.allCases : [.fullColor, .vibrant]
            let types = content == .answered ? Self.types : [Self.types[0]]
            for mode in modes {
                for scheme in [ColorScheme.light, .dark] {
                    for type in types {
                        let view = SnapshotDriver.view(session: session, displaySize: family.size, environment: environment(family, content: content, type: type.size, link: link))
                        let name = "\(prefix)-\(family.name)-\(content.rawValue)-\(mode.rawValue)-\(scheme == .light ? "light" : "dark")-\(type.name)"
                        let image = render(StandIn(family: family, mode: mode, scheme: scheme, type: type.size, content: content, inner: view), size: family.size)
                        try write(image, name)
                        XCTAssertEqual(image?.width, Int(family.size.width * 3), name)
                        names.append(name)
                    }
                }
            }
        }
        return names
    }

    /// Every text node's lines drew ink inside its frame (the root's, in
    /// the content region, at 1x).
    private func assertTextDrew(_ image: CGImage, _ snapshot: HeadlessSession.Snapshot, family: Family, _ message: String) throws {
        var drawn = 0
        for node in snapshot.nodes.values where node.kind == "text" && !node.lines.isEmpty && node.frame.width > 2 {
            let frame = try absoluteFrame(of: node, in: snapshot).offsetBy(dx: family.content.minX, dy: family.content.minY)
            XCTAssertGreaterThan(try ink(image, in: frame), 4, "\(message): \(node.lines.map(\.text))")
            drawn += 1
        }
        XCTAssertGreaterThan(drawn, 0, message)
    }

    private func absoluteFrame(of node: HeadlessSession.Node, in snapshot: HeadlessSession.Snapshot) throws -> CGRect {
        var frame = node.frame
        var parent = snapshot.nodes.values.first { $0.children.contains(node.id) }
        while let p = parent { frame = frame.offsetBy(dx: p.frame.minX, dy: p.frame.minY); parent = snapshot.nodes.values.first { $0.children.contains(p.id) } }
        return frame
    }

    // MARK: Caltrain's countdown entry

    func testCaltrainEntryRendersEveryFamilyModeAppearanceAndSize() throws {
        let plan = try Self.plan("SNAPSHOT_TRAINS")
        for family in Family.all {
            let session = try HeadlessSession(plan: plan, entry: "next-trains", size: family.content.size)
            defer { session.close() }
            let names = try renderMatrix(session: session, family: family, prefix: "trains", contents: [.answered], link: URL(string: "https://caltrain.exact.invalid"))
            XCTAssertEqual(names.count, 12, family.name)
            let (image, snapshot) = try plain(session, family: family, environment: environment(family))
            try assertTextDrew(image, snapshot, family: family, "trains \(family.name)")
            let texts = snapshot.nodes.values.filter { $0.kind == "text" }.flatMap(\.lines).map(\.text)
            XCTAssertTrue(texts.contains("Mountain View"), "\(texts)")
            XCTAssertTrue(texts.contains { $0.hasPrefix("San") }, "\(texts)")
            XCTAssertEqual(texts.filter { $0.contains(":") }.count, 2, "two departure times: \(texts)")
        }
    }

    /// A running host's boot refuses an entry plan, and the entry boots by
    /// its kind alone.
    func testAnEntryPlanBootsOnlyByItsKind() throws {
        let plan = try Self.plan("SNAPSHOT_TRAINS")
        XCTAssertThrowsError(try HeadlessSession(plan: plan, size: CGSize(width: 100, height: 100)))
        XCTAssertThrowsError(try HeadlessSession(plan: plan, entry: "other", size: CGSize(width: 100, height: 100)))
        let session = try HeadlessSession(plan: plan, entry: "next-trains", size: CGSize(width: 100, height: 100))
        session.close()
    }

    /// D4: each kernel line, drawn by the driver as one single-line `Text`
    /// in the font the kernel measured it in, is as wide in SwiftUI as the
    /// kernel said, at both text sizes, so the archive never re-breaks or
    /// truncates it: the ink of a line's view stays inside the kernel's
    /// frame for it and reaches its end.
    func testKernelLinesHoldTheirWidthInSwiftUI() throws {
        for (key, entry) in [("SNAPSHOT_TRAINS", "next-trains"), ("SNAPSHOT_QUESTION_MEDIUM", "question-medium")] {
            let family = Family.all[1]
            let session = try HeadlessSession(plan: try Self.plan(key), entry: entry, size: family.content.size)
            defer { session.close() }
            for type in Self.types {
                let (image, snapshot) = try plain(session, family: family, environment: environment(family, type: type.size))
                var lines = 0
                for node in snapshot.nodes.values where node.kind == "text" {
                    let origin = try absoluteFrame(of: node, in: snapshot).origin
                    let clamped = SnapshotStyleRows.lineClamp(node) > 0
                    for (index, line) in node.lines.enumerated() where line.width > 8 {
                        let box = CGRect(x: origin.x + node.contentBox.minX + line.origin.x + family.content.minX, y: origin.y + node.contentBox.minY + line.origin.y - line.ascent + family.content.minY,
                                         width: line.width, height: line.ascent + line.descent)
                        // A line outside the content region cannot be seen (XXXL overflows);
                        // a clamp's last line is wider than its text until WP1 supplies the cut and "…".
                        guard family.content.contains(box), !(clamped && index == node.lines.count - 1) else { continue }
                        // Ink in the line's last fifth, none in the 6 pt past its end.
                        XCTAssertGreaterThan(try ink(image, in: CGRect(x: box.maxX - box.width / 5, y: box.minY, width: box.width / 5, height: box.height)), 0, "\(entry) \(type.name): \(line.text)")
                        XCTAssertEqual(try ink(image, in: CGRect(x: box.maxX + 2, y: box.minY, width: 4, height: box.height)), 0, "\(entry) \(type.name): past \(line.text)")
                        lines += 1
                    }
                }
                XCTAssertGreaterThanOrEqual(lines, 2, "\(entry) \(type.name)")
            }
        }
    }

    // MARK: The open-question fixture against the baseline

    /// The bound on the mean 1x channel difference against the baseline,
    /// cropped to the content region, and the known exceptions: at XXXL
    /// SwiftUI drops lines to fit the height where CSS clamps to the
    /// authored count (QUEUE: `line-clamp: auto`); the rectangular
    /// placeholder's pills follow the kernel's lines; and until WP1
    /// supplies a clamped line's cut and "…", the clamp's last line shows
    /// as it broke (the `answered` cells).
    private func bound(_ name: String) -> Double {
        if name.contains("XXXL") { return 90 }
        if name.contains("rectangular-placeholder") { return 60 }
        return 40
    }

    func testQuestionEntriesRenderAndCompareWithTheBaseline() throws {
        let entries = [("SNAPSHOT_QUESTION_SMALL", "question-small", Family.all[0]), ("SNAPSHOT_QUESTION_MEDIUM", "question-medium", Family.all[1]),
                       ("SNAPSHOT_QUESTION_RECTANGULAR", "question-rectangular", Family.all[2])]
        var report: [[String: Any]] = []
        var names: [(String, Family)] = []
        for (key, entry, family) in entries {
            let session = try HeadlessSession(plan: try Self.plan(key), entry: entry, size: family.content.size)
            defer { session.close() }
            let rendered = try renderMatrix(session: session, family: family, prefix: "widget", contents: [.answered, .placeholder, .locked], link: URL(string: "interview://"))
            XCTAssertEqual(rendered.count, 20, entry)
            names += rendered.map { ($0, family) }
            let (image, snapshot) = try plain(session, family: family, environment: environment(family))
            try assertTextDrew(image, snapshot, family: family, entry)
            // The URL is the route's under the app's scheme, query and all; the placeholder opens nothing.
            XCTAssertEqual(SnapshotDriver.url(snapshot, environment: environment(family, link: URL(string: "interview://")))?.absoluteString, "interview:///prompt/q_7f3a2c")
            XCTAssertNil(SnapshotDriver.url(snapshot, environment: environment(family, content: .placeholder, link: URL(string: "interview://"))))
        }
        #if os(macOS)
        throw XCTSkip("the baseline is the iPhone's (its text ramp, its colours): compare on the iOS simulator")
        #else
        guard let baseline = Self.baseline else { throw XCTSkip("EXACT_WP4_BASELINE names the baseline's out/ for the comparison") }
        for (name, family) in names {
            let theirs = baseline.appending(path: "\(name).png"), ours = Self.out.appending(path: "\(name).png")
            guard FileManager.default.fileExists(atPath: theirs.path) else { XCTFail("no baseline \(theirs.path)"); continue }
            let difference = try compare(ours, theirs, region: family.content, diff: Self.out.appending(path: "diff-\(name).png"))
            XCTAssertTrue(difference.sameSize, name)
            XCTAssertLessThanOrEqual(difference.mean, bound(name), "\(name): mean \(difference.mean), \(Int(difference.fraction * 100))% of pixels differ")
            report.append(["name": name, "meanAbsoluteDifference": difference.mean, "pixelsDiffering": difference.fraction, "bound": bound(name)])
        }
        try JSONSerialization.data(withJSONObject: report, options: [.prettyPrinted, .sortedKeys]).write(to: Self.out.appending(path: "compare.json"))
        #endif
    }

    /// Mean absolute channel difference (0–255) at 1x over `region`
    /// (points) and the share of its pixels any of whose channels differ by
    /// more than 48, with the difference written as an image (amplified).
    private func compare(_ a: URL, _ b: URL, region: CGRect, diff: URL) throws -> (mean: Double, fraction: Double, sameSize: Bool) {
        func load(_ url: URL) throws -> CGImage {
            let source = try XCTUnwrap(CGImageSourceCreateWithURL(url as CFURL, nil))
            return try XCTUnwrap(CGImageSourceCreateImageAtIndex(source, 0, nil))
        }
        let ours = try load(a), theirs = try load(b)
        guard ours.width == theirs.width, ours.height == theirs.height else { return (255, 1, false) }
        let w = ours.width / 3, h = ours.height / 3
        let x = try pixels(ours, width: w, height: h), y = try pixels(theirs, width: w, height: h)
        var sum = 0, differing = 0, counted = 0
        var out = [UInt8](repeating: 255, count: x.data.count)
        let (y0, y1) = (max(0, Int(region.minY)), min(h, Int(region.maxY))), (x0, x1) = (max(0, Int(region.minX)), min(w, Int(region.maxX)))
        for py in stride(from: y0, to: y1, by: 1) {
            for px in stride(from: x0, to: x1, by: 1) {
                let p = (py * w + px) * 4
                var any = false
                for c in 0..<3 {
                    let d = abs(Int(x.data[p + c]) - Int(y.data[p + c]))
                    sum += d
                    if d > 48 { any = true }
                    out[p + c] = UInt8(255 - min(255, d * 3))
                }
                if any { differing += 1 }
                counted += 1
            }
        }
        if let context = CGContext(data: &out, width: w, height: h, bitsPerComponent: 8, bytesPerRow: w * 4,
                                   space: CGColorSpace(name: CGColorSpace.sRGB)!, bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue),
           let image = context.makeImage() {
            try write(image, diff.deletingPathExtension().lastPathComponent)
        }
        return (Double(sum) / Double(max(1, counted) * 3), Double(differing) / Double(max(1, counted)), true)
    }

    // MARK: Paint: gradients in the batch's forms, dashed borders

    func testGradientsPaintInTheBatchsOrderShapesAndSpaces() throws {
        let family = Family.all[1]
        let session = try HeadlessSession(plan: try Self.plan("SNAPSHOT_GRADIENT"), entry: "gradient-card", size: family.content.size)
        defer { session.close() }
        let (image, snapshot) = try plain(session, family: family, environment: environment(family))
        try write(render(SnapshotDriver.view(session: session, displaySize: family.size, environment: environment(family)).environment(\.colorScheme, .light).background(.white), size: family.size), "gradients")
        let root = try XCTUnwrap(snapshot.roots.first.flatMap { snapshot.nodes[$0] })
        let strips = root.children.compactMap { snapshot.nodes[$0] }
        XCTAssertEqual(strips.count, 8)
        func frame(_ strip: HeadlessSession.Node) -> CGRect { strip.frame.offsetBy(dx: root.frame.minX + family.content.minX, dy: root.frame.minY + family.content.minY) }
        func sample(_ strip: HeadlessSession.Node, x: CGFloat, y: CGFloat) throws -> [Int] {
            let f = frame(strip)
            return try color(image, x: Int(f.minX + f.width * x), y: Int(f.minY + f.height * y))
        }
        // Axial, red to blue: the batch's `t, r, g, b, a` order read as such.
        let left = try sample(strips[0], x: 0.03, y: 0.5), right = try sample(strips[0], x: 0.97, y: 0.5)
        XCTAssertGreaterThan(left[0], 200, "\(left)"); XCTAssertLessThan(left[2], 60, "\(left)")
        XCTAssertGreaterThan(right[2], 200, "\(right)"); XCTAssertLessThan(right[0], 60, "\(right)")
        // An ellipse, farthest side, on a wide box: green at the centre, black
        // at the side's end and at the top's middle (a circle of the farthest
        // side would still be green there).
        let centre = try sample(strips[1], x: 0.5, y: 0.5), side = try sample(strips[1], x: 0.01, y: 0.5), top = try sample(strips[1], x: 0.5, y: 0.1)
        XCTAssertGreaterThan(centre[1], 200, "\(centre)"); XCTAssertLessThan(side[1], 70, "\(side)"); XCTAssertLessThan(top[1], 70, "\(top)")
        // A length stop: the kernel places it; red up to 20 px, blue from 80%.
        let before = try sample(strips[2], x: 0.03, y: 0.5), after = try sample(strips[2], x: 0.97, y: 0.5)
        XCTAssertGreaterThan(before[0], 200, "\(before)"); XCTAssertGreaterThan(after[2], 200, "\(after)")
        // 45° on a wide box: colour is a function of the projection onto the
        // CSS direction in points, so two points with one projection match
        // (the unit-square mapping would give them different colours).
        let f = frame(strips[3])
        let p1 = try color(image, x: Int(f.midX + 5), y: Int(f.midY + 5)), p2 = try color(image, x: Int(f.midX - 5), y: Int(f.midY - 5))
        XCTAssertLessThan(abs(p1[0] - p2[0]) + abs(p1[2] - p2[2]), 24, "\(p1) \(p2)")
        let lowLeft = try sample(strips[3], x: 0.05, y: 0.9), highRight = try sample(strips[3], x: 0.95, y: 0.1)
        XCTAssertGreaterThan(lowLeft[0], highRight[0]); XCTAssertGreaterThan(highRight[2], lowLeft[2])
        // Wide stops cross as a linear-sRGB ramp (LLP 1100 D2), read as such:
        // the midpoint is brighter than an sRGB-space mix (128, 0, 128), and
        // not the black a /255 reading of the floats would give.
        let mid = try sample(strips[4], x: 0.5, y: 0.5)
        XCTAssertGreaterThan(mid[0], 140, "\(mid)"); XCTAssertGreaterThan(mid[2], 140, "\(mid)")
        // A dashed border: on the top edge, red in the first dash (3 widths),
        // the background in the gap after it.
        let d = frame(strips[5])
        let dash = try color(image, x: Int(d.minX + 3), y: Int(d.minY + 1)), gap = try color(image, x: Int(d.minX + 9), y: Int(d.minY + 1))
        XCTAssertGreaterThan(dash[0], 200, "\(dash)"); XCTAssertLessThan(dash[1], 80, "\(dash)")
        XCTAssertGreaterThan(gap[1], 200, "\(gap)")
        // The elliptical one drew, and nothing inside its curve.
        let e = frame(strips[6])
        XCTAssertGreaterThan(try ink(image, in: e), 10)
        XCTAssertEqual(try ink(image, in: e.insetBy(dx: 60, dy: 6)), 0)
    }

    /// Raster decoding is bounded (D8): a large image decodes to a
    /// thumbnail no larger than the frame's pixels, its natural size kept.
    func testRastersDecodeAsBoundedThumbnails() throws {
        let dir = Self.out.appending(path: "assets")
        try FileManager.default.createDirectory(at: dir, withIntermediateDirectories: true)
        let big = dir.appending(path: "big.png")
        let context = try XCTUnwrap(CGContext(data: nil, width: 2000, height: 1500, bitsPerComponent: 8, bytesPerRow: 0, space: CGColorSpace(name: CGColorSpace.sRGB)!, bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue))
        context.setFillColor(CGColor(srgbRed: 1, green: 0, blue: 0, alpha: 1)); context.fill(CGRect(x: 0, y: 0, width: 2000, height: 1500))
        try write(context.makeImage(), "assets/big")
        let decoded = try XCTUnwrap(SnapshotRasters.decode("big.png", roots: [Self.out], pixels: CGSize(width: 120, height: 120)))
        XCTAssertEqual(decoded.natural, CGSize(width: 2000, height: 1500))
        XCTAssertLessThanOrEqual(max(decoded.image.width, decoded.image.height), 120)
        XCTAssertNil(SnapshotRasters.decode("../../big.png", roots: [dir], pixels: CGSize(width: 10, height: 10)), "a path may not leave its root")
        XCTAssertNil(SnapshotRasters.decode("big.png", roots: [dir.appending(path: "missing")], pixels: CGSize(width: 10, height: 10)))
    }

    // MARK: D5: the accented groups

    /// Text and symbols are the accented group, a raster the default one,
    /// desaturated unless the author asked for full colour: the decision,
    /// and under the accented rendering mode a red raster renders grey by
    /// default and red with `data-rendering="full-color"`.
    func testTextAndSymbolsAreTheAccentedGroupAndRastersDesaturate() throws {
        let font = CTFontCreateWithName("Helvetica" as CFString, 13, nil)
        func node(_ id: UInt32, _ kind: String, _ props: [String: String] = [:], frame: CGRect = CGRect(x: 0, y: 0, width: 10, height: 10)) -> HeadlessSession.Node {
            HeadlessSession.Node(id: id, kind: kind, frame: frame, props: props, runs: [HeadlessSession.Run(text: "", size: 13, weight: 400, family: 0, italic: false, font: font)])
        }
        XCTAssertEqual(SnapshotAccent.group(node(1, "text")), .accented)
        XCTAssertEqual(SnapshotAccent.group(node(1, "time")), .accented)
        XCTAssertEqual(SnapshotAccent.group(node(1, "image", ["imageSource": "symbol:sf/arrow.up", "symbolName": "arrow.up"])), .accented)
        XCTAssertEqual(SnapshotAccent.group(node(1, "image", ["imageSource": "red.png"])), .desaturated)
        XCTAssertEqual(SnapshotAccent.group(node(1, "image", ["imageSource": "red.png", "dataset": "{\"rendering\":\"full-color\"}"])), .fullColor)
        XCTAssertNil(SnapshotAccent.group(node(1, "view")))
        // A red raster fixture, rendered under the accented mode.
        let dir = Self.out.appending(path: "assets")
        try FileManager.default.createDirectory(at: dir, withIntermediateDirectories: true)
        let context = try XCTUnwrap(CGContext(data: nil, width: 40, height: 40, bitsPerComponent: 8, bytesPerRow: 0, space: CGColorSpace(name: CGColorSpace.sRGB)!, bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue))
        context.setFillColor(CGColor(srgbRed: 1, green: 0, blue: 0, alpha: 1)); context.fill(CGRect(x: 0, y: 0, width: 40, height: 40))
        try write(context.makeImage(), "assets/red")
        func render(_ props: [String: String]) throws -> [Int] {
            let image = node(2, "image", props, frame: CGRect(x: 0, y: 0, width: 40, height: 40))
            let root = HeadlessSession.Node(id: 1, kind: "view", frame: CGRect(x: 0, y: 0, width: 40, height: 40), children: [2])
            let snapshot = HeadlessSession.Snapshot(size: CGSize(width: 40, height: 40), roots: [1], nodes: [1: root, 2: image])
            let view = SnapshotDriver.view(snapshot, environment: SnapshotEnvironment(family: .systemSmall, imageRoots: [Self.out], contentMargins: EdgeInsets()))
                .environment(\.widgetRenderingMode, .accented).environment(\.colorScheme, .light).background(.white)
            let rendered = try XCTUnwrap(self.render(view, size: CGSize(width: 40, height: 40), scale: 1))
            return try color(rendered, x: 20, y: 20)
        }
        let full = try render(["imageSource": "red.png", "dataset": "{\"rendering\":\"full-color\"}"])
        XCTAssertGreaterThan(full[0], 200, "full colour stays red: \(full)"); XCTAssertLessThan(full[1], 60, "\(full)")
        let desaturated = try render(["imageSource": "red.png"])
        if #available(iOS 18.0, macOS 15.0, *) {
            XCTAssertLessThan(abs(desaturated[0] - desaturated[1]), 40, "desaturated by default: \(desaturated)")
        }
    }

    // MARK: D6, against the shape WP3 is adding

    /// `time` and `progress` as the platform's live views, from synthetic
    /// nodes in WP3's shape (`datetime`, `format`; `start`, `end`): the
    /// mapping (a countdown from the entry's date, fractional ISO 8601
    /// accepted, calendar formats as the kernel's text), and each leaf
    /// drawing inside its frame.
    func testSyntheticTimeAndProgressNodesBecomeLiveViews() throws {
        let font = CTFontCreateWithName("Helvetica" as CFString, 13, nil)
        let run = HeadlessSession.Run(text: "", size: 13, weight: 400, family: 0, italic: false, font: font)
        let now = Date(timeIntervalSince1970: 1_787_915_700), entry = now.addingTimeInterval(-60)
        func node(_ id: UInt32, _ kind: String, _ props: [String: String], frame: CGRect, lines: [HeadlessSession.Line] = []) -> HeadlessSession.Node {
            HeadlessSession.Node(id: id, kind: kind, frame: frame, props: props, style: ["text_color": ["sys": "labelColor"]], lines: lines, runs: [run])
        }
        let relative = node(2, "text", ["semanticTag": "time", "datetime": "1787915700000", "format": "relative"], frame: CGRect(x: 0, y: 0, width: 160, height: 16))
        let iso = ISO8601DateFormatter(); iso.formatOptions = [.withInternetDateTime, .withFractionalSeconds]
        let countdown = node(3, "time", ["datetime": iso.string(from: now.addingTimeInterval(300.5)), "format": "countdown"], frame: CGRect(x: 0, y: 20, width: 110, height: 16))
        let interval = node(4, "control", ["type": "progress", "start": "1787915700000", "end": "1787916300000"], frame: CGRect(x: 0, y: 40, width: 160, height: 6))
        let value = node(5, "control", ["type": "progress", "value": "3", "max": "8"], frame: CGRect(x: 0, y: 50, width: 160, height: 6))
        let calendar = node(6, "time", ["datetime": "1787915700000", "format": "date-short"], frame: CGRect(x: 0, y: 60, width: 160, height: 16),
                            lines: [HeadlessSession.Line(text: "7/26/26", range: NSRange(location: 0, length: 7), origin: CGPoint(x: 0, y: 12), width: 48, ascent: 12, descent: 3)])
        XCTAssertEqual(SnapshotLive.presentation(relative, entryDate: entry), .relative(now))
        XCTAssertEqual(SnapshotLive.presentation(countdown, entryDate: entry), .countdown(entry...now.addingTimeInterval(300.5)))
        XCTAssertEqual(SnapshotLive.presentation(node(7, "time", ["datetime": "1787915700000", "format": "timer"], frame: .zero), entryDate: entry), .timer(now))
        XCTAssertNil(SnapshotLive.presentation(calendar, entryDate: entry), "a calendar format is the kernel's text")
        XCTAssertNil(SnapshotLive.presentation(value, entryDate: entry))
        XCTAssertEqual(SnapshotLive.instant("2026-07-26T12:35:00Z"), ISO8601DateFormatter().date(from: "2026-07-26T12:35:00Z"))
        XCTAssertEqual(SnapshotLive.interval(interval), now...now.addingTimeInterval(600))
        XCTAssertNil(SnapshotLive.interval(value))
        let root = HeadlessSession.Node(id: 1, kind: "view", frame: CGRect(x: 0, y: 0, width: 160, height: 80), children: [2, 3, 4, 5, 6])
        let snapshot = HeadlessSession.Snapshot(size: CGSize(width: 160, height: 80), roots: [1],
                                                nodes: Dictionary(uniqueKeysWithValues: [root, relative, countdown, interval, value, calendar].map { ($0.id, $0) }))
        let environment = SnapshotEnvironment(family: .systemSmall, contentMargins: EdgeInsets(), entryDate: entry)
        let view = SnapshotDriver.view(snapshot, environment: environment).environment(\.colorScheme, .light).background(.white)
        let image = try XCTUnwrap(render(view, size: CGSize(width: 160, height: 80), scale: 1))
        try write(render(view, size: CGSize(width: 160, height: 80)), "synthetic-time-progress")
        for leaf in [relative, countdown, interval, value, calendar] {
            XCTAssertGreaterThan(try ink(image, in: leaf.frame), 4, "\(leaf.kind) \(leaf.props) drew nothing")
        }
    }

    /// A real `time` node from the kernel carries its runs, so the live
    /// text takes the font the kernel measured it in.
    func testARealTimeNodeCarriesItsMeasuredFont() throws {
        let family = Family.all[0]
        let session = try HeadlessSession(plan: try Self.plan("SNAPSHOT_TRAINS"), entry: "next-trains", size: family.content.size)
        defer { session.close() }
        let snapshot = try session.snapshot()
        for node in snapshot.nodes.values where node.kind == "text" || node.kind == "time" {
            XCTAssertFalse(node.runs.isEmpty, "\(node.kind) \(node.props)")
        }
    }
}
#endif
