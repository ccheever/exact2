// The extension's resolver, cache and presentation (LLP 1126.001 WP10, D9),
// driven as the extension drives them: generations published in a group
// root, the embedded plans with their identity, the shape-digest gate, the
// kept last-good state and its outdated presentation, the preview as the
// placeholder, and a render keyed by every fact that shapes its layout.
import XCTest
import SwiftUI
import ExactCore
import ExactSnapshot
#if canImport(WidgetKit)
import WidgetKit
#endif

#if canImport(WidgetKit)
@MainActor
@available(iOS 17.0, macOS 14.0, visionOS 26.0, *)
final class ActivityResolverTests: XCTestCase {
    static let compatibility = "cohort-a"
    static let shape = String(repeating: "5", count: 64)
    static let fresh = Data("{\"at\":1787915700000,\"headsign\":\"San Jose\",\"station\":\"Palo Alto\",\"status\":\"Delayed\",\"train\":233}".utf8)
    static let other = Data("{\"at\":1787915700000,\"headsign\":\"San Jose\",\"station\":\"Redwood City\",\"status\":\"On time\",\"train\":118}".utf8)
    static let wrongShape = Data("{\"title\":\"Delay\"}".utf8)

    struct Fixture {
        let root: URL
        let group: URL
        let embedded: URL
        let generations: [String: URL]
        var sources: ActivitySources { ActivitySources(groupRoot: group, embeddedRoot: embedded, embeddedGeneration: "e1", compatibility: ActivityResolverTests.compatibility) }
        var embeddedGeneration: String { WidgetContainer.generationDigest(compatibility: ActivityResolverTests.compatibility, publicationID: "embedded:e1") }
        func cache(_ id: String) -> ActivityStateCache { ActivityStateCache(root: group, id: id) }
    }

    /// Two published generations of Caltrain's departure plan (`current` names the second) and the embedded plans.
    func fixture() throws -> Fixture {
        let plan = try ActivityDriverTests.plan("SNAPSHOT_DEPARTURE")
        let root = FileManager.default.temporaryDirectory.appendingPathComponent("activity-resolver-\(UUID().uuidString)")
        let group = root.appendingPathComponent("group"), embedded = root.appendingPathComponent("embedded")
        let registry = try JSONSerialization.data(withJSONObject: [["kind": "departure", "shape": Self.shape, "preview": ["station": "Mountain View", "headsign": "San Francisco", "train": 101, "at": 1_787_915_700_000, "status": "On time"]]])
        func write(plans dir: URL, generation: String?) throws {
            try FileManager.default.createDirectory(at: dir.appendingPathComponent("plans"), withIntermediateDirectories: true)
            try plan.write(to: dir.appendingPathComponent("plans/departure.plan"))
            try registry.write(to: dir.appendingPathComponent("plans/activities.json"))
            if generation != nil { try Data(Self.compatibility.utf8).write(to: dir.appendingPathComponent("compatibility")) }
        }
        var generations: [String: URL] = [:]
        let base = group.appendingPathComponent("widget-generations")
        for publication in ["pub1", "pub2"] {
            let name = WidgetContainer.generationDigest(compatibility: Self.compatibility, publicationID: publication)
            let dir = base.appendingPathComponent(name)
            try write(plans: dir, generation: name)
            generations[publication] = dir
        }
        try Data(generations["pub2"]!.lastPathComponent.utf8).write(to: base.appendingPathComponent("current"))
        try write(plans: embedded, generation: nil)
        return Fixture(root: root, group: group, embedded: embedded, generations: generations)
    }

    func request(_ fixture: Fixture, generation: String? = nil, id: String = "activity-1", stateShape: String = ActivityResolverTests.shape, state: Data = ActivityResolverTests.fresh) -> ActivityRequest {
        ActivityRequest(activityID: id, kind: "departure", generation: generation ?? fixture.generations["pub1"]!.lastPathComponent, shape: Self.shape, stateShape: stateShape, state: state)
    }
    let facts = ActivityFacts(displayScale: 2, preferences: [], rootFontSize: 17, locale: "en-US", timeZone: TimeZone(identifier: "America/Los_Angeles")!)

    func texts(_ rendering: ActivityRendering) throws -> [String] {
        let laid = try rendering.render.layout(.lockScreen, width: 300, height: nil)
        return laid.snapshot.nodes.values.filter { $0.kind == "text" || $0.kind == "time" }.flatMap(\.lines).map(\.text)
    }

