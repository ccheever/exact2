#if os(iOS)
import UIKit
import XCTest
@testable import ExactKit

/// The lifted row's lifetime, on a bare presenter: batches while it is in the
/// preview, its source invalidated, an inert popover refusing the commit.
final class ContextMenuLifetimeIOSTests: XCTestCase {
    private var window: UIWindow!

    private final class Commit: NSObject, UIContextMenuInteractionCommitAnimating {
        var preferredCommitStyle: UIContextMenuInteractionCommitStyle = .pop
        var previewViewController: UIViewController? { nil }
        func addAnimations(_ animations: @escaping () -> Void) { animations() }
        func addCompletion(_ completion: @escaping () -> Void) { completion() }
    }
    /// A source (1) naming popover 2, whose rows are a preview (3) and an item (4).
    private func presenter() -> Presenter {
        if ExactEnv.agentMode { return Presenter() }
        let p = Presenter()
        window = UIWindow(frame: CGRect(x: 0, y: 0, width: 400, height: 800))
        p.viewport.frame = window.bounds
        window.addSubview(p.viewport)
        window.makeKeyAndVisible()
        p.apply(wireBatch([
            ["op": "create", "id": 1, "kind": "button", "handlers": ["press", "focus", "blur"], "props": ["contextPopover": "m"]],
            ["op": "create", "id": 2, "kind": "view", "props": ["popover": "auto", "id": "m"]],
            ["op": "create", "id": 3, "kind": "button", "handlers": ["press"], "props": ["contextPreview": "true"]],
            ["op": "create", "id": 4, "kind": "button", "handlers": ["press"], "props": ["accessibilityLabel": "Pin"]],
            ["op": "create", "id": 9, "kind": "view"],
            ["op": "children", "id": 2, "ids": [3, 4]], ["op": "children", "id": 9, "ids": [1, 2]], ["op": "roots", "ids": [9]],
            ["op": "frame", "id": 9, "x": 0.0, "y": 0.0, "w": 400.0, "h": 800.0],
            ["op": "frame", "id": 1, "x": 0.0, "y": 100.0, "w": 400.0, "h": 60.0],
            ["op": "frame", "id": 2, "x": 0.0, "y": 0.0, "w": 316.0, "h": 260.0],
            ["op": "frame", "id": 3, "x": 8.0, "y": 8.0, "w": 300.0, "h": 180.0],
            ["op": "frame", "id": 4, "x": 8.0, "y": 192.0, "w": 300.0, "h": 44.0],
        ]))
        return p
    }
    private func opened(_ p: Presenter) throws -> (UIContextMenuInteraction, UIContextMenuConfiguration, UIViewController) {
        let source = try XCTUnwrap(p.views[1])
        let interaction = try XCTUnwrap(source.interactions.compactMap { $0 as? UIContextMenuInteraction }.first)
        let configuration = try XCTUnwrap(p.menus.context.contextMenuInteraction(interaction, configurationForMenuAtLocation: .zero))
        let controller = try XCTUnwrap(p.menus.context.lift(try XCTUnwrap(p.menus.context.open)))
        XCTAssertTrue(p.menus.context.lift(try XCTUnwrap(p.menus.context.open)) === controller, "asked again, the same controller")
        return (interaction, configuration, controller)
    }

    func testABatchWhileLiftedMovesTheRowsHomeAndItsSize() throws {
        if ExactEnv.agentMode { throw XCTSkip("under the agent the popover is painted (D4)") }
        let p = presenter()
        let (interaction, configuration, controller) = try opened(p)
        let row = try XCTUnwrap(p.views[3]), pop = try XCTUnwrap(p.views[2])
        // A frame op at the same origin, a new height; a children op on the popover.
        p.apply(wireBatch([["op": "frame", "id": 3, "x": 8.0, "y": 8.0, "w": 300.0, "h": 240.0], ["op": "children", "id": 2, "ids": [3, 4]]]))
        XCTAssertTrue(row.superview === controller.view, "a children op leaves it in the preview")
        XCTAssertEqual(row.frame, CGRect(x: 0, y: 0, width: 300, height: 240))
        XCTAssertEqual(controller.preferredContentSize, CGSize(width: 300, height: 240))
        // At the origin with the same size: the rect the preview already
        // shows it at, and still its new home.
        p.apply(wireBatch([["op": "frame", "id": 3, "x": 0.0, "y": 0.0, "w": 300.0, "h": 240.0]]))
        p.menus.context.contextMenuInteraction(interaction, willEndFor: configuration, animator: nil)
        XCTAssertTrue(row.superview === pop.container)
        XCTAssertEqual(row.frame, CGRect(x: 0, y: 0, width: 300, height: 240), "home is the last frame the kernel gave it")
    }

