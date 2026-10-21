import XCTest
@testable import ExactKit
#if os(macOS)
import AppKit
#else
import UIKit
#endif

/// Regions and parts (LLP 1075.003.000.001 §3.4, §3.5): each bound refuses a
/// registration whole and counts it; a region whose object is not in a window
/// is retired to a tombstone that every read sees alike; a view's interior is
/// its innermost bound ancestor's; a node's end takes what it registered.
final class HatchRegionsTests: XCTestCase {
    func testBoundsOwnershipAndTombstones() throws {
        let session = ExactApp.shared.makeSession(label: "regions")
        defer { session.destroy() }
        let regions = session.presenter.elements.regions
        let view = { PlatformView(frame: CGRect(x: 0, y: 0, width: 12, height: 12)) }
        var keep: [PlatformView] = []

        // A sentence of 1 to 120 bytes; 32 regions a scope; the same object again replaces its region.
        XCTAssertFalse(regions.owns(scope: "element dot", node: 7, kind: 0, object: view(), what: "", surface: false))
        XCTAssertFalse(regions.owns(scope: "element dot", node: 7, kind: 0, object: view(), what: String(repeating: "x", count: 121), surface: false))
        XCTAssertFalse(regions.owns(scope: "element dot", node: 7, kind: 0, object: nil, what: "no view", surface: false))
        for i in 0..<40 { let v = view(); keep.append(v); _ = regions.owns(scope: "element dot", node: 7, kind: 0, object: v, what: "region \(i)", surface: false) }
        XCTAssertEqual(regions.regions.count, 32)
        XCTAssertTrue(regions.owns(scope: "element dot", node: 7, kind: 0, object: keep[0], what: "said again", surface: true))
        XCTAssertEqual(regions.regions.count, 32)
        XCTAssertEqual(regions.rejected, 3 + 8)

        // 16 appearances a session, keyed by their sentence.
        for i in 0..<20 { _ = regions.owns(scope: "app", node: 0, kind: 2, object: nil, what: "tint \(i)", surface: false) }
        _ = regions.owns(scope: "app", node: 0, kind: 2, object: nil, what: "tint 0", surface: false)
        XCTAssertEqual(regions.regions.filter { $0.kind == "appearance" }.count, 16)

        // Parts: a list replaces the last; one that breaks a bound is refused whole.
        let seal = view()
        let part = { (id: String, label: String) in HatchRegions.Part(id: id, role: "button", label: label, view: seal) }
        XCTAssertTrue(regions.setParts(node: 7, scope: "element dot", [part("seal", "Verified")]))
        XCTAssertFalse(regions.setParts(node: 7, scope: "element dot", [part("a", ""), part("a", "")]))
        XCTAssertFalse(regions.setParts(node: 7, scope: "element dot", (0..<33).map { part("p\($0)", "") }))
        XCTAssertFalse(regions.setParts(node: 7, scope: "element dot", [part(String(repeating: "i", count: 65), "")]))
        XCTAssertFalse(regions.setParts(node: 7, scope: "element dot", [part("long", String(repeating: "l", count: 121))]))
        XCTAssertEqual(regions.parts[7]?.map(\.id), ["seal"])
        XCTAssertEqual(regions.part(owning: seal), "7/seal")
        let inside = view()
        seal.addSubview(inside)
        XCTAssertEqual(regions.part(owning: inside), "7/seal", "a part's interior is the part's")

        // A view's interior is its innermost bound ancestor's.
        let outer = keep[1], inner = view(), leaf = view()
        outer.addSubview(inner); inner.addSubview(leaf)
        XCTAssertEqual(regions.owner(of: leaf), "region 1")
        _ = regions.owns(scope: "element other", node: 8, kind: 0, object: inner, what: "the inner one", surface: false)
        XCTAssertEqual(regions.owner(of: leaf), "the inner one")
        XCTAssertNil(regions.owner(of: view()))

        // None of these views is in a window: the host retires each view region
        // to a tombstone, 16 kept, and a second read is the first.
        var first: [String: Any] = [:], second: [String: Any] = [:]
        regions.state(into: &first)
        regions.state(into: &second)
        let owned = try XCTUnwrap(first["owns"] as? [[String: Any]])
        XCTAssertEqual(owned.filter { ($0["observed"] as? [String: Any])?["live"] as? Bool == false }.count, 16)
        XCTAssertEqual(owned.filter { $0["kind"] as? String == "appearance" }.count, 16, "an appearance is bound to nothing and stays")
        XCTAssertEqual(try JSONSerialization.data(withJSONObject: first, options: .sortedKeys), try JSONSerialization.data(withJSONObject: second, options: .sortedKeys))

        // The node's end takes its parts and regions; the app's stay.
        regions.ended(node: 7)
        XCTAssertNil(regions.parts[7])
        XCTAssertNil(regions.part(owning: seal))
        regions.ended(scope: "app")
        XCTAssertTrue(regions.regions.isEmpty)
    }

    /// The window's walk (§3.4): a recognizer that was there before the call
    /// is nobody's news; one the call added is journaled once, by class,
    /// unless a region binds it.
    func testAWindowsNewRecognizerIsNamedOnceUnlessARegionBindsIt() throws {
        let session = ExactApp.shared.makeSession(label: "walk")
        defer { session.destroy() }
        let regions = session.presenter.elements.regions
        let root = PlatformView(frame: CGRect(x: 0, y: 0, width: 100, height: 100)), inner = PlatformView(frame: root.bounds)
        root.addSubview(inner)
        let old = HatchRecognizer(), added = HatchRecognizer(), declared = HatchRecognizer()
        inner.addGestureRecognizer(old)
        let had = regions.recognizers(under: root)
        XCTAssertEqual(had, [ObjectIdentifier(old)])
        inner.addGestureRecognizer(added)
        root.addGestureRecognizer(declared)
        XCTAssertTrue(regions.owns(scope: "window", node: 0, kind: 1, object: declared, what: "a swipe to dismiss", surface: false))
        // A view the hatch declared covers the recognizers inside it.
        let control = PlatformView(frame: root.bounds), within = PlatformView(frame: root.bounds)
        control.addSubview(within)
        within.addGestureRecognizer(HatchRecognizer())
        control.addGestureRecognizer(HatchRecognizer())
        root.addSubview(control)
        XCTAssertTrue(regions.owns(scope: "window", node: 0, kind: 0, object: control, what: "a close button", surface: false))
        // An unstarted session keeps no journal to read: what was said is the check's own record.
        regions.undeclared(under: root, by: "window") { had.contains(ObjectIdentifier($0)) }
        regions.undeclared(under: root, by: "window") { had.contains(ObjectIdentifier($0)) }
        XCTAssertEqual(regions.reported, ["window\n\(String(describing: HatchRecognizer.self))"])
    }
}