    /// D9: the generation the attributes name, never `current`; the
    /// embedded plans only as the generation their identity makes; else
    /// a refusal, and nothing in its place.
    func testTheNamedGenerationIsPinnedTheEmbeddedOnlyByIdentityElseRefused() throws {
        let f = try fixture()
        defer { try? FileManager.default.removeItem(at: f.root) }
        let pinned = try ActivityResolver.resolve(request(f), facts: facts, sources: f.sources, stateCache: nil)
        XCTAssertEqual(pinned.root.standardizedFileURL, f.generations["pub1"]!.standardizedFileURL, "the named generation, not current")
        XCTAssertFalse(pinned.outdated); XCTAssertFalse(pinned.placeholder)
        XCTAssertTrue(try texts(pinned).contains("Palo Alto to San Jose"))
        let embedded = try ActivityResolver.resolve(request(f, generation: f.embeddedGeneration), facts: facts, sources: f.sources, stateCache: nil)
        XCTAssertEqual(embedded.root.standardizedFileURL, f.embedded.standardizedFileURL)
        let unknown = String(repeating: "c", count: 64)
        XCTAssertThrowsError(try ActivityResolver.resolve(request(f, generation: unknown), facts: facts, sources: f.sources, stateCache: nil)) { error in
            XCTAssertEqual(error as? ActivityResolver.Failure, .generationUnavailable(unknown))
        }
        // Another cohort's generation of the same name is refused too.
        let foreign = ActivitySources(groupRoot: f.group, embeddedRoot: f.embedded, embeddedGeneration: "e1", compatibility: "cohort-b")
        XCTAssertThrowsError(try ActivityResolver.resolve(request(f), facts: facts, sources: foreign, stateCache: nil))
        // The cache keeps no failed resolution, and keeps the named generation's render apart from current's.
        let renders = ActivityRenders(limit: 4)
        XCTAssertThrowsError(try renders.rendering(for: request(f, generation: unknown), facts: facts, sources: f.sources, stateCache: nil))
        XCTAssertEqual(renders.count, 0)
        let a = try renders.rendering(for: request(f), facts: facts, sources: f.sources, stateCache: nil)
        let b = try renders.rendering(for: request(f, generation: f.generations["pub2"]!.lastPathComponent), facts: facts, sources: f.sources, stateCache: nil)
        XCTAssertNotEqual(a.root.standardizedFileURL, b.root.standardizedFileURL)
        XCTAssertEqual(renders.count, 2)
    }

    /// The shape-digest gate and the kept state: an envelope of another
    /// shape, or one the plan cannot decode, shows the last decodable state
    /// of this activity (outdated), else the preview as the placeholder;
    /// a decodable envelope is kept for the next time.
    func testTheShapeGateTheKeptStateAndThePlaceholder() throws {
        let f = try fixture()
        defer { try? FileManager.default.removeItem(at: f.root) }
        let cache = f.cache("activity-1")
        // Nothing kept yet: another shape's state is the placeholder.
        let first = try ActivityResolver.resolve(request(f, stateShape: String(repeating: "6", count: 64), state: Self.wrongShape), facts: facts, sources: f.sources, stateCache: cache)
        XCTAssertTrue(first.placeholder); XCTAssertFalse(first.outdated)
        XCTAssertTrue(try texts(first).contains("Mountain View to San Francisco"))
        // A decodable envelope renders and is kept.
        let good = try ActivityResolver.resolve(request(f), facts: facts, sources: f.sources, stateCache: cache)
        XCTAssertFalse(good.outdated); XCTAssertFalse(good.placeholder)
        XCTAssertTrue(try texts(good).contains("Palo Alto to San Jose"))
        // The right shape digest with fields the plan does not know: the kept state, outdated.
        let undecodable = try ActivityResolver.resolve(request(f, state: Self.wrongShape), facts: facts, sources: f.sources, stateCache: cache)
        XCTAssertTrue(undecodable.outdated); XCTAssertFalse(undecodable.placeholder)
        XCTAssertTrue(try texts(undecodable).contains("Palo Alto to San Jose"))
        // Another shape digest with a decodable state: the gate refuses it, the kept state shows.
        let gated = try ActivityResolver.resolve(request(f, stateShape: String(repeating: "6", count: 64), state: Self.other), facts: facts, sources: f.sources, stateCache: cache)
        XCTAssertTrue(gated.outdated)
        XCTAssertTrue(try texts(gated).contains("Palo Alto to San Jose"))
        XCTAssertFalse(try texts(gated).contains("Redwood City to San Jose"))
        // Another activity keeps its own state.
        let second = try ActivityResolver.resolve(request(f, id: "activity-2", state: Self.wrongShape), facts: facts, sources: f.sources, stateCache: f.cache("activity-2"))
        XCTAssertTrue(second.placeholder)
        // A fresh decodable state replaces the kept one.
        let next = try ActivityResolver.resolve(request(f, state: Self.other), facts: facts, sources: f.sources, stateCache: cache)
        XCTAssertFalse(next.outdated)
        XCTAssertTrue(try texts(next).contains("Redwood City to San Jose"))
        XCTAssertTrue(try texts(try ActivityResolver.resolve(request(f, state: Self.wrongShape), facts: facts, sources: f.sources, stateCache: cache)).contains("Redwood City to San Jose"))
    }

