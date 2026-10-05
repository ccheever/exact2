#if os(iOS)
import UIKit
import XCTest
@testable import ExactKit

/// LLP 1021 §5.1 on UIKit: a node naming a popover with `contextPopover` is
/// a UIContextMenuInteraction. The node lifts as its own targeted preview,
/// the popover's `contextPreview` row is the preview controller's view
/// (sized by its layout), its menu rows the UIMenu, and tapping the preview
/// presses that row: `.pop` when the press navigates, pushed without the
/// stack's animation, `.dismiss` otherwise. The native fixture's `peek` row.
/// UIKit, so a simulator runs it: bun host/apple/build.mjs --test --ios
final class ContextMenuIOSTests: XCTestCase {
    private var window: UIWindow?
    private var session: ExactSession?

    override func tearDown() {
        session?.destroy()
        window?.isHidden = true
        window = nil
        session = nil
        super.tearDown()
    }

    /// UIKit's commit animator, as the delegate sees it.
    private final class Commit: NSObject, UIContextMenuInteractionCommitAnimating {
        var preferredCommitStyle: UIContextMenuInteractionCommitStyle = .pop
        var previewViewController: UIViewController? { nil }
        var animated = 0
        func addAnimations(_ animations: @escaping () -> Void) { animated += 1; animations() }
        func addCompletion(_ completion: @escaping () -> Void) { completion() }
    }

    private func boot() throws -> ExactSession {
        if ExactEnv.agentMode { throw XCTSkip("under the agent the popover is painted (D4)") }
        let plan = try Data(contentsOf: URL(fileURLWithPath: try XCTUnwrap(ProcessInfo.processInfo.environment["EXACT_FIXTURE_PLAN"], "build.mjs --test --ios compiles the fixture's plan")))
        let session = ExactApp.shared.makeSession(label: "context-menu")
        self.session = session
        let view = ExactView(session: session), host = UIViewController()
        let scene = UIApplication.shared.connectedScenes.first as? UIWindowScene
        let window = scene.map { UIWindow(windowScene: $0) } ?? UIWindow(frame: CGRect(x: 0, y: 0, width: 402, height: 874))
        window.frame = CGRect(x: 0, y: 0, width: 402, height: 874)
        window.rootViewController = host
        window.makeKeyAndVisible()
        host.view.addSubview(view)
        self.window = window
        XCTAssertNil(session.boot(plan: plan, size: window.bounds.size).error)
        view.frame = host.view.bounds
        view.layoutIfNeeded()
        RunLoop.main.run(until: Date().addingTimeInterval(0.3))
        return session
    }
    private func node(_ session: ExactSession, _ testId: String) throws -> NodeView {
        try XCTUnwrap(session.presenter.views.values.first { $0.props["testId"] == testId }, testId)
    }
    private func text(_ node: NodeView) -> String { node.paragraphSpec().runs.map(\.text).joined() }