    func testASourceThatStopsNamingThePopoverEndsTheMenuAndReturnsTheRow() throws {
        if ExactEnv.agentMode { throw XCTSkip("under the agent the popover is painted (D4)") }
        let p = presenter()
        _ = try opened(p)
        let row = try XCTUnwrap(p.views[3]), pop = try XCTUnwrap(p.views[2])
        p.apply(wireBatch([["op": "props", "id": 1, "clear": ["contextPopover"]]]))
        XCTAssertNil(p.menus.context.open)
        XCTAssertTrue(row.superview === pop.container, "back without waiting for willEnd")
        XCTAssertTrue(try XCTUnwrap(p.views[1]).interactions.compactMap { $0 as? UIContextMenuInteraction }.isEmpty)
    }

    func testAConfigurationThatNeverShowsIsNoMenuAndADroppedRowStaysOut() throws {
        if ExactEnv.agentMode { throw XCTSkip("under the agent the popover is painted (D4)") }
        let p = presenter()
        let source = try XCTUnwrap(p.views[1])
        let interaction = try XCTUnwrap(source.interactions.compactMap { $0 as? UIContextMenuInteraction }.first)
        _ = try XCTUnwrap(p.menus.context.contextMenuInteraction(interaction, configurationForMenuAtLocation: .zero))
        XCTAssertNil(p.menus.observation(), "asked for, never shown (a tap): no menu is up")
        let (_, configuration, controller) = try opened(p)
        XCTAssertEqual(p.menus.observation()?["kind"] as? String, "contextmenu")
        // Its popover's children op drops the row, then another parent takes it.
        let row = try XCTUnwrap(p.views[3]), other = try XCTUnwrap(p.views[9])
        p.apply(wireBatch([["op": "children", "id": 2, "ids": [4]], ["op": "children", "id": 9, "ids": [1, 2, 3]]]))
        XCTAssertTrue(row.superview === controller.view, "still the preview's while the menu shows")
        p.menus.context.contextMenuInteraction(interaction, willEndFor: configuration, animator: nil)
        XCTAssertTrue(row.superview === other.container, "then where the kernel put it")
    }

    /// UIKit's type-to-select takes a focus that is not text and, with no
    /// hardware keyboard, raises the software keyboard over the menu: the
    /// focus is set aside while the menu shows, dispatching neither `blur`
    /// nor `focus` (MenuFocusIOS).
    func testTheFocusIsSetAsideQuietlyWhileTheMenuShows() throws {
        if ExactEnv.agentMode { throw XCTSkip("under the agent the popover is painted (D4)") }
        let p = presenter()
        let source = try XCTUnwrap(p.views[1])
        XCTAssertTrue(source.becomeFirstResponder())
        var events: [String] = []
        p.onFocus = { _ in events.append("focus") }
        p.onBlur = { _ in events.append("blur") }
        let (interaction, configuration, _) = try opened(p)
        p.menus.context.contextMenuInteraction(interaction, willDisplayMenuFor: configuration, animator: nil)
        XCTAssertFalse(source.isFirstResponder, "set aside as the menu shows")
        p.menus.context.contextMenuInteraction(interaction, willEndFor: configuration, animator: nil)
        XCTAssertTrue(source.isFirstResponder, "and back when it has ended")
        XCTAssertEqual(events, [], "neither blur nor focus reaches the app")
    }

