// A Live Activity's placements through the snapshot driver (LLP 1126.001
// WP10): Caltrain's two activities and a tinted fixture, each placement
// laid out at the widths the system proposes to a region (finite, none,
// zero, infinite), rendered in-process with `ImageRenderer` as the widget
// tests are, with the envelope's state, the kept last-good state and the
// preview as placeholder. The 1x PNGs go to `EXACT_WP10_OUT` for the eye.
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
final class ActivityDriverTests: XCTestCase {
    private static func plan(_ key: String) throws -> Data {
        let path = try XCTUnwrap(ProcessInfo.processInfo.environment["EXACT_\(key)_PLAN"], "EXACT_\(key)_PLAN: build.mjs --test compiles it")
        return try Data(contentsOf: URL(fileURLWithPath: path))
    }
    private static let out: URL = {
        let dir = ProcessInfo.processInfo.environment["EXACT_WP10_OUT"].map { URL(fileURLWithPath: $0) }
            ?? FileManager.default.temporaryDirectory.appending(path: "exact-wp10-\(ProcessInfo.processInfo.processIdentifier)")
        try? FileManager.default.createDirectory(at: dir, withIntermediateDirectories: true)
        return dir
    }()
    private static let facts = SessionFacts(displayScale: 1, preferences: [], rootFontSize: 17)
    private static let assets = AssetResolver(root: FileManager.default.temporaryDirectory)

    /// The widths stage 0 saw the system propose (one phone's evidence): the
    /// expanded leading and trailing regions, the centre, the bottom and the
    /// Lock Screen; the tests take them as a set of region widths, not as facts.
    private static let widths: [CGFloat] = [165, 109.33, 338, 362]

    private func render(_ plan: Data, kind: String, state: String?) throws -> ActivityRender {
        try ActivityRender(plan: plan, kind: kind, state: state.map { Data($0.utf8) }, facts: Self.facts, assets: Self.assets)
    }

    /// No content margins: the placement's view is its kernel size, so the
    /// sizes compare; the Lock Screen's margins have their own assertion.
    private func environment(_ placement: ActivityPlacement, placeholder: Bool = false, link: URL? = nil, margins: EdgeInsets? = EdgeInsets()) -> SnapshotEnvironment {
        SnapshotEnvironment(family: .liveActivity, placement: placement, placeholder: placeholder, linkBase: link, contentMargins: margins, rootFontSize: 17)
    }

