// The navigation, sheet, keyboard and focus rules (LLP 1035.001 slice 1),
// held as assertions about decisions: no window, no run loop, no clock,
// nothing that can flake on timing. `NavigationRules` is what UIKit's
// projection applies; this file is what says the projection's rules are
// the RFC's.
//
// @ref LLP 1035.001 D1–D5, D8; `rules/RULES.md` §Loop shape
#if os(macOS)
import XCTest
@testable import ExactKit

final class NavigationRulesTests: XCTestCase {
    /// D1: the stack is the prefix through the selected route; a key that
    /// matches no route leaves the stack alone.
    func testTheStackIsThePrefixThroughTheSelectedRoute() {
        let keys = ["", "thread", "details:thread", "compose"]
        XCTAssertEqual(NavigationRules.stack(routeKeys: keys, selected: ""), 0..<1)
        XCTAssertEqual(NavigationRules.stack(routeKeys: keys, selected: "details:thread"), 0..<3)
        XCTAssertNil(NavigationRules.stack(routeKeys: keys, selected: "elsewhere"))
    }

    /// D2: completion dispatches once, on a key change only; a cancelled
    /// swipe and a programmatic Back dispatch nothing.
    func testACompletedPopDispatchesExactlyOnAKeyChange() {
        XCTAssertTrue(NavigationRules.dispatchesBack(shownKey: "", rootKey: "thread", modalActive: false))
        // Cancelled: UIKit shows the same route the root still names.
        XCTAssertFalse(NavigationRules.dispatchesBack(shownKey: "thread", rootKey: "thread", modalActive: false))
        // Programmatic: the key already moved when UIKit finished.
        XCTAssertFalse(NavigationRules.dispatchesBack(shownKey: "", rootKey: "", modalActive: false))
        // A sheet's dismissal has its own path.
        XCTAssertFalse(NavigationRules.dispatchesBack(shownKey: "", rootKey: "compose", modalActive: true))
    }

    /// D1: the Back control is resolved by id among enabled, pressable, live
    /// controls, lowest view id first; a disabled one blocks nothing but
    /// resolves to nothing.
    func testTheBackControlIsResolvedAtUseByHTMLId() {
        struct C { let id: UInt32; let html: String?; let press: Bool; let disabled: Bool }
        let controls = [
            C(id: 9, html: "back", press: true, disabled: true),
            C(id: 12, html: "back", press: true, disabled: false),
            C(id: 4, html: "back", press: false, disabled: false),
            C(id: 3, html: "close", press: true, disabled: false),
        ]
        let resolve = { (target: String?) -> UInt32? in
            NavigationRules.backControl(named: target, among: controls, id: \.id, htmlID: \.html, pressable: \.press, disabled: \.disabled)?.id
        }
        XCTAssertEqual(resolve("back"), 12)
        XCTAssertEqual(resolve("close"), 3)
        XCTAssertNil(resolve("missing"))
        XCTAssertNil(resolve(nil))
        // All disabled: nothing may begin.
        let disabled = [C(id: 1, html: "back", press: true, disabled: true)]
        XCTAssertNil(NavigationRules.backControl(named: "back", among: disabled, id: \.id, htmlID: \.html, pressable: \.press, disabled: \.disabled))
    }

    /// D1: an interactive pop needs a stack to pop, no transition, no sheet,
    /// a Back control, and no context preview.
    func testWhenAPopMayBegin() {
        XCTAssertTrue(NavigationRules.popMayBegin(depth: 2, changing: false, modalActive: false, hasBackControl: true, contextPreviewActive: false))
        XCTAssertFalse(NavigationRules.popMayBegin(depth: 1, changing: false, modalActive: false, hasBackControl: true, contextPreviewActive: false))
        XCTAssertFalse(NavigationRules.popMayBegin(depth: 2, changing: true, modalActive: false, hasBackControl: true, contextPreviewActive: false))
        XCTAssertFalse(NavigationRules.popMayBegin(depth: 2, changing: false, modalActive: true, hasBackControl: true, contextPreviewActive: false))
        XCTAssertFalse(NavigationRules.popMayBegin(depth: 2, changing: false, modalActive: false, hasBackControl: false, contextPreviewActive: false))
        XCTAssertFalse(NavigationRules.popMayBegin(depth: 2, changing: false, modalActive: false, hasBackControl: true, contextPreviewActive: true))
    }