    /// The cache's key is the request and every fact the session reads:
    /// Bold Text alone boots a new session; the same facts share one.
    func testTheRenderIsKeyedByEveryFactThatShapesItsLayout() throws {
        let f = try fixture()
        defer { try? FileManager.default.removeItem(at: f.root) }
        let renders = ActivityRenders(limit: 8)
        let base = try renders.rendering(for: request(f), facts: facts, sources: f.sources, stateCache: nil)
        XCTAssertTrue(try renders.rendering(for: request(f), facts: facts, sources: f.sources, stateCache: nil) === base, "the same request and facts share the render")
        let bold = ActivityFacts(displayScale: 2, preferences: Preferences(reducedMotion: false, reducedTransparency: false, contrast: "no-preference", dark: false, gamut: "srgb", highDynamicRange: false, boldText: true), rootFontSize: 17, locale: "en-US", timeZone: TimeZone(identifier: "America/Los_Angeles")!)
        let bolder = try renders.rendering(for: request(f), facts: bold, sources: f.sources, stateCache: nil)
        XCTAssertFalse(bolder === base, "Bold Text alone is a new session")
        XCTAssertFalse(bolder.render.session === base.render.session)
        for other in [ActivityFacts(displayScale: 3, preferences: [], rootFontSize: 17), ActivityFacts(displayScale: 2, preferences: [.contrastMore], rootFontSize: 17),
                      ActivityFacts(displayScale: 2, preferences: [.reducedTransparency], rootFontSize: 17), ActivityFacts(displayScale: 2, preferences: [.dark], rootFontSize: 17),
                      ActivityFacts(displayScale: 2, preferences: [], rootFontSize: 23), ActivityFacts(displayScale: 2, preferences: [], rootFontSize: 17, locale: "fr-FR"),
                      ActivityFacts(displayScale: 2, preferences: [], rootFontSize: 17, timeZone: TimeZone(identifier: "Europe/Paris")!)] {
            XCTAssertFalse(try renders.rendering(for: request(f), facts: other, sources: f.sources, stateCache: nil) === base, "\(other)")
        }
        // The full state bytes key the render: two states of one digest are two renders.
        XCTAssertFalse(try renders.rendering(for: request(f, state: Self.other), facts: facts, sources: f.sources, stateCache: nil) === base)
        XCTAssertLessThanOrEqual(renders.count, 8)
    }

    /// The presentation: the placeholder redacted, the kept state dimmed
    /// once, a fresh state as it is.
    func testThePresentationRedactsThePlaceholderAndDimsTheKeptStateOnce() throws {
        let f = try fixture()
        defer { try? FileManager.default.removeItem(at: f.root) }
        let cache = f.cache("activity-1")
        let fresh = try ActivityResolver.resolve(request(f), facts: facts, sources: f.sources, stateCache: cache)
        let kept = try ActivityResolver.resolve(request(f, state: Self.wrongShape), facts: facts, sources: f.sources, stateCache: cache)
        let placeholder = try ActivityResolver.resolve(request(f, id: "activity-9", state: Self.wrongShape), facts: facts, sources: f.sources, stateCache: nil)
        let environment = SnapshotEnvironment(family: .liveActivity, placement: .lockScreen, contentMargins: EdgeInsets(), rootFontSize: 17)
        func render(_ rendering: ActivityRendering) throws -> CGImage {
            let renderer = ImageRenderer(content: ActivityPresentation(rendering: rendering, placement: .lockScreen, environment: SnapshotEnvironment(family: .liveActivity, placement: .lockScreen, placeholder: rendering.placeholder, contentMargins: EdgeInsets(), rootFontSize: 17)).background(Color.white))
            renderer.scale = 1; renderer.proposedSize = ProposedViewSize(width: 300, height: nil)
            return try XCTUnwrap(renderer.cgImage)
        }
        func darkest(_ image: CGImage) throws -> Int {
            let w = image.width, h = image.height
            var data = [UInt8](repeating: 0, count: w * h * 4)
            let context = try XCTUnwrap(CGContext(data: &data, width: w, height: h, bitsPerComponent: 8, bytesPerRow: w * 4, space: CGColorSpace(name: CGColorSpace.sRGB)!, bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue))
            context.draw(image, in: CGRect(x: 0, y: 0, width: w, height: h))
            return stride(from: 0, to: data.count, by: 4).map { Int(data[$0]) + Int(data[$0 + 1]) + Int(data[$0 + 2]) }.min() ?? 765
        }
        let freshInk = try darkest(try render(fresh)), keptInk = try darkest(try render(kept))
        XCTAssertLessThan(freshInk, 120, "black text on white")
        XCTAssertGreaterThan(keptInk, freshInk + 150, "the kept state is dimmed")
        XCTAssertLessThan(keptInk, 600, "dimmed once, still legible")
        _ = environment
        XCTAssertTrue(placeholder.placeholder)
        XCTAssertNotEqual(try darkest(try render(placeholder)), freshInk, "the placeholder is redacted")
    }
}
#endif