    private func image<V: View>(_ view: V, proposal: ProposedViewSize) -> CGImage? {
        let renderer = ImageRenderer(content: view)
        renderer.scale = 1
        renderer.proposedSize = proposal
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

    /// How many pixels of a 1x image differ from `background` by more than
    /// 32 in some channel: the ink the leaves drew.
    private func ink(_ image: CGImage, background: [UInt8]) throws -> Int {
        let w = image.width, h = image.height
        var data = [UInt8](repeating: 0, count: w * h * 4)
        let context = try XCTUnwrap(CGContext(data: &data, width: w, height: h, bitsPerComponent: 8, bytesPerRow: w * 4,
                                              space: CGColorSpace(name: CGColorSpace.sRGB)!, bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue))
        context.draw(image, in: CGRect(x: 0, y: 0, width: w, height: h))
        var count = 0
        for at in stride(from: 0, to: data.count, by: 4) where (0..<3).contains(where: { abs(Int(data[at + $0]) - Int(background[$0])) > 32 }) { count += 1 }
        return count
    }

    private func texts(_ laid: ActivityRender.Laid) -> [String] {
        var found: [String] = [], stack = [laid.root.id]
        while let id = stack.popLast() {
            guard let node = laid.snapshot.nodes[id] else { continue }
            if node.kind == "text" || node.kind == "time" { found.append(contentsOf: node.lines.map(\.text)) }
            stack.append(contentsOf: node.children.reversed())
        }
        return found
    }

    // MARK: Caltrain's two activities, every placement, at the region widths

    func testCaltrainPlacementsLayOutAtEveryRegionWidthAndDraw() throws {
        for (key, kind, dark) in [("SNAPSHOT_DEPARTURE", "departure", false), ("SNAPSHOT_SERVICE_ALERT", "service-alert", true)] {
            let render = try self.render(try Self.plan(key), kind: kind, state: nil)
            for placement in ActivityPlacement.allCases {
                for width in Self.widths {
                    let proposal = ProposedViewSize(width: width, height: nil)
                    let size = render.size(placement, proposal: proposal)
                    XCTAssertGreaterThan(size.height, 0, "\(kind) \(placement.name) at \(width)")
                    if placement.fillsWidth { XCTAssertEqual(size.width, width, "\(kind) \(placement.name) fills its region") }
                    else { XCTAssertLessThanOrEqual(size.width, width, "\(kind) \(placement.name) shrinks to its content") }
                    let laid = try render.layout(placement, width: size.width, height: nil)
                    XCTAssertEqual(laid.root.frame.size, size, "\(kind) \(placement.name): the view's size is the placement root's")
                    XCTAssertEqual(laid.viewport, size, "\(kind) \(placement.name): snapshot_viewport reads the placement's size")
                    let view = SnapshotActivity.view(render, placement: placement, environment: environment(placement))
                        .environment(\.colorScheme, dark ? .dark : .light).background(dark ? Color.black : Color.white)
                    let name = "\(kind)-\(placement.name)-\(Int(width))"
                    let image = image(view, proposal: proposal)
                    try write(image, name)
                    XCTAssertEqual(image?.width, Int(size.width.rounded(.up)), name)
                    XCTAssertEqual(image?.height, Int(size.height.rounded(.up)), name)
                    if !texts(laid).isEmpty || placement == .minimal {
                        XCTAssertGreaterThan(try ink(try XCTUnwrap(image), background: dark ? [0, 0, 0] : [255, 255, 255]), 8, "\(name) drew")
                    }
                }
                // The system's probes before a finite width: nothing for zero, the content's own for none and infinity.
                XCTAssertEqual(render.size(placement, proposal: .zero), .zero, "\(kind) \(placement.name) minimum")
                let ideal = render.size(placement, proposal: .unspecified)
                XCTAssertEqual(ideal, render.size(placement, proposal: .infinity), "\(kind) \(placement.name) ideal is the maximum")
                XCTAssertGreaterThan(ideal.width, 0); XCTAssertLessThanOrEqual(ideal.width, ActivityRender.widest)
            }
            // The preview state is what an unstated state renders.
            let lock = try render.layout(.lockScreen, width: 362, height: nil)
            XCTAssertTrue(texts(lock).contains { $0.contains(kind == "departure" ? "Mountain View" : "Service delay") }, "\(texts(lock))")
            // D5: the Lock Screen placement sits inside the content margins, the platform's 16 pt when the system gives none.
            let padded = image(SnapshotActivity.view(render, placement: .lockScreen, environment: environment(.lockScreen, margins: nil)), proposal: ProposedViewSize(width: 362, height: nil))
            let inner = try render.layout(.lockScreen, width: 362 - 32, height: nil)
            XCTAssertEqual(padded?.width, 362); XCTAssertEqual(padded?.height, Int((inner.root.frame.height + 32).rounded(.up)), kind)
            // The Island's regions take no margins, and a region's fixed height is the viewport's while the view stays its content's height.
            let compact = try render.layout(.compactLeading, width: render.size(.compactLeading, proposal: ProposedViewSize(width: 100, height: 22)).width, height: 22)
            XCTAssertEqual(compact.viewport.height, 22)
            XCTAssertEqual(image(SnapshotActivity.view(render, placement: .compactLeading, environment: environment(.compactLeading, margins: nil)), proposal: ProposedViewSize(width: 100, height: 22))?.height, Int(compact.root.frame.height.rounded(.up)), kind)
        }
    }

    /// The compact regions and the minimal view are as wide as what they
    /// hold: a train number, a countdown in the width the kernel reserved
    /// for its widest value, a symbol.
    func testCompactPlacementsAreAsWideAsTheirContent() throws {
        let render = try self.render(try Self.plan("SNAPSHOT_DEPARTURE"), kind: "departure", state: nil)
        let leading = render.size(.compactLeading, proposal: ProposedViewSize(width: 200, height: 22.667))
        XCTAssertGreaterThan(leading.width, 8); XCTAssertLessThan(leading.width, 60, "the train number: \(leading)")
        let trailing = render.size(.compactTrailing, proposal: ProposedViewSize(width: 200, height: 22.667))
        let laid = try render.layout(.compactTrailing, width: trailing.width, height: 22.667)
        let time = try XCTUnwrap(laid.snapshot.nodes.values.first { $0.kind == "time" || $0.props["semanticTag"] == "time" })
        XCTAssertNotNil(time.props["textReservation"], "the kernel reserved the countdown's widest value")
        XCTAssertGreaterThanOrEqual(trailing.width, time.lines.map(\.width).max() ?? 0, "the reserved width holds the current value")
        XCTAssertLessThan(trailing.width, 120, "\(trailing)")
        XCTAssertEqual(laid.viewport.height, 22.667, "the Island's height proposal is the viewport's")
        let minimal = render.size(.minimal, proposal: .unspecified)
        XCTAssertGreaterThan(minimal.width, 8); XCTAssertLessThan(minimal.width, 40, "the symbol: \(minimal)")
        // A centred line comes to its own width, not the line it was first laid out in.
        let tinted = try self.render(try Self.plan("SNAPSHOT_TINTED"), kind: "tinted", state: "{\"title\":\"Hi\"}")
        let centred = tinted.size(.compactTrailing, proposal: ProposedViewSize(width: 300, height: nil))
        XCTAssertLessThan(centred.width, 40, "\(centred)")
        // An authored width is taken as it is: the trailing region is as wide as its 120 pt text, not the text's two letters.
        XCTAssertEqual(tinted.size(.expandedTrailing, proposal: ProposedViewSize(width: 165, height: nil)).width, 120)
        XCTAssertEqual(try tinted.layout(.expandedTrailing, width: 120, height: nil).root.frame.width, 120)
        XCTAssertTrue(try tinted.layout(.expandedTrailing, width: 165, height: nil).snapshot.nodes.values.contains { $0.frame.width == 120 && $0.lines.first?.text == "Hi" })
    }

    // MARK: The state paths: the envelope's, the kept last-good one, the preview

    func testTheEnvelopeStateTheKeptStateAndThePlaceholderEachRender() throws {
        let plan = try Self.plan("SNAPSHOT_DEPARTURE")
        let fresh = "{\"at\":1787915700000,\"headsign\":\"San Jose\",\"station\":\"Palo Alto\",\"status\":\"Delayed\",\"train\":233}"
        let render = try self.render(plan, kind: "departure", state: fresh)
        let lock = try render.layout(.lockScreen, width: 362, height: nil)
        XCTAssertTrue(texts(lock).contains("Palo Alto to San Jose"), "\(texts(lock))")
        XCTAssertTrue(texts(try render.layout(.compactLeading, width: 100, height: nil)).contains("233"))
        // Another shape, or a field the shape does not know, is refused at the boot.
        XCTAssertThrowsError(try self.render(plan, kind: "departure", state: "{\"title\":\"Delay\"}"))
        XCTAssertThrowsError(try self.render(plan, kind: "departure", state: "{\"at\":1,\"headsign\":\"x\",\"station\":\"y\",\"status\":\"z\",\"train\":\"one\"}"))
        // The kept state: what the extension shows when the envelope's cannot be decoded (D9).
        let root = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
        defer { try? FileManager.default.removeItem(at: root) }
        let cache = ActivityStateCache(root: root, id: "activity-1")
        let accepts = { (bytes: Data) in (try? ActivityRender(plan: plan, kind: "departure", state: bytes, facts: Self.facts, assets: Self.assets)) != nil }
        XCTAssertFalse(try cache.resolve(Data(fresh.utf8), accepts: accepts).outdated)
        let kept = try cache.resolve(Data("{\"title\":\"Delay\"}".utf8), accepts: accepts)
        XCTAssertTrue(kept.outdated)
        XCTAssertEqual(kept.state, Data(fresh.utf8))
        let outdated = try self.render(plan, kind: "departure", state: kept.state.map { String(decoding: $0, as: UTF8.self) })
        XCTAssertTrue(texts(try outdated.layout(.lockScreen, width: 362, height: nil)).contains("Palo Alto to San Jose"))
        // The placeholder: the preview, redacted by the system; it opens nothing.
        let preview = try self.render(plan, kind: "departure", state: nil)
        let placeholder = try preview.layout(.lockScreen, width: 362, height: nil)
        XCTAssertTrue(texts(placeholder).contains("Mountain View to San Francisco"), "\(texts(placeholder))")
        let view = SnapshotActivity.view(preview, placement: .lockScreen, environment: environment(.lockScreen, placeholder: true)).redacted(reason: .placeholder)
            .environment(\.colorScheme, .light).background(Color.white)
        let image = image(view, proposal: ProposedViewSize(width: 362, height: nil))
        try write(image, "departure-lockScreen-placeholder")
        XCTAssertGreaterThan(try ink(try XCTUnwrap(image), background: [255, 255, 255]), 8, "the redacted placeholder draws its shapes")
        XCTAssertNil(SnapshotActivity.url(placeholder, environment: environment(.lockScreen, placeholder: true)))
    }

    // MARK: D5 and D7: the tint is the Lock Screen root's background; a link is the placement's URL

    func testTheTintIsTheLockScreenBackgroundAndALinkIsThePlacementURL() throws {
        let base = URL(string: "https://tinted.exact.invalid")!
        let tinted = try self.render(try Self.plan("SNAPSHOT_TINTED"), kind: "tinted", state: "{\"title\":\"Northbound delayed\"}")
        let tint = try XCTUnwrap(SnapshotActivity.tint(tinted, dark: false))
        let resolved = tint.resolve(in: EnvironmentValues())
        XCTAssertEqual(Int((resolved.red * 255).rounded()), 0x1e); XCTAssertEqual(Int((resolved.green * 255).rounded()), 0x3a); XCTAssertEqual(Int((resolved.blue * 255).rounded()), 0x5f)
        let lock = try tinted.layout(.lockScreen, width: 362, height: nil)
        XCTAssertEqual(SnapshotActivity.url(lock, environment: environment(.lockScreen, link: base)), URL(string: "https://tinted.exact.invalid/alerts"))
        XCTAssertEqual(SnapshotActivity.url(try tinted.layout(.expandedCenter, width: 109, height: nil), environment: environment(.expandedCenter, link: base))?.path, "/alerts")
        XCTAssertNil(SnapshotActivity.url(try tinted.layout(.compactLeading, width: 60, height: nil), environment: environment(.compactLeading, link: base)))
        XCTAssertTrue(environment(.lockScreen).takesLinks); XCTAssertTrue(environment(.expandedBottom).takesLinks)
        XCTAssertFalse(environment(.compactTrailing).takesLinks); XCTAssertFalse(environment(.minimal).takesLinks)
        // Caltrain's activities leave the background unsaid: the system's material.
        XCTAssertNil(SnapshotActivity.tint(try self.render(try Self.plan("SNAPSHOT_DEPARTURE"), kind: "departure", state: nil), dark: false))
        // The Lock Screen root's own background is the tint, never drawn twice: the white title is the ink on the tint.
        let view = SnapshotActivity.view(tinted, placement: .lockScreen, environment: environment(.lockScreen, link: base))
            .environment(\.colorScheme, .light).background(tint)
        let image = try XCTUnwrap(image(view, proposal: ProposedViewSize(width: 362, height: nil)))
        try write(image, "tinted-lockScreen")
        XCTAssertGreaterThan(try ink(image, background: [0x1e, 0x3a, 0x5f]), 8)
    }

    /// One session serves every placement (D9): laid out again only at a
    /// size it has not seen, and released with its render.
    func testOneSessionServesEveryPlacementAndIsReleased() throws {
        let plan = try Self.plan("SNAPSHOT_SERVICE_ALERT")
        weak var session: HeadlessSession?
        try autoreleasepool {
            let render = try self.render(plan, kind: "service-alert", state: nil)
            session = render.session
            for placement in ActivityPlacement.allCases { _ = render.size(placement, proposal: ProposedViewSize(width: 165, height: nil)) }
            let count = render.layoutCount
            XCTAssertGreaterThan(count, 0); XCTAssertLessThanOrEqual(count, 8 * 6, "a layout per placement, its content-height pass and a content-sized region's settling: \(count)")
            for placement in ActivityPlacement.allCases { _ = render.size(placement, proposal: ProposedViewSize(width: 165, height: nil)) }
            XCTAssertEqual(render.layoutCount, count, "a size seen before costs no layout")
            XCTAssertGreaterThan(render.layoutMilliseconds, 0)
            XCTAssertNotNil(session)
        }
        XCTAssertNil(session, "the session is retained after its render is gone")
    }
}
#endif