    /// D1's arbitration: the edge is navigation's; past it a `swiperight`
    /// node under the start wins; then horizontal beats vertical.
    func testAPanYieldsToASwipeRightNodePastTheEdge() {
        XCTAssertTrue(NavigationRules.panMayBegin(startX: 10, overSwipeRight: true, velocity: CGPoint(x: 300, y: 20)))
        XCTAssertFalse(NavigationRules.panMayBegin(startX: 40, overSwipeRight: true, velocity: CGPoint(x: 300, y: 20)))
        XCTAssertTrue(NavigationRules.panMayBegin(startX: 40, overSwipeRight: false, velocity: CGPoint(x: 300, y: 20)))
        XCTAssertFalse(NavigationRules.panMayBegin(startX: 40, overSwipeRight: false, velocity: CGPoint(x: 20, y: 300)))
    }

    /// D1: `closedby="none"` refuses the sheet gesture; anything else permits it.
    func testClosedByNoneRefusesDismissal() {
        XCTAssertTrue(NavigationRules.modalRefusesDismissal(closedby: "none"))
        XCTAssertFalse(NavigationRules.modalRefusesDismissal(closedby: "closerequest"))
        XCTAssertFalse(NavigationRules.modalRefusesDismissal(closedby: nil))
    }

    /// D4: deferred geometry replays frames before contents, ids ascending.
    func testDeferredGeometryReplaysFramesBeforeContentsInIdOrder() {
        let order = NavigationRules.replayOrder(deferred: [7: ["content", "frame"], 3: ["frame"], 12: ["content"]])
        XCTAssertEqual(order.map { "\($0.id):\($0.kind)" }, ["3:frame", "7:frame", "7:content", "12:content"])
    }

    /// D5: a session answers a keyboard only for its own editor, or while it
    /// still holds an inset it applied.
    func testAKeyboardConcernsOnlyTheSessionThatOwnsIt() {
        XCTAssertTrue(NavigationRules.keyboardConcerns(editing: true, holdsInset: false))
        XCTAssertTrue(NavigationRules.keyboardConcerns(editing: false, holdsInset: true))
        XCTAssertFalse(NavigationRules.keyboardConcerns(editing: false, holdsInset: false))
    }

    /// D5: the viewport freeze is for an initially interactive pop, never a
    /// sheet — the Messages defect of 2026-09-09.
    func testTheViewportFreezeIsForAnInteractivePopOnly() {
        XCTAssertTrue(NavigationRules.freezesViewport(modalActive: false, changing: true, initiallyInteractive: true))
        XCTAssertFalse(NavigationRules.freezesViewport(modalActive: true, changing: true, initiallyInteractive: true))
        XCTAssertFalse(NavigationRules.freezesViewport(modalActive: false, changing: false, initiallyInteractive: true))
        XCTAssertFalse(NavigationRules.freezesViewport(modalActive: false, changing: true, initiallyInteractive: false))
    }

    /// D3: a focus that cannot be delivered has a named reason, in a fixed
    /// order, and a deliverable one has none.
    func testAFocusRefusalNamesItsReason() {
        XCTAssertNil(NavigationRules.focusRefusal(mounted: true, disabled: false, zeroSize: false, hiddenAncestor: false, inertAncestor: false))
        XCTAssertEqual(NavigationRules.focusRefusal(mounted: false, disabled: true, zeroSize: true, hiddenAncestor: true, inertAncestor: true), "not mounted")
        XCTAssertEqual(NavigationRules.focusRefusal(mounted: true, disabled: false, zeroSize: false, hiddenAncestor: false, inertAncestor: true), "inert ancestor")
    }
}
#endif