    func testAPressThatClearsItsSourcesPopoverEndsTheMenuAndItsInteraction() throws {
        if ExactEnv.agentMode { throw XCTSkip("under the agent the popover is painted (D4)") }
        let p = presenter()
        let (interaction, configuration, _) = try opened(p)
        let row = try XCTUnwrap(p.views[3]), pop = try XCTUnwrap(p.views[2]), source = try XCTUnwrap(p.views[1])
        p.onPress = { [weak p] _ in p?.apply(wireBatch([["op": "props", "id": 1, "clear": ["contextPopover"]]])) }
        let commit = Commit()
        p.menus.context.contextMenuInteraction(interaction, willPerformPreviewActionForMenuWith: configuration, animator: commit)
        XCTAssertEqual(commit.preferredCommitStyle, .dismiss)
        XCTAssertNil(p.menus.context.open)
        XCTAssertTrue(row.superview === pop.container, "the row back in its popover")
        XCTAssertTrue(source.interactions.compactMap { $0 as? UIContextMenuInteraction }.isEmpty, "and the interaction gone with the popover's name")
    }

    /// A touch on a bar leaves a focused node blurred, as a touch on a page's
    /// chrome leaves an element on the web (`BarTouch`).
    func testABarsTouchBlursAFocusedNode() throws {
        if ExactEnv.agentMode { throw XCTSkip("under the agent the popover is painted (D4)") }
        let p = presenter()
        let source = try XCTUnwrap(p.views[1])
        XCTAssertTrue(source.becomeFirstResponder())
        var events: [String] = []
        p.onBlur = { _ in events.append("blur") }
        p.menus.focus.setAside(untilTouch: true)
        XCTAssertFalse(source.isFirstResponder)
        XCTAssertEqual(events, ["blur"])
        XCTAssertFalse(p.menus.focus.untilNextTouch, "with no session view, nothing waits to return")
        // The bar's recognizer is installed once, and holds no touch.
        let bar = UINavigationBar(frame: CGRect(x: 0, y: 0, width: 400, height: 44))
        p.menus.focus.watch(bar)
        p.menus.focus.watch(bar)
        let touches = (bar.gestureRecognizers ?? []).filter { $0 is BarTouch }
        XCTAssertEqual(touches.count, 1)
        XCTAssertEqual(touches.first?.cancelsTouchesInView, false)
        XCTAssertEqual(touches.first?.delaysTouchesBegan, false)
        XCTAssertEqual((p.viewport.gestureRecognizers ?? []).filter { $0 is BarTouch }.count, 1, "and one on the viewport, carried into a presentation")
    }

    func testAnInertPopoverRefusesTheCommit() throws {
        if ExactEnv.agentMode { throw XCTSkip("under the agent the popover is painted (D4)") }
        let p = presenter()
        let (interaction, configuration, _) = try opened(p)
        var pressed: [UInt32] = []
        p.onPress = { pressed.append($0) }
        p.apply(wireBatch([["op": "props", "id": 2, "set": ["inert": "true"], "clear": []]]))
        let commit = Commit()
        p.menus.context.contextMenuInteraction(interaction, willPerformPreviewActionForMenuWith: configuration, animator: commit)
        XCTAssertEqual(pressed, [], "inert where it is authored, though not in the preview's tree")
        XCTAssertEqual(commit.preferredCommitStyle, .dismiss)
        p.apply(wireBatch([["op": "props", "id": 2, "clear": ["inert"]], ["op": "style", "id": 3, "style": ["display": "none"]]]))
        p.menus.context.contextMenuInteraction(interaction, willPerformPreviewActionForMenuWith: configuration, animator: commit)
        XCTAssertEqual(pressed, [], "nor a row hidden where it is authored")
        p.apply(wireBatch([["op": "style", "id": 3, "style": ["display": "block"]], ["op": "style", "id": 2, "style": ["display": "none"]]]))
        p.menus.context.contextMenuInteraction(interaction, willPerformPreviewActionForMenuWith: configuration, animator: commit)
        XCTAssertEqual(pressed, [], "nor a popover the author hid, rather than the host")
        p.apply(wireBatch([["op": "style", "id": 2, "style": ["display": "block"]]]))
        p.menus.context.contextMenuInteraction(interaction, willPerformPreviewActionForMenuWith: configuration, animator: commit)
        XCTAssertEqual(pressed, [3])
        XCTAssertEqual(commit.preferredCommitStyle, .dismiss, "a press that stays")
        XCTAssertTrue(p.menus.context.contextMenuInteraction(interaction, previewForDismissingMenuWithConfiguration: configuration)?.view === p.views[1],
                      "and returns to its source")
    }
}
#endif