    func testTheRowLiftsIntoUIKitsMenuWithItsPreviewAndThePreviewPopsIntoItsScreen() throws {
        let session = try boot()
        let host = session.presenter.menus.context
        let peek = try node(session, "peek")
        let interaction = try XCTUnwrap(peek.interactions.compactMap { $0 as? UIContextMenuInteraction }.first, "the node's context menu is UIKit's")
        XCTAssertEqual(peek.contextRecognizer?.isEnabled, false, "its plain long press is off: the interaction recognizes it")
        XCTAssertEqual(text(try node(session, "peeks")), "peeks 0")

        let configuration = try XCTUnwrap(host.contextMenuInteraction(interaction, configurationForMenuAtLocation: CGPoint(x: 10, y: 10)))
        XCTAssertEqual(text(try node(session, "peeks")), "peeks 0", "nothing fires before the menu is shown")
        XCTAssertTrue(host.contextMenuInteraction(interaction, previewForHighlightingMenuWithConfiguration: configuration)?.view === peek,
                      "UIKit lifts the node itself")
        let owner = try XCTUnwrap(host.open)
        let controller = try XCTUnwrap(host.lift(owner))
        let menu = try XCTUnwrap(host.menu(owner))
        XCTAssertEqual(text(try node(session, "peeks")), "peeks 1", "its own contextmenu, once, before the rows or preview are read")

        let preview = try node(session, "peek-preview")
        XCTAssertTrue(preview.superview === controller.view, "the preview row is the controller's view's content")
        XCTAssertFalse(preview.isHidden)
        XCTAssertEqual(controller.preferredContentSize, CGSize(width: 300, height: 180), "sized by its layout: the popover's 300-point content box")
        XCTAssertEqual(preview.frame.origin, .zero)
        // Two sections (the hr), the preview not among the items.
        let titles = menu.children.flatMap { ($0 as? UIMenu)?.children ?? [$0] }.map { ($0 as? UIAction)?.title ?? "" }
        XCTAssertEqual(titles, ["Bump", "Clear"])
        XCTAssertEqual((menu.children.last as? UIMenu)?.children.first.flatMap { ($0 as? UIAction)?.attributes.contains(.destructive) }, true)
        XCTAssertEqual(session.presenter.menus.observation()?["kind"] as? String, "contextmenu")

        // Tapping the preview: its press pushes Detail, unanimated, and UIKit pops into it.
        let before = session.presenter.navigation.activeKey
        let commit = Commit()
        commit.preferredCommitStyle = .dismiss
        host.contextMenuInteraction(interaction, willPerformPreviewActionForMenuWith: configuration, animator: commit)
        XCTAssertEqual(commit.preferredCommitStyle, .pop, "a press that navigates commits with .pop")
        XCTAssertEqual(commit.animated, 1, "with an animation, without which UIKit shrinks the preview away")
        XCTAssertNotEqual(session.presenter.navigation.activeKey, before, "the press pushed")
        XCTAssertNotNil(try node(session, "route-detail"))
        XCTAssertNil(host.contextMenuInteraction(interaction, previewForDismissingMenuWithConfiguration: configuration),
                     "after it navigated, the source is under the new screen: no return to it")
        host.contextMenuInteraction(interaction, willEndFor: configuration, animator: nil)
        XCTAssertNil(host.open)
        let popover = try XCTUnwrap(session.presenter.views.values.first { $0.props["id"] == "peek" })
        XCTAssertTrue(popover.container.subviews.contains(preview), "the row is back in its popover")
        XCTAssertTrue(popover.isHidden, "which stays hidden")
    }

    func testAMenuDismissedWithoutAChoiceReturnsItsRowAndAPreviewThatDoesNotNavigateDismisses() throws {
        let session = try boot()
        let host = session.presenter.menus.context
        let peek = try node(session, "peek")
        let interaction = try XCTUnwrap(peek.interactions.compactMap { $0 as? UIContextMenuInteraction }.first)
        let configuration = try XCTUnwrap(host.contextMenuInteraction(interaction, configurationForMenuAtLocation: .zero))
        let owner = try XCTUnwrap(host.open)
        _ = try XCTUnwrap(host.lift(owner))
        let preview = try node(session, "peek-preview")
        XCTAssertTrue(host.contextMenuInteraction(interaction, previewForDismissingMenuWithConfiguration: configuration)?.view === peek,
                      "dismissed, the preview returns to the node")
        host.contextMenuInteraction(interaction, willEndFor: configuration, animator: nil)
        XCTAssertNil(host.open)
        let popover = try XCTUnwrap(session.presenter.views.values.first { $0.props["id"] == "peek" })
        XCTAssertTrue(popover.container.subviews.contains(preview))

        // A menu item presses its row.
        var pressed: [String] = []
        let press = session.presenter.onPress
        session.presenter.onPress = { [weak presenter = session.presenter] id in pressed.append(presenter?.views[id]?.props["testId"] ?? "?"); press?(id) }
        _ = try XCTUnwrap(host.contextMenuInteraction(interaction, configurationForMenuAtLocation: .zero))
        let menu = try XCTUnwrap(host.menu(try XCTUnwrap(host.open)))
        let bump = try XCTUnwrap(menu.children.flatMap { ($0 as? UIMenu)?.children ?? [$0] }.first as? UIAction)
        UIButton().sendAction(bump)
        XCTAssertEqual(pressed, ["peek-bump"])
        host.contextMenuInteraction(interaction, willEndFor: try XCTUnwrap(host.contextMenuInteraction(interaction, configurationForMenuAtLocation: .zero)), animator: nil)

        // A preview whose press stays on this screen dismisses.
        session.presenter.onPress = { _ in }
        let again = try XCTUnwrap(host.contextMenuInteraction(interaction, configurationForMenuAtLocation: .zero))
        _ = try XCTUnwrap(host.lift(try XCTUnwrap(host.open)))
        let commit = Commit()
        host.contextMenuInteraction(interaction, willPerformPreviewActionForMenuWith: again, animator: commit)
        XCTAssertEqual(commit.preferredCommitStyle, .dismiss)
        XCTAssertEqual(commit.animated, 0)
        host.contextMenuInteraction(interaction, willEndFor: again, animator: nil)
        session.presenter.onPress = press
    }
}

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
