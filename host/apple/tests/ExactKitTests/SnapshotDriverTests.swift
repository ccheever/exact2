// The snapshot driver (LLP 1126.000 WP4) over real entry plans, rendered
// in-process with `ImageRenderer` as the hand-built baseline
// (`spike/1126-baseline`, `Host/Renderer.swift`) rendered itself: the same
// family frames, content margins, container stand-in and mode emulation,
// so the two sets of PNGs compare pixel for pixel. The PNGs go to
// `EXACT_WP4_OUT` when set; with `EXACT_WP4_BASELINE` naming the baseline's
// `out/`, each pair is compared and the differences written beside them.
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
        var content: CGSize { CGSize(width: size.width - 2 * margins, height: size.height - 2 * margins) }
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

    /// The frame, the system's content margins, the container background
    /// (kept in full colour, removed in the accented and vibrant modes), a
    /// 35% grey backdrop where the system shows tinted glass or wallpaper,
    /// desaturation and a dark scheme where the system composites in white.
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
                inner
                    .redacted(reason: content == .placeholder ? .placeholder : [])
                    .padding(family.margins)
            }
            .frame(width: family.size.width, height: family.size.height)
            .saturation(mode == .fullColor ? 1 : 0)
            .environment(\.colorScheme, mode == .fullColor ? scheme : .dark)
            .environment(\.widgetRenderingMode, mode == .fullColor ? .fullColor : mode == .accented ? .accented : .vibrant)
            .environment(\.dynamicTypeSize, type)
            .environment(\.redactionReasons, content == .locked ? .privacy : [])
        }
    }

    private func render<V: View>(_ view: V, size: CGSize) -> CGImage? {
        let renderer = ImageRenderer(content: view.frame(width: size.width, height: size.height))
        renderer.scale = 3
        renderer.proposedSize = ProposedViewSize(size)
        return renderer.cgImage
    }

    private func write(_ image: CGImage?, _ name: String) throws -> URL {
        let url = Self.out.appending(path: "\(name).png")
        let image = try XCTUnwrap(image, name)
        let destination = try XCTUnwrap(CGImageDestinationCreateWithURL(url as CFURL, UTType.png.identifier as CFString, 1, nil))
        CGImageDestinationAddImage(destination, image, nil)
        XCTAssertTrue(CGImageDestinationFinalize(destination), name)
        return url
    }

    /// One entry through the matrix: each family, mode, scheme and type
    /// size (`contents` beyond `answered` in full colour and vibrant at the
    /// default size, as the baseline's matrix runs).
    private func renderMatrix(plan: Data, entry: String, family: Family, prefix: String, contents: [Content], link: URL?) throws -> [String] {
        let session = try HeadlessSession(plan: plan, entry: entry, size: family.content)
        defer { session.close() }
        var names: [String] = []
        for content in contents {
            let modes: [Mode] = content == .answered ? Mode.allCases : [.fullColor, .vibrant]
            let types = content == .answered ? Self.types : [Self.types[0]]
            for mode in modes {
                for scheme in [ColorScheme.light, .dark] {
                    for type in types {
                        let environment = SnapshotEnvironment(family: family.family, placeholder: content == .placeholder, linkBase: link)
                        // The iPhone's ramp on every platform: the baseline's frames are the iPhone 17 Pro's.
                        let view = try SnapshotDriver.render(session, size: family.content, rootFontSize: SnapshotDriver.bodySize(for: type.size), environment: environment)
                        let name = "\(prefix)-\(family.name)-\(content.rawValue)-\(mode.rawValue)-\(scheme == .light ? "light" : "dark")-\(type.name)"
                        let image = render(StandIn(family: family, mode: mode, scheme: scheme, type: type.size, content: content, inner: view), size: family.size)
                        let url = try write(image, name)
                        XCTAssertEqual(image?.width, Int(family.size.width * 3), url.path)
                        names.append(name)
                    }
                }
            }
        }
        return names
    }

    // MARK: Caltrain's countdown entry

    func testCaltrainEntryRendersEveryFamilyModeAppearanceAndSize() throws {
        let plan = try Self.plan("SNAPSHOT_TRAINS")
        for family in Family.all {
            let names = try renderMatrix(plan: plan, entry: "next-trains", family: family, prefix: "trains", contents: [.answered], link: URL(string: "https://caltrain.exact.invalid"))
            XCTAssertEqual(names.count, 12, family.name)
        }
        // The entry's text is the kernel's: the station and two departures.
        let session = try HeadlessSession(plan: plan, entry: "next-trains", size: Family.all[0].content)
        defer { session.close() }
        let snapshot = try session.snapshot()
        let texts = snapshot.nodes.values.filter { $0.kind == "text" }.flatMap(\.lines).map(\.text)
        XCTAssertTrue(texts.contains("Mountain View"), "\(texts)")
        XCTAssertTrue(texts.contains { $0.hasPrefix("San") }, "\(texts)")
        XCTAssertEqual(texts.filter { $0.contains(":") }.count, 2, "two departure times: \(texts)")
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

    /// D4: each kernel line, drawn as one single-line `Text` in the font
    /// the kernel measured it in, is as wide in SwiftUI as the kernel said,
    /// at both text sizes, so the archive never re-breaks or truncates it.
    func testKernelLinesHoldTheirWidthInSwiftUI() throws {
        for (key, entry) in [("SNAPSHOT_TRAINS", "next-trains"), ("SNAPSHOT_QUESTION_MEDIUM", "question-medium")] {
            let session = try HeadlessSession(plan: try Self.plan(key), entry: entry, size: Family.all[1].content)
            defer { session.close() }
            for type in Self.types {
                try session.setRootFontSize(SnapshotDriver.bodySize(for: type.size))
                let snapshot = try session.snapshot()
                var lines = 0
                for node in snapshot.nodes.values where node.kind == "text" {
                    for line in node.lines where !line.text.isEmpty {
                        let text = line.segments.reduce(Text(verbatim: "")) { whole, segment in
                            let run = node.runs[segment.run]
                            return whole + Text(verbatim: segment.text).font(Font(run.font)).tracking(run.letterSpacing)
                        }
                        let image = render(text.lineLimit(1).fixedSize(), size: CGSize(width: 1000, height: 100))
                        // The rendered width is the image's ink-free bounds; a
                        // fixed-size single line is its own width.
                        let renderer = ImageRenderer(content: text.lineLimit(1).fixedSize())
                        renderer.proposedSize = .unspecified
                        let measured = renderer.cgImage.map { CGFloat($0.width) / renderer.scale } ?? 0
                        XCTAssertNotNil(image)
                        XCTAssertEqual(measured, line.width, accuracy: 2, "\(entry) \(type.name): \(line.text)")
                        lines += 1
                    }
                }
                XCTAssertGreaterThan(lines, 2, "\(entry) \(type.name)")
            }
        }
    }

    // MARK: The open-question fixture against the baseline

    func testQuestionEntriesRenderAndCompareWithTheBaseline() throws {
        let entries = [("SNAPSHOT_QUESTION_SMALL", "question-small", Family.all[0]), ("SNAPSHOT_QUESTION_MEDIUM", "question-medium", Family.all[1]),
                       ("SNAPSHOT_QUESTION_RECTANGULAR", "question-rectangular", Family.all[2])]
        var report: [[String: Any]] = []
        for (key, entry, family) in entries {
            let plan = try Self.plan(key)
            let names = try renderMatrix(plan: plan, entry: entry, family: family, prefix: "widget", contents: [.answered, .placeholder, .locked], link: URL(string: "interview://"))
            XCTAssertEqual(names.count, 20, entry)
            // The URL is the route's under the app's scheme, and the placeholder opens nothing.
            let session = try HeadlessSession(plan: plan, entry: entry, size: family.content)
            defer { session.close() }
            let snapshot = try session.snapshot()
            XCTAssertEqual(SnapshotDriver.url(snapshot, environment: SnapshotEnvironment(family: family.family, linkBase: URL(string: "interview://")))?.absoluteString, "interview:///prompt/q_7f3a2c")
            XCTAssertNil(SnapshotDriver.url(snapshot, environment: SnapshotEnvironment(family: family.family, placeholder: true, linkBase: URL(string: "interview://"))))
            guard let baseline = Self.baseline else { continue }
            for name in names {
                let theirs = baseline.appending(path: "\(name).png"), ours = Self.out.appending(path: "\(name).png")
                guard FileManager.default.fileExists(atPath: theirs.path) else { report.append(["name": name, "baseline": "missing"]); continue }
                let difference = try compare(ours, theirs, diff: Self.out.appending(path: "diff-\(name).png"))
                report.append(["name": name, "meanAbsoluteDifference": difference.mean, "pixelsDiffering": difference.fraction, "sameSize": difference.sameSize])
            }
        }
        if Self.baseline != nil {
            let json = try JSONSerialization.data(withJSONObject: report, options: [.prettyPrinted, .sortedKeys])
            try json.write(to: Self.out.appending(path: "compare.json"))
        }
    }

    /// Mean absolute channel difference (0–255) and the share of pixels any
    /// of whose channels differ by more than 48, with the difference written
    /// as an image (amplified three times).
    private func compare(_ a: URL, _ b: URL, diff: URL) throws -> (mean: Double, fraction: Double, sameSize: Bool) {
        func pixels(_ url: URL) throws -> (CGImage, [UInt8]) {
            let source = try XCTUnwrap(CGImageSourceCreateWithURL(url as CFURL, nil))
            let image = try XCTUnwrap(CGImageSourceCreateImageAtIndex(source, 0, nil))
            var data = [UInt8](repeating: 0, count: image.width * image.height * 4)
            let context = try XCTUnwrap(CGContext(data: &data, width: image.width, height: image.height, bitsPerComponent: 8, bytesPerRow: image.width * 4,
                                                  space: CGColorSpace(name: CGColorSpace.sRGB)!, bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue))
            context.draw(image, in: CGRect(x: 0, y: 0, width: image.width, height: image.height))
            return (image, data)
        }
        let (ours, x) = try pixels(a), (theirs, y) = try pixels(b)
        guard ours.width == theirs.width, ours.height == theirs.height else { return (255, 1, false) }
        var sum = 0, differing = 0
        var out = [UInt8](repeating: 255, count: x.count)
        for p in stride(from: 0, to: x.count, by: 4) {
            var any = false
            for c in 0..<3 {
                let d = abs(Int(x[p + c]) - Int(y[p + c]))
                sum += d
                if d > 48 { any = true }
                out[p + c] = UInt8(255 - min(255, d * 3))
            }
            if any { differing += 1 }
        }
        if let context = CGContext(data: &out, width: ours.width, height: ours.height, bitsPerComponent: 8, bytesPerRow: ours.width * 4,
                                   space: CGColorSpace(name: CGColorSpace.sRGB)!, bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue),
           let image = context.makeImage() {
            _ = try write(image, diff.deletingPathExtension().lastPathComponent)
        }
        let pixels = Double(x.count / 4)
        return (Double(sum) / (pixels * 3), Double(differing) / pixels, true)
    }

    // MARK: D6, against the shape WP3 is adding

    /// `time` and `progress` as the platform's live views, from synthetic
    /// nodes in WP3's shape (`datetime`, `format`; `start`, `end`): the
    /// mapping and a render of each.
    func testSyntheticTimeAndProgressNodesBecomeLiveViews() throws {
        let font = CTFontCreateWithName("Helvetica" as CFString, 13, nil)
        let run = HeadlessSession.Run(text: "", size: 13, weight: 400, family: 0, italic: false, font: font)
        let now = Date(timeIntervalSince1970: 1_787_915_700)
        func node(_ id: UInt32, _ kind: String, _ props: [String: String], frame: CGRect) -> HeadlessSession.Node {
            HeadlessSession.Node(id: id, kind: kind, frame: frame, props: props, style: ["text_color": ["sys": "labelColor"]], runs: [run])
        }
        let relative = node(2, "text", ["semanticTag": "time", "datetime": "1787915700000", "format": "relative"], frame: CGRect(x: 0, y: 0, width: 160, height: 16))
        let timer = node(3, "time", ["datetime": ISO8601DateFormatter().string(from: now.addingTimeInterval(300)), "format": "countdown"], frame: CGRect(x: 0, y: 20, width: 110, height: 16))
        let interval = node(4, "control", ["type": "progress", "start": "1787915700000", "end": "1787916300000"], frame: CGRect(x: 0, y: 40, width: 160, height: 4))
        let value = node(5, "control", ["type": "progress", "value": "3", "max": "8"], frame: CGRect(x: 0, y: 50, width: 160, height: 4))
        XCTAssertEqual(SnapshotLive.presentation(relative), .relative(now))
        XCTAssertEqual(SnapshotLive.presentation(timer), .timer(now.addingTimeInterval(300)))
        XCTAssertNil(SnapshotLive.presentation(value))
        XCTAssertEqual(SnapshotLive.interval(interval), now...now.addingTimeInterval(600))
        XCTAssertNil(SnapshotLive.interval(value))
        let root = HeadlessSession.Node(id: 1, kind: "view", frame: CGRect(x: 0, y: 0, width: 160, height: 60), children: [2, 3, 4, 5])
        let snapshot = HeadlessSession.Snapshot(size: CGSize(width: 160, height: 60), roots: [1],
                                                nodes: Dictionary(uniqueKeysWithValues: [root, relative, timer, interval, value].map { ($0.id, $0) }))
        let view = SnapshotDriver.view(snapshot, environment: SnapshotEnvironment(family: .systemSmall))
        let image = render(view.environment(\.colorScheme, .light).background(.white), size: CGSize(width: 160, height: 60))
        _ = try write(image, "synthetic-time-progress")
        XCTAssertEqual(image?.width, 480)
    }
}
#endif
