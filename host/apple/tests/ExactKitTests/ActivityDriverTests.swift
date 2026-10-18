// A Live Activity's placements through the snapshot driver (LLP 1126.001
// WP10): Caltrain's two activities and three fixtures (a tinted root with
// links, a gradient root, the sizes a region takes), each placement laid
// out at the widths the system proposes to a region (finite, none, zero,
// infinite), rendered in-process with `ImageRenderer` as the widget tests
// are. The 1x PNGs go to `EXACT_WP10_OUT` for the eye. The resolver, its
// cache and the presentation are `ActivityResolverTests`'.
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
    static func plan(_ key: String) throws -> Data {
        let path = try XCTUnwrap(ProcessInfo.processInfo.environment["EXACT_\(key)_PLAN"], "EXACT_\(key)_PLAN: build.mjs --test compiles it")
        return try Data(contentsOf: URL(fileURLWithPath: path))
    }
    static let out: URL = {
        let dir = ProcessInfo.processInfo.environment["EXACT_WP10_OUT"].map { URL(fileURLWithPath: $0) }
            ?? FileManager.default.temporaryDirectory.appending(path: "exact-wp10-\(ProcessInfo.processInfo.processIdentifier)")
        try? FileManager.default.createDirectory(at: dir, withIntermediateDirectories: true)
        return dir
    }()
    static let facts = SessionFacts(displayScale: 1, preferences: [], rootFontSize: 17)
    static let assets = AssetResolver(root: FileManager.default.temporaryDirectory)
    static let margin = ActivityPlacementView.platformMargins.leading

    /// The widths stage 0 saw the system propose (one phone's evidence): the
    /// expanded leading and trailing regions, the centre, the bottom and the
    /// Lock Screen; the tests take them as a set of region widths, not as facts.
    static let widths: [CGFloat] = [165, 109.33, 338, 362]

    func render(_ plan: Data, kind: String, state: String?) throws -> ActivityRender {
        try ActivityRender(plan: plan, kind: kind, state: state.map { Data($0.utf8) }, facts: Self.facts, assets: Self.assets)
    }

    /// No content margins: the placement's view is its kernel size, so the
    /// sizes compare; the Lock Screen's margins have their own assertion.
    func environment(_ placement: ActivityPlacement, placeholder: Bool = false, link: URL? = nil, margins: EdgeInsets? = EdgeInsets()) -> SnapshotEnvironment {
        SnapshotEnvironment(family: .liveActivity, placement: placement, placeholder: placeholder, linkBase: link, contentMargins: margins, rootFontSize: 17)
    }

    func image<V: View>(_ view: V, proposal: ProposedViewSize) -> CGImage? {
        let renderer = ImageRenderer(content: view)
        renderer.scale = 1
        renderer.proposedSize = proposal
        return renderer.cgImage
    }

    @discardableResult
    func write(_ image: CGImage?, _ name: String) throws -> URL {
        let url = Self.out.appending(path: "\(name).png")
        let image = try XCTUnwrap(image, name)
        let destination = try XCTUnwrap(CGImageDestinationCreateWithURL(url as CFURL, UTType.png.identifier as CFString, 1, nil))
        CGImageDestinationAddImage(destination, image, nil)
        XCTAssertTrue(CGImageDestinationFinalize(destination), name)
        return url
    }

    /// RGBA8 pixels of a 1x image.
    func pixels(_ image: CGImage) throws -> (width: Int, height: Int, data: [UInt8]) {
        let w = image.width, h = image.height
        var data = [UInt8](repeating: 0, count: w * h * 4)
        let context = try XCTUnwrap(CGContext(data: &data, width: w, height: h, bitsPerComponent: 8, bytesPerRow: w * 4,
                                              space: CGColorSpace(name: CGColorSpace.sRGB)!, bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue))
        context.draw(image, in: CGRect(x: 0, y: 0, width: w, height: h))
        return (w, h, data)
    }

    /// How many pixels in `rect` (the whole image when nil) differ from
    /// `background` by more than 32 in some channel: the ink the leaves drew.
    func ink(_ image: CGImage, in rect: CGRect? = nil, background: [UInt8]) throws -> Int {
        let p = try pixels(image)
        let rect = rect ?? CGRect(x: 0, y: 0, width: p.width, height: p.height)
        var count = 0
        for y in max(0, Int(rect.minY))..<min(p.height, Int(rect.maxY.rounded(.up))) {
            for x in max(0, Int(rect.minX))..<min(p.width, Int(rect.maxX.rounded(.up))) {
                let at = (y * p.width + x) * 4
                if (0..<3).contains(where: { abs(Int(p.data[at + $0]) - Int(background[$0])) > 32 }) { count += 1 }
            }
        }
        return count
    }

    func color(_ image: CGImage, x: Int, y: Int) throws -> [Int] {
        let p = try pixels(image)
        let at = (y * p.width + x) * 4
        return [Int(p.data[at]), Int(p.data[at + 1]), Int(p.data[at + 2])]
    }

    func texts(_ laid: ActivityRender.Laid) -> [String] {
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
                    XCTAssertEqual(laid.size, size, "\(kind) \(placement.name): the view's size is the placement's margin box")
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
            // D5: the Lock Screen placement sits inside the content margins, the HIG's 14 pt when the system gives none.
            XCTAssertEqual(Self.margin, 14)
            let padded = image(SnapshotActivity.view(render, placement: .lockScreen, environment: environment(.lockScreen, margins: nil)), proposal: ProposedViewSize(width: 362, height: nil))
            let inner = try render.layout(.lockScreen, width: 362 - 2 * Self.margin, height: nil)
            XCTAssertEqual(padded?.width, 362); XCTAssertEqual(padded?.height, Int((inner.size.height + 2 * Self.margin).rounded(.up)), kind)
            // The Island's regions take no margins, and a region's fixed height is the viewport's while the view stays its content's height.
            let compact = try render.layout(.compactLeading, width: render.size(.compactLeading, proposal: ProposedViewSize(width: 100, height: 22)).width, height: 22)
            XCTAssertEqual(compact.viewport.height, 22)
            XCTAssertEqual(image(SnapshotActivity.view(render, placement: .compactLeading, environment: environment(.compactLeading, margins: nil)), proposal: ProposedViewSize(width: 100, height: 22))?.height, Int(compact.size.height.rounded(.up)), kind)
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
    }

    /// The sizes a region takes, measured by the kernel's shrink-to-fit
    /// trial, honouring CSS: an authored width at or past the proposal is
    /// kept, a percentage resolves against the proposal, justified content
    /// is its own width, a paragraph that wraps is as wide as its widest
    /// line, a clamped line fills the proposal, a content the row cannot
    /// hold takes the row and no more, and a root's margins are the region's.
    func testRegionSizesHonourTheAuthorsConstraints() throws {
        let sized = try self.render(try Self.plan("SNAPSHOT_SIZED"), kind: "sized", state: nil)
        // compactLeading: `view width=200` holding "Hi", proposed 165 or 200: kept at 200, the author's.
        XCTAssertEqual(sized.size(.compactLeading, proposal: ProposedViewSize(width: 165, height: nil)).width, 200)
        XCTAssertEqual(sized.size(.compactLeading, proposal: ProposedViewSize(width: 200, height: nil)).width, 200)
        XCTAssertEqual(try sized.layout(.compactLeading, width: 200, height: nil).root.frame.width, 200)
        // compactTrailing: an automatic root holding a text `width=165`: the text's width is the root's
        // max-content, so the region is 165 at a 165 proposal, not the text's two letters; at a 120
        // proposal the root takes the row and the text overflows it, as CSS lays it out.
        XCTAssertEqual(sized.size(.compactTrailing, proposal: ProposedViewSize(width: 165, height: nil)).width, 165)
        XCTAssertEqual(sized.size(.compactTrailing, proposal: ProposedViewSize(width: 120, height: nil)).width, 120)
        XCTAssertEqual(try sized.layout(.compactTrailing, width: 120, height: nil).root.children.compactMap { try sized.layout(.compactTrailing, width: 120, height: nil).snapshot.nodes[$0]?.frame.width }.first, 165)
        // minimal: the root `width="50%"`: half the proposal.
        XCTAssertEqual(sized.size(.minimal, proposal: ProposedViewSize(width: 200, height: nil)).width, 100)
        // expanded.leading: a row justified to its end with "Hi": the text's width, not the row's proposal.
        let justified = sized.size(.expandedLeading, proposal: ProposedViewSize(width: 165, height: nil))
        XCTAssertLessThan(justified.width, 40, "\(justified)")
        XCTAssertGreaterThan(justified.width, 8)
        // expanded.trailing: a paragraph that wraps to the proposal: its widest line, narrower than the proposal.
        let wrapped = sized.size(.expandedTrailing, proposal: ProposedViewSize(width: 165, height: nil))
        let laid = try sized.layout(.expandedTrailing, width: wrapped.width, height: nil)
        XCTAssertGreaterThan(texts(laid).count, 1, "wrapped: \(texts(laid))")
        XCTAssertLessThan(wrapped.width, 165, "\(wrapped)")
        XCTAssertEqual(wrapped.width, ActivityRender.widestLine(under: laid.root, in: laid.snapshot).rounded(.up))
        XCTAssertGreaterThan(wrapped.height, 30, "two lines or more")
        // expanded.center: the same paragraph clamped to one line fills the proposal (a truncated line, as a Text's).
        XCTAssertEqual(sized.size(.expandedCenter, proposal: ProposedViewSize(width: 165, height: nil)).width, 165)
        // A `time` whose reserved value is wider than the region takes the region, never more (the Island keeps its neighbours).
        let alert = try self.render(try Self.plan("SNAPSHOT_SERVICE_ALERT"), kind: "service-alert", state: nil)
        XCTAssertGreaterThan(alert.size(.expandedTrailing, proposal: ProposedViewSize(width: 400, height: nil)).width, 109.33, "the relative time's reservation")
        XCTAssertEqual(alert.size(.expandedTrailing, proposal: ProposedViewSize(width: 109.33, height: nil)).width, 109.33)
        // lockScreen: `margin=12` on the root: the margins are the region's, the root at its own offset.
        let lock = try sized.layout(.lockScreen, width: 362, height: nil)
        XCTAssertEqual(lock.margins.left, 12); XCTAssertEqual(lock.margins.top, 12); XCTAssertEqual(lock.margins.right, 12); XCTAssertEqual(lock.margins.bottom, 12)
        XCTAssertEqual(lock.root.frame.minX, 12)
        XCTAssertEqual(lock.root.frame.width, 362 - 24)
        XCTAssertEqual(lock.size, CGSize(width: 362, height: lock.root.frame.height + 24))
        XCTAssertEqual(sized.size(.lockScreen, proposal: ProposedViewSize(width: 362, height: nil)), lock.size)
        let picture = try XCTUnwrap(image(SnapshotActivity.view(sized, placement: .lockScreen, environment: environment(.lockScreen)).background(Color.white), proposal: ProposedViewSize(width: 362, height: nil)))
        try write(picture, "sized-lockScreen")
        XCTAssertEqual(picture.height, Int(lock.size.height.rounded(.up)))
        XCTAssertEqual(try ink(picture, in: CGRect(x: 0, y: 0, width: 362, height: 11), background: [255, 255, 255]), 0, "nothing above the top margin")
        XCTAssertEqual(try ink(picture, in: CGRect(x: 0, y: 0, width: 11, height: picture.height), background: [255, 255, 255]), 0, "nothing left of the left margin")
        XCTAssertGreaterThan(try ink(picture, background: [255, 255, 255]), 8)
    }

    // MARK: D8: a Live Activity's content is not account content unless a node says so

    func testActivityContentStaysUnderPrivacyRedactionUnlessANodeIsPrivate() throws {
        let departure = try self.render(try Self.plan("SNAPSHOT_DEPARTURE"), kind: "departure", state: nil)
        let open = try XCTUnwrap(image(SnapshotActivity.view(departure, placement: .lockScreen, environment: environment(.lockScreen)).background(Color.white), proposal: ProposedViewSize(width: 362, height: nil)))
        let locked = try XCTUnwrap(image(SnapshotActivity.view(departure, placement: .lockScreen, environment: environment(.lockScreen)).background(Color.white).environment(\.redactionReasons, .privacy), proposal: ProposedViewSize(width: 362, height: nil)))
        XCTAssertEqual(try pixels(open).data, try pixels(locked).data, "Caltrain's departure shows the same on a locked phone")
        // The sized fixture's second line says `data-privacy="private"`: redacted under privacy, the title not.
        let sized = try self.render(try Self.plan("SNAPSHOT_SIZED"), kind: "sized", state: nil)
        let laid = try sized.layout(.lockScreen, width: 362, height: nil)
        let lines = laid.root.children.compactMap { laid.snapshot.nodes[$0] }.filter { $0.kind == "text" }.sorted { $0.frame.minY < $1.frame.minY }
        XCTAssertEqual(lines.count, 2)
        let title = lines[0].frame.offsetBy(dx: laid.root.frame.minX, dy: laid.margins.top), long = lines[1].frame.offsetBy(dx: laid.root.frame.minX, dy: laid.margins.top)
        let shown = try XCTUnwrap(image(SnapshotActivity.view(sized, placement: .lockScreen, environment: environment(.lockScreen)).background(Color.white), proposal: ProposedViewSize(width: 362, height: nil)))
        let redacted = try XCTUnwrap(image(SnapshotActivity.view(sized, placement: .lockScreen, environment: environment(.lockScreen)).background(Color.white).environment(\.redactionReasons, .privacy), proposal: ProposedViewSize(width: 362, height: nil)))
        try write(redacted, "sized-lockScreen-privacy")
        func region(_ image: CGImage, _ rect: CGRect) throws -> [UInt8] {
            let p = try pixels(image)
            var out: [UInt8] = []
            for y in Int(rect.minY)..<Int(rect.maxY) { for x in Int(rect.minX)..<Int(rect.maxX) { out.append(contentsOf: p.data[((y * p.width + x) * 4)..<((y * p.width + x) * 4 + 3)]) } }
            return out
        }
        XCTAssertEqual(try region(shown, title), try region(redacted, title), "the title is not account content")
        XCTAssertNotEqual(try region(shown, long), try region(redacted, long), "the private line is redacted")
    }

    // MARK: D5 and D7: the tint, the gradient and the links of a Lock Screen placement

    func testTheTintIsTheLockScreenColourAndALinkIsThePlacementURL() throws {
        let base = URL(string: "https://tinted.exact.invalid")!
        let tinted = try self.render(try Self.plan("SNAPSHOT_TINTED"), kind: "tinted", state: "{\"title\":\"Northbound delayed\"}")
        let lock = try tinted.layout(.lockScreen, width: 362, height: nil)
        let background = SnapshotActivity.background(of: lock, dark: false)
        let tint = try XCTUnwrap(background.tint)
        XCTAssertFalse(background.hasGradients)
        let resolved = tint.resolve(in: EnvironmentValues())
        XCTAssertEqual(Int((resolved.red * 255).rounded()), 0x1e); XCTAssertEqual(Int((resolved.green * 255).rounded()), 0x3a); XCTAssertEqual(Int((resolved.blue * 255).rounded()), 0x5f)
        // The links, by content: the Lock Screen's and an expanded region's resolve against the origin, a compact region's is none.
        XCTAssertEqual(SnapshotActivity.url(lock, environment: environment(.lockScreen, link: base)), URL(string: "https://tinted.exact.invalid/alerts"))
        XCTAssertTrue(texts(lock).contains("Northbound delayed"), "\(texts(lock))")
        XCTAssertEqual(SnapshotActivity.url(try tinted.layout(.expandedCenter, width: 109, height: nil), environment: environment(.expandedCenter, link: base))?.path, "/alerts")
        XCTAssertNil(SnapshotActivity.url(try tinted.layout(.compactLeading, width: 60, height: nil), environment: environment(.compactLeading, link: base)))
        XCTAssertTrue(environment(.lockScreen).takesLinks); XCTAssertTrue(environment(.expandedBottom).takesLinks)
        XCTAssertFalse(environment(.compactTrailing).takesLinks); XCTAssertFalse(environment(.minimal).takesLinks)
        XCTAssertNil(SnapshotActivity.url(try tinted.layout(.lockScreen, width: 362, height: nil), environment: environment(.lockScreen, placeholder: true, link: base)), "the placeholder opens nothing")
        // Caltrain's activities leave the background unsaid: the system's material.
        let departure = try self.render(try Self.plan("SNAPSHOT_DEPARTURE"), kind: "departure", state: nil)
        let unsaid = SnapshotActivity.background(of: try departure.layout(.lockScreen, width: 362, height: nil), dark: false)
        XCTAssertNil(unsaid.tint); XCTAssertFalse(unsaid.hasGradients)
    }

    /// A root with a colour and a gradient: the colour is the tint, the
    /// gradient paints full bleed behind the margins (as a widget's
    /// container background does), the text over it.
    func testAGradientRootPaintsFullBleedBehindTheMargins() throws {
        let graded = try self.render(try Self.plan("SNAPSHOT_GRADED"), kind: "graded", state: "{\"title\":\"Over the gradient\"}")
        let lock = try graded.layout(.lockScreen, width: 300 - 2 * Self.margin, height: nil)
        let background = SnapshotActivity.background(of: lock, dark: false)
        XCTAssertNotNil(background.tint); XCTAssertTrue(background.hasGradients)
        let picture = try XCTUnwrap(image(SnapshotActivity.view(graded, placement: .lockScreen, environment: environment(.lockScreen, margins: nil)).background(Color.white), proposal: ProposedViewSize(width: 300, height: nil)))
        try write(picture, "graded-lockScreen")
        XCTAssertEqual(picture.width, 300)
        // Inside the margins, where no text is: red at the left edge, blue at the right, the gradient full bleed.
        let y = picture.height - 3
        let left = try color(picture, x: 2, y: y), right = try color(picture, x: 297, y: y)
        XCTAssertGreaterThan(left[0], left[2] + 100, "red at the left: \(left)")
        XCTAssertGreaterThan(right[2], right[0] + 100, "blue at the right: \(right)")
        XCTAssertTrue(texts(lock).contains("Over the gradient"))
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
            XCTAssertGreaterThan(count, 0)
            for placement in ActivityPlacement.allCases { _ = render.size(placement, proposal: ProposedViewSize(width: 165, height: nil)) }
            XCTAssertEqual(render.layoutCount, count, "a size seen before costs no layout")
            XCTAssertGreaterThan(render.layoutMilliseconds, 0)
            XCTAssertNotNil(session)
        }
        XCTAssertNil(session, "the session is retained after its render is gone")
    }
}
#endif
