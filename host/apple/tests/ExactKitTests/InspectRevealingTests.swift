#if EXACT_INSPECT
import XCTest
@testable import ExactKit

/// The copy an outline brings into view, one rule for iOS and the Mac (Inspect.revealing; Apple pass #2): a session
/// holding the selection brings its own copy, or nothing when that copy shows nowhere, never another copy; a session
/// holding none there (the tool's Outline), the first copy that shows.
final class InspectRevealingTests: XCTestCase {
    override func tearDown() { Inspect.set(false, seq: nil); super.tearDown() }

    func testTheSelectionsOwnCopyElseNothingAndTheFirstShownWithoutOne() {
        let a = NSObject(), b = NSObject()
        let one = ObjectIdentifier(a), two = ObjectIdentifier(b)
        let copies: [(session: ObjectIdentifier, id: UInt32, view: String, shown: Bool)] = [
            (one, 3, "a3", false), (one, 5, "a5", true), (one, 9, "a9", true), (two, 9, "b9", true),
        ]
        let running = [one: 4, two: 1]
        Inspect.set(true, seq: 1)
        // No selection: the first copy that shows.
        XCTAssertEqual(Inspect.revealing(copies, sites: [7], plan: "p", sessions: running), "a5")
        // The selection (session one, generation 4, node 9, site 7): its own copy, not the first, nor another session's node 9.
        Inspect.selected = (one, 4, 9, nil, 7, "p", "[]")
        XCTAssertEqual(Inspect.revealing(copies, sites: [7], plan: "p", sessions: running), "a9")
        // Its copy shows nowhere: nothing, never another copy.
        Inspect.selected = (one, 4, 3, nil, 7, "p", "[]")
        XCTAssertNil(Inspect.revealing(copies, sites: [7], plan: "p", sessions: running))
        // Its copy is gone (a recycled row): nothing.
        Inspect.selected = (one, 4, 11, nil, 7, "p", "[]")
        XCTAssertNil(Inspect.revealing(copies, sites: [7], plan: "p", sessions: running))
        // Not the selection's outline (another site, another plan, an older generation, a session not on this plan): the first that shows.
        Inspect.selected = (one, 4, 9, nil, 8, "p", "[]")
        XCTAssertEqual(Inspect.revealing(copies, sites: [7], plan: "p", sessions: running), "a5")
        Inspect.selected = (one, 4, 9, nil, 7, "q", "[]")
        XCTAssertEqual(Inspect.revealing(copies, sites: [7], plan: "p", sessions: running), "a5")
        Inspect.selected = (one, 3, 9, nil, 7, "p", "[]")
        XCTAssertEqual(Inspect.revealing(copies, sites: [7], plan: "p", sessions: running), "a5")
        Inspect.selected = (one, 4, 9, nil, 7, "p", "[]")
        XCTAssertEqual(Inspect.revealing(copies, sites: [7], plan: "p", sessions: [two: 1]), "a5")
    }
}
#endif
