#if os(iOS)
import UIKit
import XCTest
@testable import ExactKit

final class PopoverSheetIOSTests: XCTestCase {
    private final class Touch: UITouch {
        let target: UIView
        init(_ target: UIView) { self.target = target; super.init() }
        override var view: UIView? { target }
        override func location(in view: UIView?) -> CGPoint {
            target.convert(CGPoint(x: target.bounds.midX, y: target.bounds.midY), to: view)
        }
    }

    var window: UIWindow?
    var session: ExactSession?
    var kept: Presenter?
    var hasScene: Bool { UIApplication.shared.connectedScenes.contains { $0 is UIWindowScene } }

    func settle(_ done: () -> Bool, file: StaticString = #filePath, line: UInt = #line) {
        let end = Date().addingTimeInterval(5)
        while !done(), Date() < end { RunLoop.main.run(mode: .default, before: Date().addingTimeInterval(0.05)) }
        XCTAssertTrue(done(), "the presentation settled", file: file, line: line)
    }

    override func tearDown() {
        if let p = kept {
            settle { !p.menus.inTransition && !p.modals.inTransition }
            p.menus.reset()
            p.modals.reset()
            settle { self.window?.rootViewController?.presentedViewController == nil && !p.menus.inTransition && !p.modals.inTransition }
        }
        session?.destroy()
        session = nil
        kept = nil
        window?.isHidden = true
        window = nil
        super.tearDown()
    }

    func testSheetReportsItsScrolledBorderBoxAndSeparateClipUntilClosing() throws {
        guard hasScene else { throw missing("scene", "the dialog must be presented in a sheet") }
        let p = try page(), dialog = try XCTUnwrap(p.views[2])
        p.apply(wireBatch([["op": "frame", "id": 2, "x": 0, "y": 200, "w": 300, "h": 1800]]))
        let sheet = try show(in: p)
        p.viewport.contentOffset = CGPoint(x: 11, y: 37)
        let open = p.openTopLayer
        XCTAssertEqual(open.frames.map { $0.0 }, [2])
        XCTAssertEqual(open.modal, 0)
        let entry = try XCTUnwrap(open.frames.first), clip = try XCTUnwrap(entry.2)
        XCTAssertEqual(entry.1, p.viewportPoints(dialog.convert(dialog.bounds, to: p.viewport)))
        XCTAssertEqual(entry.1.height, 1800, accuracy: 0.5)
        XCTAssertGreaterThan(entry.1.height, clip.height, "the card clips; it must not shorten the reported box")
        XCTAssertEqual(clip, p.viewportPoints(sheet.scroller.convert(sheet.scroller.bounds, to: p.viewport)))
        let offset = sheet.scroller.contentOffset
        sheet.scroller.setContentOffset(CGPoint(x: offset.x, y: offset.y + 120), animated: false)
        let scrolled = try XCTUnwrap(p.openTopLayer.frames.first)
        XCTAssertEqual(scrolled.1.minY, entry.1.minY - 120, accuracy: 0.5)
        XCTAssertEqual(scrolled.1.size, entry.1.size)
        XCTAssertEqual(scrolled.2, clip, "scrolling moves the border box, not the card's clip")

        p.menus.sheets.close(dialog, animated: true)
        XCTAssertTrue(p.menus.sheets.owns(dialog), "UIKit still holds the closing sheet")
        XCTAssertFalse(p.menus.sheets.isOpen(dialog))
        XCTAssertTrue(p.openTopLayer.frames.isEmpty)
        XCTAssertNil(p.openTopLayer.modal)
        settle { !p.menus.inTransition && self.window?.rootViewController?.presentedViewController == nil }
        XCTAssertFalse(p.menus.sheets.owns(dialog))
        XCTAssertTrue(p.openTopLayer.frames.isEmpty)
        XCTAssertNil(p.openTopLayer.modal)
    }

    func testSheetContentIsReadThroughTheRuntimeWithItsClip() throws {
        guard hasScene else { throw missing("scene", "the runner reads content in a presented sheet") }
        let scene = try XCTUnwrap(UIApplication.shared.connectedScenes.compactMap { $0 as? UIWindowScene }.first)
        let plan = try Data(contentsOf: URL(fileURLWithPath: try XCTUnwrap(ProcessInfo.processInfo.environment["EXACT_DIALOG_SHEET_READS_PLAN"])))
        let session = ExactApp.shared.makeSession(label: "dialog-sheet-reads")
        self.session = session
        let p = session.presenter, view = ExactView(session: session), host = UIViewController()
        kept = p
        let window = UIWindow(windowScene: scene)
        window.frame = scene.screen.bounds
        window.rootViewController = host
        window.makeKeyAndVisible()
        self.window = window
        host.view.addSubview(view)
        XCTAssertNil(session.boot(plan: plan, size: window.bounds.size).error)
        view.frame = host.view.bounds
        view.layoutIfNeeded()
        session.firstDrawMs = ExactEnv.wall()
        let sheet = try show(in: p)
        func named(_ id: String) throws -> NodeView {
            try XCTUnwrap(p.views.values.first { $0.props["id"] == id }, id)
        }
        let dialog = try named("form"), content = try named("content"), clipped = try named("clipped")
        let clip = try XCTUnwrap(p.openTopLayer.frames.first { $0.0 == dialog.id }?.2)
        func point(_ node: NodeView) -> CGPoint {
            let frame = p.viewportPoints(node.convert(node.bounds, to: p.viewport))
            return CGPoint(x: frame.midX, y: frame.midY)
        }
        func read(_ at: CGPoint) throws -> [String: Any] {
            let sample = PointerSample(x: 0, y: 0, buttons: 0, pressure: 0, type: "mouse", id: 1,
                                       clientX: Double(at.x), clientY: Double(at.y))
            let batch = session.runtime.pointer(dialog.id, .move, sample, now: session.now())
            XCTAssertNil(batch.error)
            session.apply(batch)
            let state = try XCTUnwrap(try JSONSerialization.jsonObject(with: Data(session.runtime.agent("{\"op\":\"state\"}").utf8)) as? [String: Any])
            return try XCTUnwrap(state["slots"] as? [String: Any])
        }
        XCTAssertTrue(clip.contains(point(content)))
        XCTAssertFalse(clip.contains(point(clipped)))
        let inside = try read(point(content))
        XCTAssertEqual(inside["hit"] as? String, "content")
        XCTAssertEqual(inside["hits"] as? String, "content,form")
        let outside = try read(point(clipped))
        XCTAssertEqual(outside["hit"] as? String, "none")
        XCTAssertEqual(outside["hits"] as? String, "")
        XCTAssertTrue(content.isDescendant(of: sheet.scroller))
    }

    /// A route presentation whose routes show no box yet (not in the window
    /// while UIKit presents it) still begins the modal boundary.
    func testARoutePresentationWithNoBoxYetKeepsTheModalBoundary() throws {
        guard hasScene else { throw missing("scene", "a route presentation on its way to the window") }
        let p = try page()
        p.apply(wireBatch([
            ["op": "create", "id": 30, "kind": "view", "props": ["navigationPresentation": "modal"]],
            ["op": "frame", "id": 30, "x": 0, "y": 0, "w": 300, "h": 600],
            ["op": "children", "id": 30, "ids": [20]],
            ["op": "children", "id": 1, "ids": [2, 30]],
        ]))
        let route = try XCTUnwrap(p.views[30]), owner = try XCTUnwrap(window?.rootViewController)
        let navigation = UINavigationController(rootViewController: RouteController(route))
        p.modals.present(route, navigation: navigation, from: owner, preceding: nil, owner: owner)
        XCTAssertNil(navigation.view.window, "UIKit has not put it in the window yet")
        XCTAssertEqual(p.openTopLayer.frames.map { $0.0 }, [])
        XCTAssertEqual(p.openTopLayer.modal, 0, "the page is inert behind it already")
        settle { !p.modals.inTransition }
        XCTAssertEqual(p.openTopLayer.frames.map { $0.0 }, [30])
        XCTAssertEqual(p.openTopLayer.modal, 0)
        p.modals.closeTop(animated: false)
        settle { !p.modals.inTransition }
    }

    func testRouteDialogAndPopoverReportTheirUIKitStackOrder() throws {
        guard hasScene else { throw missing("scene", "the dialog must be above a presented route") }
        let p = try page()
        p.apply(wireBatch([
            ["op": "create", "id": 30, "kind": "view", "props": ["navigationPresentation": "modal"]],
            ["op": "frame", "id": 30, "x": 0, "y": 0, "w": 300, "h": 600],
            ["op": "children", "id": 30, "ids": [2, 20]],
            ["op": "children", "id": 1, "ids": [30]],
        ]))
        let route = try XCTUnwrap(p.views[30]), owner = try XCTUnwrap(window?.rootViewController)
        let navigation = UINavigationController(rootViewController: RouteController(route))
        p.modals.present(route, navigation: navigation, from: owner, preceding: nil, owner: owner)
        settle { !p.modals.inTransition }
        XCTAssertEqual(p.openTopLayer.frames.map { $0.0 }, [30])
        XCTAssertEqual(p.openTopLayer.modal, 0)
        let sheet = try show(in: p)
        XCTAssertEqual(p.openTopLayer.frames.map { $0.0 }, [30, 2])
        XCTAssertEqual(p.openTopLayer.modal, 1)
        p.menus.action("showPopover", ["choices", "opener"])
        try assertAbove(try XCTUnwrap(p.views[5]), sheet)
        XCTAssertEqual(p.openTopLayer.frames.map { $0.0 }, [30, 2, 5])
        XCTAssertEqual(p.openTopLayer.modal, 1)
        p.menus.action("hidePopover", ["choices"])
        p.menus.sheets.close(try XCTUnwrap(p.views[2]), animated: false)
        settle { !p.menus.inTransition && sheet.presentingViewController == nil }
        XCTAssertEqual(p.openTopLayer.frames.map { $0.0 }, [30])
        XCTAssertEqual(p.openTopLayer.modal, 0)
        p.modals.closeTop(animated: true)
        XCTAssertNil(p.openTopLayer.modal, "a retiring route no longer owns the modal boundary")
        settle { !p.modals.inTransition }
    }

    func testPopoverAboveSheetKeepsItsAnchorAndReportedFrame() throws {
        guard hasScene else { throw missing("scene", "stacking and hit testing need a presented sheet") }
        let session = ExactApp.shared.makeSession(label: "popover-sheet")
        let p = try page(session: session), sheet = try show(in: p)
        let pop = try XCTUnwrap(p.views[5]), opener = try XCTUnwrap(p.views[4]), content = try XCTUnwrap(p.views[6])
        p.menus.action("showPopover", ["choices", "opener"])
        p.menus.sync()
        try assertAbove(pop, sheet)
        let anchor = opener.convert(opener.bounds, to: window), drawn = pop.convert(pop.bounds, to: window)
        XCTAssertEqual(drawn.midX, anchor.midX, accuracy: 0.5)
        XCTAssertEqual(drawn.minY, anchor.maxY, accuracy: 0.5)
        XCTAssertEqual(p.openTopLayer.frames.map { $0.0 }, [2, 5])
        XCTAssertEqual(p.openTopLayer.modal, 0)
        XCTAssertNotNil(p.openTopLayer.frames.first?.2)
        XCTAssertNil(p.openTopLayer.frames.last?.2)
        let reported = try XCTUnwrap(p.openTopLayerFrames.first { $0.0 == pop.id }?.1)
        let expected = p.viewportPoints(pop.convert(pop.bounds, to: p.viewport))
        XCTAssertEqual(reported.minX, expected.minX, accuracy: 0.5)
        XCTAssertEqual(reported.minY, expected.minY, accuracy: 0.5)
        XCTAssertEqual(reported.size, expected.size)
        XCTAssertTrue(Agent.shows(session, content))
        XCTAssertTrue(p.menus.sheets.shows(content))
        XCTAssertEqual(Agent(session: session).presentedOverApp(), "sheet", "a screenshot includes the window's sheet and popover")

        p.menus.action("showPopover", ["submenu", "sub-opener"])
        let sub = try XCTUnwrap(p.views[8]), parentLayer = try XCTUnwrap(pop.superview), subLayer = try XCTUnwrap(sub.superview)
        p.menus.sync()
        XCTAssertTrue(parentLayer.superview === subLayer.superview)
        let siblings = try XCTUnwrap(parentLayer.superview).subviews
        XCTAssertGreaterThan(try XCTUnwrap(siblings.firstIndex(of: subLayer)), try XCTUnwrap(siblings.firstIndex(of: parentLayer)))
        try assertAbove(sub, sheet)
        XCTAssertEqual(p.openTopLayer.frames.map { $0.0 }, [2, 5, 8])
        XCTAssertEqual(p.openTopLayer.modal, 0)
    }

    func testOpeningOrderSurvivesSyncAndReopeningMovesAboveSheet() throws {
        guard hasScene else { throw missing("scene", "a later UIKit sheet must cover an older popover") }
        let p = try page(), pop = try XCTUnwrap(p.views[5]), dialog = try XCTUnwrap(p.views[2])
        p.menus.action("showPopover", ["choices"])
        let layer = try XCTUnwrap(pop.superview), host = try XCTUnwrap(layer.superview)
        let sheet = try show(in: p)
        p.menus.sync(); p.menus.sync()
        XCTAssertTrue(layer.superview === host)
        XCTAssertEqual(p.openTopLayer.frames.map { $0.0 }, [5, 2])
        XCTAssertEqual(p.openTopLayer.modal, 1)
        XCTAssertTrue(pop.isDescendant(of: p.viewport), "authorship inside the dialog must not choose its later sheet")
        XCTAssertTrue(p.listenerParent(of: try XCTUnwrap(p.views[3])) === dialog)
        let point = pop.convert(CGPoint(x: pop.bounds.midX, y: pop.bounds.midY), to: window)
        XCTAssertFalse(try XCTUnwrap(window?.hitTest(point, with: nil)).isDescendant(of: pop))
        p.menus.action("hidePopover", ["choices"])
        p.menus.action("showPopover", ["choices", "opener"])
        try assertAbove(pop, sheet)
        XCTAssertEqual(p.openTopLayer.frames.map { $0.0 }, [2, 5])
        XCTAssertEqual(p.openTopLayer.modal, 0)
    }

    func testAccessibilityKeepsTheLiftedButtonInsideItsModalUntilALaterSheetOpens() throws {
        guard hasScene else { throw missing("scene", "accessibility must include a popover above a presented sheet") }
        let p = try page()
        p.apply(wireBatch([
            ["op": "create", "id": 11, "kind": "button", "handlers": ["press"],
             "props": ["testId": "choice", "accessibilityLabel": "Choose"]],
            ["op": "frame", "id": 11, "x": 10, "y": 60, "w": 160, "h": 40],
            ["op": "children", "id": 5, "ids": [6, 7, 8, 11]],
        ]))
        let sheet = try show(in: p), pop = try XCTUnwrap(p.views[5]), button = try XCTUnwrap(p.views[11])
        p.menus.action("showPopover", ["choices", "opener"])
        p.menus.sync()
        try assertAbove(pop, sheet)
        XCTAssertFalse(button.isDescendant(of: try XCTUnwrap(p.views[2])))

        func choice(modal id: UInt32) throws -> [String: Any] {
            let roots: [UIView] = p.accessibilitySurfaces
            let ax = p.axElements(roots: roots)
            let modal = try XCTUnwrap(ax["modal"] as? [String: Any])
            XCTAssertEqual(modal["present"] as? Bool, true)
            XCTAssertEqual(modal["id"] as? UInt32, id)
            let elements = try XCTUnwrap(ax["elements"] as? [[String: Any]])
            return try XCTUnwrap(elements.first { $0["testId"] as? String == "choice" })
        }
        let inside = try choice(modal: 2)
        XCTAssertEqual(inside["name"] as? String, "Choose")
        XCTAssertEqual(inside["role"] as? String, "button")
        XCTAssertEqual(inside["interactive"] as? Bool, true)
        XCTAssertEqual(inside["outsideModal"] as? Bool, false)
        XCTAssertTrue(p.mounts(button), "the session's, as a native editor there must be")

        // A modal narrower than the sheet, which the popover is not authored in.
        p.apply(wireBatch([["op": "props", "id": 10, "set": ["accessibilityModal": "true"]]]))
        try XCTUnwrap(p.views[10]).accessibilityViewIsModal = true
        try XCTUnwrap(p.views[2]).accessibilityViewIsModal = false
        XCTAssertEqual(try choice(modal: 10)["outsideModal"] as? Bool, true, "sharing the sheet is no membership in a nested modal")
        p.apply(wireBatch([["op": "props", "id": 10, "clear": ["accessibilityModal"]]]))
        try XCTUnwrap(p.views[10]).accessibilityViewIsModal = false
        try XCTUnwrap(p.views[2]).accessibilityViewIsModal = true

        // A later sheet covers the lower one, its popover with it.
        let upper = try show("second", in: p)
        p.menus.sync()
        XCTAssertTrue(PopoverLayer.containing(button)?.sheet === sheet)
        XCTAssertEqual(p.openTopLayer.frames.map { $0.0 }, [2, 5, 20])
        XCTAssertEqual(p.openTopLayer.modal, 2)
        let covered = p.axElements(roots: p.accessibilitySurfaces)
        XCTAssertNil((covered["elements"] as? [[String: Any]])?.first { $0["testId"] as? String == "choice" }, "hidden under the later sheet, as its card is")
        p.menus.sheets.close(try XCTUnwrap(p.views[20]), animated: false)
        settle { sheet.presentedViewController == nil && !p.menus.inTransition }
        XCTAssertNil(upper.view.window)
        XCTAssertEqual(try choice(modal: 2)["outsideModal"] as? Bool, false, "exposed again once the later sheet closes")
        XCTAssertEqual(p.openTopLayer.frames.map { $0.0 }, [2, 5])
        XCTAssertEqual(p.openTopLayer.modal, 0)
    }

    func testRetiringRouteModalMovesExistingAndNewPopoversToTheViewport() throws {
        guard hasScene else { throw missing("scene", "route modals must remain in the window while retiring") }
        try retiringRouteModal(withLower: false)
    }

    func testRetiringRouteModalMovesExistingAndNewPopoversToTheNextLiveModal() throws {
        guard hasScene else { throw missing("scene", "route modals must remain in the window while retiring") }
        try retiringRouteModal(withLower: true)
    }

    /// A route modal presented after a popover opened takes the popover up
    /// with it, as at BASE, so the runner's top-layer order (popovers after
    /// routes, `openTopLayer`) matches what UIKit draws.
    func testARouteModalPresentedLaterHostsAnOpenPopover() throws {
        guard hasScene else { throw missing("scene", "a route modal must be presented") }
        let p = try page()
        p.apply(wireBatch([
            ["op": "create", "id": 30, "kind": "view", "props": ["navigationPresentation": "modal"]],
            ["op": "frame", "id": 30, "x": 0, "y": 0, "w": 300, "h": 360],
            ["op": "children", "id": 1, "ids": [2, 20, 30]],
        ]))
        let pop = try XCTUnwrap(p.views[5])
        p.menus.action("showPopover", ["choices"])
        XCTAssertTrue(pop.isDescendant(of: p.viewport))
        let route = try XCTUnwrap(p.views[30]), owner = try XCTUnwrap(window?.rootViewController)
        let navigation = UIViewController()
        navigation.view.addSubview(route)
        p.modals.present(route, navigation: navigation, from: owner, preceding: nil, owner: owner)
        settle { !p.modals.inTransition }
        let modal = try XCTUnwrap(p.modals.owner)
        p.menus.sync()
        XCTAssertTrue(p.menus.isOpen(pop))
        XCTAssertTrue(pop.isDescendant(of: modal.view), "above the route modal, where the runner's order puts it")
        let point = pop.convert(CGPoint(x: pop.bounds.midX, y: pop.bounds.midY), to: window)
        XCTAssertTrue(try XCTUnwrap(window?.hitTest(point, with: nil)).isDescendant(of: pop))
        p.modals.closeTop(animated: false)
        settle { !p.modals.inTransition }
    }

    private func retiringRouteModal(withLower: Bool) throws {
        let p = try page()
        p.apply(wireBatch([
            ["op": "create", "id": 30, "kind": "view", "props": ["navigationPresentation": "modal"]],
            ["op": "create", "id": 31, "kind": "view", "props": ["navigationPresentation": "modal"]],
            ["op": "frame", "id": 30, "x": 0, "y": 0, "w": 300, "h": 360],
            ["op": "frame", "id": 31, "x": 0, "y": 0, "w": 300, "h": 360],
            ["op": "children", "id": 2, "ids": [10]],
            ["op": "children", "id": 31, "ids": [3]],
            ["op": "children", "id": 1, "ids": [2, 20, 30, 31]],
        ]))
        func present(_ id: UInt32) throws -> UIViewController {
            let route = try XCTUnwrap(p.views[id]), owner = try XCTUnwrap(p.modals.owner ?? window?.rootViewController)
            let navigation = UIViewController()
            navigation.view.addSubview(route)
            p.modals.present(route, navigation: navigation, from: owner, preceding: nil, owner: owner)
            settle { !p.modals.inTransition }
            let modal = try XCTUnwrap(p.modals.owner)
            XCTAssertTrue(route.isDescendant(of: modal.view))
            XCTAssertTrue(modal.view.window === window)
            return modal
        }
        let lower = withLower ? try present(30) : nil
        let upper = try present(31), pop = try XCTUnwrap(p.views[5])
        let fallback: UIView = lower?.view ?? p.viewport
        p.menus.action("showPopover", ["choices", "opener"])
        XCTAssertTrue(pop.superview?.superview === upper.view)
        XCTAssertTrue(p.menus.isOpen(pop))

        let layer = try XCTUnwrap(pop.superview)
        p.modals.closeTop(animated: true)
        // No run-loop turn: retirement has started, but UIKit has not detached the view.
        XCTAssertTrue(p.modals.isDismissing)
        XCTAssertTrue(upper.view.window === window)
        p.menus.sync()
        XCTAssertTrue(pop.superview === layer)
        XCTAssertTrue(layer.superview === fallback)
        XCTAssertFalse(pop.isDescendant(of: upper.view))
        XCTAssertTrue(p.menus.isOpen(pop))

        p.menus.action("hidePopover", ["choices"])
        p.menus.action("showPopover", ["choices"])
        if withLower {
            // The retiring route's content is behind the live modal, inert (`inertBehind`): it opens nothing.
            XCTAssertTrue(pop.inert)
            XCTAssertFalse(p.menus.isOpen(pop))
            settle { !p.modals.inTransition }
            return
        }
        XCTAssertFalse(pop.superview === layer, "reopening captures a new set of hosts during retirement")
        XCTAssertTrue(pop.superview?.superview === fallback)
        XCTAssertFalse(pop.isDescendant(of: upper.view))
        XCTAssertTrue(p.menus.isOpen(pop))
        XCTAssertTrue(upper.view.window === window)
        settle { !p.modals.inTransition }
    }

    func testPressBubblesThroughTheLiftAndClosingReturnsToAuthoredParent() throws {
        guard hasScene else { throw missing("scene", "the bubbled press must start above the sheet") }
        let p = try page(), sheet = try show(in: p)
        let parent = try XCTUnwrap(p.views[3]), pop = try XCTUnwrap(p.views[5]), content = try XCTUnwrap(p.views[6])
        let index = try XCTUnwrap(parent.subviews.firstIndex(of: pop)), center = pop.center
        p.menus.action("showPopover", ["choices", "opener"])
        try assertAbove(pop, sheet)
        XCTAssertTrue(p.listenerParent(of: pop) === parent)
        XCTAssertTrue(p.menus.logicalParent(of: pop) === parent)
        let layer = try XCTUnwrap(pop.superview)
        let pointer = try XCTUnwrap(layer.gestureRecognizers?.compactMap { $0 as? PointerRecognizer }.first)
        XCTAssertTrue(pointer.relay?.listener(content, { $0.wantsPointer }) === parent)
        let path = PressPath(from: content, to: parent, presenter: p)
        XCTAssertTrue(path.isLive(in: p))
        var presses: [UInt32] = []
        p.onPress = { presses.append($0) }
        let touch = Touch(content)
        content.touchesBegan([touch], with: nil)
        XCTAssertTrue(parent.pressed)
        content.touchesEnded([touch], with: nil)
        XCTAssertEqual(presses, [parent.id])
        XCTAssertFalse(parent.pressed)
        p.menus.action("hidePopover", ["choices"])
        XCTAssertTrue(pop.superview === parent)
        XCTAssertEqual(parent.subviews.firstIndex(of: pop), index)
        XCTAssertEqual(pop.center, center)
        XCTAssertTrue(pop.isHidden)
        XCTAssertNil(layer.superview)
        XCTAssertNil(pointer.relay)
        XCTAssertFalse(path.isLive(in: p))
    }

    func testClosingHostingSheetsFallsBackThroughTheCapturedStack() throws {
        guard hasScene else { throw missing("scene", "fallback crosses two presented sheet containers") }
        let p = try page(), lower = try show(in: p), upper = try show("second", in: p)
        let pop = try XCTUnwrap(p.views[5])
        p.menus.action("showPopover", ["choices"])
        // `choices` is the lower dialog's: the upper modal makes it inert, so it stays under that sheet.
        XCTAssertTrue(pop.superview?.superview === lower.presentationController?.containerView)
        XCTAssertNotNil(upper.view.window)
        let covered = pop.convert(CGPoint(x: pop.bounds.midX, y: pop.bounds.midY), to: window)
        XCTAssertFalse(try XCTUnwrap(window?.hitTest(covered, with: nil)).isDescendant(of: pop))
        p.menus.sheets.close(try XCTUnwrap(p.views[20]), animated: false)
        settle { lower.presentedViewController == nil && !p.menus.inTransition }
        try assertAbove(pop, lower)
        XCTAssertTrue(p.menus.isOpen(pop))
        p.menus.sheets.close(try XCTUnwrap(p.views[2]), animated: false)
        settle { self.window?.rootViewController?.presentedViewController == nil && !p.menus.inTransition }
        XCTAssertTrue(p.menus.isOpen(pop))
        XCTAssertTrue(pop.superview?.superview === p.viewport)
        XCTAssertTrue(pop.window === window)
        XCTAssertFalse(pop.isHidden)
        let point = pop.convert(CGPoint(x: pop.bounds.midX, y: pop.bounds.midY), to: window)
        XCTAssertTrue(try XCTUnwrap(window?.hitTest(point, with: nil)).isDescendant(of: pop))
    }

    /// Over a sheet the layer takes the session's theme as the sheet does,
    /// stays clear, and a popover whose opener is gone is recentred.
    func testALayerOverASheetTakesTheThemeAndAGoneOpenerRecentresIt() throws {
        guard hasScene else { throw missing("scene", "the popover must be lifted over a presented sheet") }
        let p = try page(), sheet = try show(in: p), pop = try XCTUnwrap(p.views[5])
        p.menus.action("showPopover", ["choices", "opener"])
        try assertAbove(pop, sheet)
        let layer = try XCTUnwrap(pop.superview)
        XCTAssertTrue(ThemeRoles.adopted(by: p.root).contains { $0 === layer }, "the session's theme, outside its root")
        p.applyThemeTint()
        XCTAssertNil(layer.backgroundColor, "the layer itself paints nothing")
        let middle = CGPoint(x: layer.bounds.midX, y: layer.bounds.midY)
        XCTAssertNotEqual(pop.center, middle, "anchored to its opener")
        p.apply(wireBatch([["op": "children", "id": 3, "ids": [5]], ["op": "destroy", "id": 4]]))
        p.menus.sync()
        XCTAssertNil(p.views[4])
        XCTAssertTrue(p.menus.isOpen(pop))
        XCTAssertEqual(pop.center, middle, "its opener gone, it is centred in the layer")
    }

    /// Moved out of the dialog while open, the popover is the modal's no
    /// more: it leaves the sheet and goes under it, as at its opening.
    func testAPopoverMovedOutOfTheDialogLeavesItsSheet() throws {
        guard hasScene else { throw missing("scene", "the popover must be lifted over a presented sheet") }
        let p = try page(), sheet = try show(in: p), pop = try XCTUnwrap(p.views[5])
        p.menus.action("showPopover", ["choices", "opener"])
        try assertAbove(pop, sheet)
        p.apply(wireBatch([["op": "children", "id": 2, "ids": [10]], ["op": "children", "id": 1, "ids": [2, 20, 3]]]))
        p.menus.sync()
        XCTAssertTrue(p.menus.isOpen(pop))
        XCTAssertTrue(pop.isDescendant(of: p.viewport), "hosted where a popover outside the modal opens")
        XCTAssertEqual(p.openTopLayer.frames.map { $0.0 }, [5, 2])
        XCTAssertEqual(p.openTopLayer.modal, 1)
        let point = pop.convert(CGPoint(x: pop.bounds.midX, y: pop.bounds.midY), to: window)
        XCTAssertFalse(try XCTUnwrap(window?.hitTest(point, with: nil)).isDescendant(of: pop), "under the sheet, which takes the touch")
    }

    /// Its sheet going (the app's close, UIKit's own dismissal), a popover
    /// over it leaves the sheet's container while that is still in the
    /// window: a field focused in it keeps the focus, as when the popover
    /// was under the sheet; `layout agree` walks the layer meanwhile.
    func testTheSheetGoingKeepsTheFocusInItsPopover() throws {
        guard hasScene else { throw missing("scene", "the sheet must be presented and dismissed") }
        for byUIKit in [false, true] {
            let p = try page(), sheet = try show(in: p)
            p.apply(wireBatch([
                ["op": "create", "id": 11, "kind": "input", "props": ["value": "draft"]],
                ["op": "frame", "id": 11, "x": 10, "y": 60, "w": 160, "h": 40],
                ["op": "children", "id": 5, "ids": [6, 7, 8, 11]],
            ]))
            p.menus.action("showPopover", ["choices", "opener"])
            let pop = try XCTUnwrap(p.views[5])
            try assertAbove(pop, sheet)
            XCTAssertTrue(p.surfaces(includingRetiring: true).contains { $0.view === pop.superview }, "layout agree walks the layer over the sheet")
            let field = try XCTUnwrap(p.views[11]?.field)
            XCTAssertTrue(field.becomeFirstResponder())
            var blurs: [UInt32] = []
            p.onBlur = { blurs.append($0) }
            if byUIKit {
                let dismissed = expectation(description: "UIKit dismissed the sheet")
                sheet.dismiss(animated: true) { dismissed.fulfill() }
                wait(for: [dismissed], timeout: 5)
            } else {
                p.menus.sheets.close(try XCTUnwrap(p.views[2]), animated: true)
            }
            settle { self.window?.rootViewController?.presentedViewController == nil && !p.menus.inTransition }
            XCTAssertTrue(p.menus.isOpen(pop))
            XCTAssertTrue(pop.isDescendant(of: p.viewport))
            XCTAssertTrue(field.isFirstResponder, "the popover's field keeps the focus (\(byUIKit ? "UIKit's" : "the app's") dismissal)")
            XCTAssertEqual(blurs, [])
            tearDown()
        }
    }

    /// What the agent reads of a popover over a sheet: a finger reaches the
    /// layer's box on screen, and a geometry animation in it is native work
    /// in flight, as one in the page or a route's mount is.
    func testTheAgentReachesAndWatchesAPopoverOverTheSheet() throws {
        guard hasScene else { throw missing("scene", "the popover must be lifted over a presented sheet") }
        let session = ExactApp.shared.makeSession(label: "popover-sheet")
        let p = try page(session: session), sheet = try show(in: p), pop = try XCTUnwrap(p.views[5])
        p.menus.action("showPopover", ["choices", "opener"])
        try assertAbove(pop, sheet)
        let layer = try XCTUnwrap(pop.superview), content = try XCTUnwrap(p.views[6]), win = try XCTUnwrap(window)
        let shown = layer.convert(layer.bounds, to: nil).intersection(win.bounds)
        XCTAssertEqual(p.reach(of: content), p.viewportPoints(p.viewport.convert(shown, from: nil)))
        let agent = Agent(session: session)
        settle { !agent.nativeGeometryInFlight() }
        let move = CABasicAnimation(keyPath: "position")
        move.fromValue = NSValue(cgPoint: content.layer.position)
        move.toValue = NSValue(cgPoint: CGPoint(x: content.layer.position.x + 40, y: content.layer.position.y))
        move.duration = 30
        content.layer.add(move, forKey: "test-move")
        XCTAssertTrue(agent.nativeGeometryInFlight(), "a geometry animation in the popover over the sheet")
        content.layer.removeAnimation(forKey: "test-move")
        XCTAssertFalse(agent.nativeGeometryInFlight())
    }

    func testDetachedPresentationCannotStrandAnOpenPopover() throws {
        guard hasScene else { throw missing("scene", "UIKit must remove the hosting presentation from its window") }
        let p = try page(), sheet = try show(in: p), pop = try XCTUnwrap(p.views[5])
        p.menus.action("showPopover", ["choices", "opener"])
        try assertAbove(pop, sheet)
        let placed = pop.convert(CGPoint(x: pop.bounds.midX, y: pop.bounds.midY), to: nil)
        let dismissed = expectation(description: "UIKit dismissed the host")
        sheet.dismiss(animated: false) { dismissed.fulfill() }
        wait(for: [dismissed], timeout: 5)
        XCTAssertNil(sheet.view.window)
        XCTAssertTrue(pop.window === window, "detachment rehosts without waiting for a runner batch")
        XCTAssertTrue(pop.superview?.superview === p.viewport)
        XCTAssertTrue(p.menus.isOpen(pop))
        let kept = pop.convert(CGPoint(x: pop.bounds.midX, y: pop.bounds.midY), to: nil)
        XCTAssertEqual(kept.x, placed.x, accuracy: 0.5, "its opener out of the window, it keeps its place")
        XCTAssertEqual(kept.y, placed.y, accuracy: 0.5)
    }

    func testSheetTouchesLightDismissAndPopoverEscapesOnlyItsOwnClip() throws {
        guard hasScene else { throw missing("scene", "sheet content and backdrop must receive real hit tests") }
        let p = try page(), sheet = try show(in: p)
        let pop = try XCTUnwrap(p.views[5]), dialog = try XCTUnwrap(p.views[2]), content = try XCTUnwrap(p.views[6])
        let outside = try XCTUnwrap(p.views[10]), frame = dialog.frame
        p.menus.action("showPopover", ["choices", "opener"])
        try assertAbove(pop, sheet)
        let container = try XCTUnwrap(sheet.presentationController?.containerView)
        let observer = try XCTUnwrap(container.gestureRecognizers?.compactMap { $0 as? PopoverTouch }.first)
        XCTAssertTrue(outside.isDescendant(of: container), "the observer is on the touched sheet's ancestry")
        XCTAssertFalse(observer.cancelsTouchesInView)
        XCTAssertTrue(try XCTUnwrap(window?.hitTest(Touch(outside).location(in: window), with: nil)).isDescendant(of: outside))
        observer.touchesBegan([Touch(content)], with: UIEvent())
        XCTAssertTrue(p.menus.isOpen(pop))
        observer.touchesBegan([Touch(outside)], with: UIEvent())
        XCTAssertFalse(p.menus.isOpen(pop))

        pop.applyStyle(["position_area": "top"])
        p.menus.action("showPopover", ["choices", "opener"])
        try assertAbove(pop, sheet)
        let point = pop.convert(CGPoint(x: pop.bounds.midX, y: pop.bounds.midY), to: container)
        XCTAssertFalse(sheet.view.convert(sheet.view.bounds, to: container).contains(point))
        XCTAssertFalse(sheet.gestureRecognizer(UITapGestureRecognizer(), shouldReceive: Touch(content)), "popover content is not the dialog's backdrop")
        XCTAssertEqual(dialog.frame, frame, "lifting changes neither the dialog's frame nor its content extent")
        XCTAssertTrue(sheet.scroller.clipsToBounds)
        let below = UIView(frame: CGRect(x: 20, y: sheet.scroller.bounds.maxY + 40, width: 40, height: 40))
        sheet.scroller.addSubview(below)
        let clipped = below.convert(CGPoint(x: 20, y: 20), to: window)
        XCTAssertFalse(window?.hitTest(clipped, with: nil) === below, "other dialog content keeps the sheet's clip")
    }

    /// The current holding-sheet policy keeps an outside popover under the
    /// sheet. HTML inertness alone does not require that paint order.
    func testAPopoverOutsideTheModalStaysUnderItsSheet() throws {
        guard hasScene else { throw missing("scene", "the sheet must be presented over the page") }
        let p = try page()
        p.apply(wireBatch([
            ["op": "create", "id": 30, "kind": "view", "props": ["id": "outside", "popover": "auto"]],
            ["op": "create", "id": 31, "kind": "text", "props": ["text": "Outside the dialog"]],
            ["op": "frame", "id": 30, "x": 20, "y": 600, "w": 200, "h": 80],
            ["op": "frame", "id": 31, "x": 10, "y": 10, "w": 160, "h": 40],
            ["op": "children", "id": 30, "ids": [31]],
            ["op": "children", "id": 1, "ids": [2, 20, 30]],
        ]))
        _ = try show(in: p)
        p.menus.action("showPopover", ["outside"])
        let pop = try XCTUnwrap(p.views[30])
        XCTAssertTrue(p.menus.isOpen(pop))
        XCTAssertTrue(pop.isDescendant(of: p.viewport), "hosted where it was before the sheet")
        XCTAssertEqual(p.openTopLayer.frames.map { $0.0 }, [30, 2])
        XCTAssertEqual(p.openTopLayer.modal, 1)
        let point = pop.convert(CGPoint(x: pop.bounds.midX, y: pop.bounds.midY), to: window)
        XCTAssertFalse(try XCTUnwrap(window?.hitTest(point, with: nil)).isDescendant(of: pop))
    }

    /// A field in a popover over the sheet is still the session's: ending
    /// the session's editing ends it, as it did under the viewport.
    func testEndingEditingReachesAFieldInAPopoverOverTheSheet() throws {
        guard hasScene else { throw missing("scene", "the popover must be lifted over a presented sheet") }
        let p = try page(), sheet = try show(in: p)
        p.apply(wireBatch([
            ["op": "create", "id": 11, "kind": "input", "props": ["value": "draft"]],
            ["op": "frame", "id": 11, "x": 10, "y": 60, "w": 160, "h": 40],
            ["op": "children", "id": 5, "ids": [6, 7, 8, 11]],
        ]))
        p.menus.action("showPopover", ["choices", "opener"])
        let pop = try XCTUnwrap(p.views[5])
        try assertAbove(pop, sheet)
        let field = try XCTUnwrap(p.views[11]?.field)
        XCTAssertTrue(field.becomeFirstResponder())
        XCTAssertTrue(p.endEditing(), "an editor resigned")
        XCTAssertFalse(field.isFirstResponder)
    }
    private final class StraySurfaceView: UIView {}

    private final class NoResponderView: UIView {
        override var next: UIResponder? { nil }
    }

    private func addPresentationButton(_ p: Presenter, target: String) throws -> NodeView {
        p.buttonFace = { id in var face = ButtonFace(); face.title = id == 42 ? "Continue" : "Open"; return face }
        p.apply(wireBatch([
            ["op": "create", "id": 40, "kind": "control", "handlers": ["press"],
             "props": ["type": "button", "commandfor": target, "command": "show-modal"],
             "style": ["appearance": "auto"]],
            ["op": "frame", "id": 40, "x": 10, "y": 60, "w": 160, "h": 44],
            ["op": "children", "id": 5, "ids": [6, 7, 8, 40]],
        ]))
        let node = try XCTUnwrap(p.views[40])
        XCTAssertTrue(node.isNativeButton)
        return node
    }

    /// Run the button's primary-action handler, which UIKit's sendActions
    /// does not deliver in the bare XCTest host.
    private func activate(_ node: NodeView) { node.activateNative() }

    func testLiftedButtonPresentsAnAlertFromItsSheetWithEitherResponderChain() throws {
        guard hasScene else { throw missing("scene", "the button presents an alert above an existing sheet") }
        for breaksChain in [false, true] {
            let p = try page()
            p.apply(wireBatch([
                ["op": "create", "id": 41, "kind": "view",
                 "props": ["id": "confirm", "semanticTag": "dialog", "accessibilityRole": "alertdialog", "accessibilityLabel": "Continue?"]],
                ["op": "create", "id": 42, "kind": "button", "handlers": ["press"],
                 "props": ["commandfor": "confirm", "command": "close"]],
                ["op": "create", "id": 43, "kind": "button", "props": ["commandfor": "confirm", "command": "close"]],
                ["op": "create", "id": 44, "kind": "text", "props": ["text": "Keep the change."]],
                ["op": "children", "id": 41, "ids": [44, 42, 43]],
                ["op": "children", "id": 2, "ids": [3, 10, 41]],
            ]))
            let button = try addPresentationButton(p, target: "confirm"), sheet = try show(in: p)
            p.menus.action("showPopover", ["choices", "opener"])
            try assertAbove(try XCTUnwrap(p.views[5]), sheet)
            if breaksChain {
                let wrapper = NoResponderView(frame: button.frame), parent = try XCTUnwrap(button.superview)
                parent.addSubview(wrapper); wrapper.addSubview(button); button.frame = wrapper.bounds
            }
            if !breaksChain {
                let chain = sequence(first: button as UIResponder, next: { $0.next }).map { String(describing: type(of: $0)) }
                XCTAssertFalse(chain.contains { $0.hasSuffix("Controller") && $0 != "UIApplication" }, "UIKit's chain from the layer reaches no controller: \(chain)")
            }
            XCTAssertTrue(ViewPresentation.controller(for: button) === sheet)
            XCTAssertTrue(p.rowCells.screen(of: button) === sheet)
            var pressed: [UInt32] = []
            p.onPress = { pressed.append($0) }
            activate(button)
            settle { sheet.presentedViewController is UIAlertController && !p.menus.inTransition }
            let alert = try XCTUnwrap(sheet.presentedViewController as? UIAlertController)
            XCTAssertEqual(alert.preferredStyle, .alert)
            XCTAssertEqual(alert.title, "Continue?")
            XCTAssertEqual(pressed, [40])
            XCTAssertTrue(p.menus.isOpen(try XCTUnwrap(p.views[41])))
            p.menus.closeModal("confirm")
            settle { sheet.presentedViewController == nil && !p.menus.inTransition }
            tearDown()
        }
    }

    func testLiftedButtonPresentsAnotherDialogSheet() throws {
        guard hasScene else { throw missing("scene", "a button above one dialog presents another sheet") }
        for breaksChain in [false, true] {
            let p = try page(), button = try addPresentationButton(p, target: "second"), lower = try show(in: p)
            p.menus.action("showPopover", ["choices", "opener"])
            try assertAbove(try XCTUnwrap(p.views[5]), lower)
            if breaksChain {
                let wrapper = NoResponderView(frame: button.frame), parent = try XCTUnwrap(button.superview)
                parent.addSubview(wrapper); wrapper.addSubview(button); button.frame = wrapper.bounds
            }
            var pressed: [UInt32] = []
            p.onPress = { pressed.append($0) }
            activate(button)
            settle { lower.presentedViewController is DialogSheetController && !p.menus.inTransition }
            let upper = try XCTUnwrap(lower.presentedViewController as? DialogSheetController)
            XCTAssertTrue(p.menus.sheets.top === upper)
            XCTAssertTrue(try XCTUnwrap(p.views[20]).isDescendant(of: upper.view))
            XCTAssertEqual(pressed, [40])
            tearDown()
        }
    }

    /// A picker or share sheet asked from a view out of the window is refused
    /// at once: UIKit would refuse the presentation and call back nothing.
    func testNoPresenterOutOfTheWindow() throws {
        guard hasScene else { throw missing("scene", "a retained controller whose view left the window") }
        _ = try page()
        let owner = UIViewController(), field = UIView()
        owner.view.addSubview(field)
        XCTAssertTrue(ViewPresentation.controller(for: field) === owner, "the chain still reaches it")
        XCTAssertNil(ViewPresentation.presenter(for: field))
        let shown = UIView()
        try XCTUnwrap(window?.rootViewController).view.addSubview(shown)
        XCTAssertTrue(ViewPresentation.presenter(for: shown) === window?.rootViewController)
        shown.removeFromSuperview()
    }

    func testPresentationResolutionPrefersTheLayerThenContainmentThenWindow() throws {
        guard hasScene else { throw missing("scene", "ownership must be resolved above a presented sheet") }
        let p = try page(), sheet = try show(in: p), content = try XCTUnwrap(p.views[6])
        p.menus.action("showPopover", ["choices", "opener"])
        let layer = try XCTUnwrap(PopoverLayer.containing(content))
        XCTAssertTrue(ViewPresentation.controller(for: content) === sheet)
        XCTAssertTrue(ViewPresentation.root(of: content) === layer)
        let detached = NoResponderView()
        XCTAssertNil(ViewPresentation.controller(for: detached))
        try XCTUnwrap(window).addSubview(detached)
        XCTAssertTrue(ViewPresentation.controller(for: detached, fallbackToWindow: true) === window?.rootViewController)
        XCTAssertNil(ViewPresentation.controller(for: detached))
        detached.removeFromSuperview()
        let child = UIViewController(), owner = try XCTUnwrap(window?.rootViewController)
        owner.addChild(child); owner.view.addSubview(child.view); child.didMove(toParent: owner)
        let ordinary = UIView()
        child.view.addSubview(ordinary)
        XCTAssertTrue(ViewPresentation.controller(for: ordinary) === child)
        XCTAssertTrue(ViewPresentation.root(of: ordinary) === owner.view)
        child.willMove(toParent: nil); child.view.removeFromSuperview(); child.removeFromParent()
    }

    /// UIKit takes the layer, outside every controller's view, for a view no
    /// controller holds: making the sheet the screen's parent raises
    /// UIViewControllerHierarchyInconsistency.
    func testNativeScreenInALiftedPopoverIsShownWithoutAParent() throws {
        guard hasScene else { throw missing("scene", "the native screen mounts above a presented dialog") }
        let p = try page(), sheet = try show(in: p)
        p.menus.action("showPopover", ["choices", "opener"])
        let owner = NodeView(id: 40, kind: "native", presenter: p)
        owner.frame = CGRect(x: 0, y: 0, width: 100, height: 60)
        p.views[40] = owner
        try XCTUnwrap(p.views[5]).addSubview(owner)
        PopoverScreenModule.table.withUnsafeBytes { NativeViews.install(table: $0.baseAddress!) }
        let natives = NativeViews()
        natives.install(module: UnsafeMutableRawPointer(bitPattern: 1)!)
        defer {
            natives.destroy(id: 40); natives.destroyModule(); NativeViews.uninstallTable()
            p.views[40] = nil; owner.removeFromSuperview()
        }
        natives.create(owner: owner)
        owner.props = ["nativeViewModuleName": "popover-screen", "nativeViewProps": "{}"]
        natives.update(owner)
        let screen = try XCTUnwrap(PopoverScreenModule.made)
        XCTAssertNotNil(PopoverLayer.containing(owner)?.sheet)
        XCTAssertNil(screen.parent)
        XCTAssertTrue(screen.view.isDescendant(of: owner))
        XCTAssertFalse(sheet.children.contains { $0 === screen })
        natives.destroy(id: 40)
        XCTAssertNil(screen.parent)
        XCTAssertNil(screen.view.superview)
    }

    func testFlightsToAndFromALiftedPopoverStayInTheirPresentation() throws {
        guard hasScene else { throw missing("scene", "flight stacking needs a presented sheet") }
        let p = try page(), sheet = try show(in: p)
        p.menus.action("showPopover", ["choices", "opener"])
        let content = try XCTUnwrap(p.views[6]), formText = try XCTUnwrap(p.views[10])
        let layer = try XCTUnwrap(PopoverLayer.containing(content))
        for (from, to, expected) in [(formText, content, layer as UIView), (content, formText, sheet.view!)] {
            var op = BatchOp(op: .flight, nodeID: to.id)
            op.payload["from"] = NSNumber(value: from.id)
            p.beginFlight(op)
            let flight = try XCTUnwrap(p.flights[to.id])
            XCTAssertTrue(flight.source.root === (from === content ? layer : sheet.view))
            p.presentFlight(to.id, 0.5); p.flightsBatchApplied()
            XCTAssertTrue(try XCTUnwrap(flight.container).superview === expected)
            XCTAssertFalse(flight.container?.superview is UIWindow)
            p.landFlight(flight)
            XCTAssertTrue(to.isDescendant(of: expected))
        }
    }

    func testRasterUrgencyAndRankingDoNotCullSheetOrPopoverTextToTheViewport() throws {
        guard hasScene else { throw missing("scene", "text is painted on a sheet outside the viewport") }
        let session = ExactApp.shared.makeSession(label: "surface-raster"), p = try page(session: session)
        _ = try show(in: p)
        p.menus.action("showPopover", ["choices", "opener"])
        let text = try XCTUnwrap(p.views[6]), formText = try XCTUnwrap(p.views[10])
        // The text engine is the session's.
        p.session = session
        defer { p.session = nil }
        p.viewport.frame.size.height = 20
        p.viewport.clipsToBounds = true
        for node in [text, formText] {
            XCTAssertTrue(node.canRasterText)
            XCTAssertFalse(node.convert(node.bounds, to: p.viewport).intersects(p.viewport.bounds))
            node.dropTextRaster()
        }
        p.paintVisibleText()
        for node in [text, formText] {
            XCTAssertNotNil(node.textRasterKey, "urgent pixels asked for #\(node.id)")
            node.dropTextRaster()
        }
        // A fast upward document scroll must not mark presentation text as passed.
        p.refreshVisibleText(velocity: -10_000)
        for node in [text, formText] {
            XCTAssertNotNil(node.textRasterKey, "ranked pixels asked for #\(node.id)")
        }
    }

    func testARouteClosingUnderADialogSheetLeavesNoStaleEntry() throws {
        guard hasScene else { throw missing("scene", "a route presentation and a dialog's sheet over it") }
        for authoredInRoute in [true, false] {
            let p = try page(), route = try presentRoute(p, authoredInRoute: authoredInRoute)
            p.apply(wireBatch([
                ["op": "create", "id": 11, "kind": "input", "props": ["value": "kept draft"]],
                ["op": "frame", "id": 11, "x": 20, "y": 320, "w": 160, "h": 30],
                ["op": "children", "id": 2, "ids": [3, 10, 11]],
            ]))
            let dialog = try XCTUnwrap(p.views[2]), home = try XCTUnwrap(dialog.superview), frame = dialog.frame
            dialog.handlers.formUnion(["close", "cancel"])
            var closed: [UInt32] = [], cancelled: [UInt32] = []
            p.onClose = { closed.append($0) }
            p.menus.sheets.onCancel = { cancelled.append($0) }
            let sheet = try show(in: p)
            XCTAssertTrue(sheet.presentingViewController === route, "authored in the route: \(authoredInRoute)")
            p.menus.action("showPopover", ["choices", "opener"])
            let pop = try XCTUnwrap(p.views[5]), field = try XCTUnwrap(p.views[11]?.field)
            try assertAbove(pop, sheet)
            XCTAssertTrue(field.becomeFirstResponder())
            p.modals.closeTop(animated: true)
            settle { !p.modals.inTransition && !p.menus.inTransition && route.presentingViewController == nil }
            XCTAssertNil(route.presentingViewController, "the route has gone (authored in the route: \(authoredInRoute))")
            XCTAssertNil(sheet.presentingViewController, "and the original sheet over it")
            XCTAssertFalse(route.view.accessibilityElementsHidden)
            XCTAssertTrue(p.menus.isOpen(pop))
            if authoredInRoute {
                XCTAssertFalse(p.menus.sheets.isOpen(dialog))
                XCTAssertFalse(p.menus.sheets.owns(dialog))
                XCTAssertNil(p.menus.sheets.top)
                XCTAssertTrue(dialog.isHidden)
                XCTAssertTrue(dialog.superview === home)
                XCTAssertEqual(dialog.frame, frame)
                XCTAssertFalse(dialog.accessibilityViewIsModal)
                XCTAssertTrue(pop.isDescendant(of: p.viewport))
            } else {
                let replacement = try XCTUnwrap(p.menus.sheets.top as? DialogSheetController)
                XCTAssertFalse(replacement === sheet)
                XCTAssertTrue(replacement.presentingViewController === window?.rootViewController)
                XCTAssertTrue(p.menus.sheets.isOpen(dialog), "the page's dialog stays open as on the web")
                XCTAssertTrue(p.menus.sheets.authoredParent(of: dialog) === home)
                XCTAssertTrue(dialog.superview === replacement.scroller)
                XCTAssertFalse(dialog.isHidden)
                XCTAssertTrue(dialog.accessibilityViewIsModal)
                XCTAssertTrue(field.isFirstResponder)
                XCTAssertEqual(field.text, "kept draft")
                try assertAbove(pop, replacement)
            }
            flushCallbacks()
            XCTAssertEqual(closed, [], "removing route content is not dialog.close()")
            XCTAssertEqual(cancelled, [])
            tearDown()
        }
    }

    func testAnEmbedderDismissingTheRouteEndsEveryDialogSheet() throws {
        guard hasScene else { throw missing("scene", "an embedder dismisses a route and two dialog sheets") }
        let p = try page(), route = try presentRoute(p)
        let first = try show(in: p), second = try show("second", in: p)
        let dialogs = [try XCTUnwrap(p.views[2]), try XCTUnwrap(p.views[20])]
        for dialog in dialogs { dialog.handlers.formUnion(["close", "cancel"]) }
        var closed: [UInt32] = [], cancelled: [UInt32] = []
        p.onClose = { closed.append($0) }
        p.menus.sheets.onCancel = { cancelled.append($0) }
        let owner = try XCTUnwrap(route.presentingViewController)
        let dismissed = expectation(description: "the embedder dismissed the stack")
        owner.dismiss(animated: true) { dismissed.fulfill() }
        wait(for: [dismissed], timeout: 5)
        settle { !p.menus.inTransition && dialogs.allSatisfy { !p.menus.sheets.owns($0) } }
        XCTAssertNil(route.presentingViewController)
        XCTAssertNil(first.presentingViewController)
        XCTAssertNil(second.presentingViewController)
        XCTAssertNil(p.menus.sheets.top)
        for dialog in dialogs {
            XCTAssertFalse(p.menus.sheets.isOpen(dialog))
            XCTAssertTrue(dialog.isHidden)
            XCTAssertTrue(dialog.superview === p.views[30]?.container)
            XCTAssertFalse(dialog.accessibilityViewIsModal)
        }
        XCTAssertFalse(route.view.accessibilityElementsHidden)
        XCTAssertFalse(first.view.accessibilityElementsHidden, "hiding is unwound from the top down")
        flushCallbacks()
        XCTAssertEqual(closed, [])
        XCTAssertEqual(cancelled, [])
    }

    func testADialogCoveredByAnotherSheetIsNotEnded() throws {
        guard hasScene else { throw missing("scene", "a later full-screen dialog covers the first sheet") }
        let p = try page(), firstOwner = SheetPresenter(try XCTUnwrap(window?.rootViewController))
        let dialog = try XCTUnwrap(p.views[2]), secondDialog = try XCTUnwrap(p.views[20])
        XCTAssertTrue(p.menus.sheets.show(dialog, from: nil, over: firstOwner))
        settle { !p.menus.inTransition }
        let first = try XCTUnwrap(p.menus.sheets.top as? DialogSheetController)
        // Full screen makes UIKit deliver viewDidDisappear to the covered sheet.
        let owner = SheetPresenter(first)
        XCTAssertTrue(p.menus.sheets.show(secondDialog, from: nil, over: owner))
        settle { !p.menus.inTransition }
        let second = try XCTUnwrap(p.menus.sheets.top as? DialogSheetController)
        flushCallbacks()
        XCTAssertTrue(first.presentedViewController === second)
        XCTAssertNotNil(first.presentingViewController)
        XCTAssertTrue(p.menus.sheets.isOpen(dialog))
        XCTAssertTrue(dialog.superview === first.scroller)
        XCTAssertFalse(dialog.isHidden)
        XCTAssertTrue(first.view.accessibilityElementsHidden)
        p.menus.sheets.close(secondDialog, animated: true)
        settle { !p.menus.inTransition }
        XCTAssertTrue(p.menus.sheets.top === first)
        XCTAssertTrue(p.menus.sheets.isOpen(dialog))
        XCTAssertFalse(first.view.accessibilityElementsHidden)
        XCTAssertTrue(dialog.accessibilityViewIsModal)
    }

    func testACancelledInteractiveSheetDismissalKeepsItsEntry() throws {
        guard hasScene else { throw missing("scene", "UIKit must run and cancel an interactive dismissal") }
        let p = try page(), dialog = try XCTUnwrap(p.views[2])
        let transition = SheetDismissal(), owner = SheetPresenter(try XCTUnwrap(window?.rootViewController), transition: transition)
        XCTAssertTrue(p.menus.sheets.show(dialog, from: nil, over: owner))
        settle { !p.menus.inTransition }
        let sheet = try XCTUnwrap(p.menus.sheets.top as? DialogSheetController)
        dialog.handlers.formUnion(["close", "cancel"])
        var closed: [UInt32] = [], cancelled: [UInt32] = []
        p.onClose = { closed.append($0) }
        p.menus.sheets.onCancel = { cancelled.append($0) }
        sheet.dismiss(animated: true)
        settle { sheet.transitionCoordinator?.isInteractive == true && transition.started }
        transition.interaction.update(0.4)
        XCTAssertTrue(p.menus.sheets.isOpen(dialog))
        transition.interaction.cancel()
        settle { sheet.transitionCoordinator == nil && !p.menus.inTransition }
        flushCallbacks()
        XCTAssertNotNil(sheet.presentingViewController)
        XCTAssertTrue(p.menus.sheets.top === sheet)
        XCTAssertTrue(p.menus.sheets.isOpen(dialog))
        XCTAssertTrue(dialog.superview === sheet.scroller)
        XCTAssertFalse(dialog.isHidden)
        XCTAssertTrue(p.menus.sheets.controllers(holding: dialog).contains { $0 === sheet }, "the returned sheet may host popovers again")
        XCTAssertEqual(closed, [])
        XCTAssertEqual(cancelled, [])
        transition.interactive = false
    }

    func testClosingARouteRetiresItsNativeConfirmationAndPreservesAPageConfirmation() throws {
        guard hasScene else { throw missing("scene", "a native confirmation above a dialog and route") }
        for (authoredInRoute, withSheet) in [(true, false), (false, false), (true, true), (false, true)] {
            let p = try page(), route = try presentRoute(p, authoredInRoute: authoredInRoute)
            p.apply(wireBatch([
                ["op": "create", "id": 41, "kind": "view", "handlers": ["close"],
                 "props": ["id": "confirm", "semanticTag": "dialog", "accessibilityRole": "alertdialog", "accessibilityLabel": "Continue?"]],
                ["op": "create", "id": 42, "kind": "button", "handlers": ["press"],
                 "props": ["commandfor": "confirm", "command": "close"]],
                ["op": "create", "id": 43, "kind": "button", "props": ["commandfor": "confirm", "command": "close"]],
                ["op": "create", "id": 44, "kind": "text", "props": ["text": "Keep the change."]],
                ["op": "children", "id": 41, "ids": [44, 42, 43]],
                ["op": "children", "id": 2, "ids": [3, 10, 41]],
            ]))
            let sheet = withSheet ? try show(in: p) : nil, confirm = try XCTUnwrap(p.views[41])
            let presenting: UIViewController = sheet ?? route
            var closed: [UInt32] = [], pressed: [UInt32] = []
            p.onClose = { closed.append($0) }
            p.onPress = { pressed.append($0) }
            p.dialogCommand("showModal", "confirm")
            settle { !p.menus.inTransition && presenting.presentedViewController is UIAlertController }
            let alert = try XCTUnwrap(presenting.presentedViewController as? UIAlertController)
            XCTAssertTrue(p.menus.nativeConfirmation === confirm)
            p.modals.closeTop(animated: true)
            settle { !p.modals.inTransition && !p.menus.inTransition && route.presentingViewController == nil }
            XCTAssertNil(route.presentingViewController)
            XCTAssertNil(sheet?.presentingViewController)
            XCTAssertNil(alert.presentingViewController)
            if authoredInRoute {
                XCTAssertNil(p.menus.nativeConfirmation)
                XCTAssertFalse(p.menus.isOpen(confirm))
                XCTAssertNil(p.menus.sheets.top)
            } else {
                let replacement = try XCTUnwrap(withSheet ? p.menus.sheets.top : window?.rootViewController)
                let replacementAlert = try XCTUnwrap(replacement.presentedViewController as? UIAlertController)
                XCTAssertFalse(replacementAlert === alert)
                XCTAssertTrue(p.menus.nativeConfirmation === confirm)
                XCTAssertTrue(p.menus.isOpen(confirm))
                XCTAssertTrue(replacementAlert.presentingViewController === replacement)
                if !withSheet { XCTAssertNil(p.menus.sheets.top) }
            }
            flushCallbacks()
            XCTAssertEqual(closed, [])
            XCTAssertEqual(pressed, [], "teardown chooses no confirmation action")
            tearDown()
        }
    }

    func testClosingAPageDialogDuringRouteDismissalDoesNotReopenIt() throws {
        guard hasScene else { throw missing("scene", "a close while the route and its sheet retire") }
        let p = try page(), route = try presentRoute(p, authoredInRoute: false)
        let sheet = try show(in: p), dialog = try XCTUnwrap(p.views[2])
        dialog.handlers.insert("close")
        var closed: [UInt32] = []
        p.onClose = { closed.append($0) }
        p.modals.closeTop(animated: true)
        XCTAssertTrue(p.menus.isOpen(dialog), "relocation preserves logical visibility")
        p.dialogCommand("close", "form")
        settle { !p.modals.inTransition && !p.menus.inTransition && closed.count == 1 }
        flushCallbacks()
        XCTAssertNil(route.presentingViewController)
        XCTAssertNil(sheet.presentingViewController)
        XCTAssertNil(p.menus.sheets.top)
        XCTAssertFalse(p.menus.sheets.isOpen(dialog))
        XCTAssertEqual(closed, [2], "the author's close fires once despite viewDidDisappear")
        XCTAssertTrue(dialog.superview === p.views[1]?.container)
        XCTAssertTrue(dialog.isHidden)
    }

    func testCheckIncludesAPopoverAboveADialogAboveARouteModal() throws {
        guard hasScene else { throw missing("scene", "check must cross both UIKit presentation containers") }
        let session = ExactApp.shared.makeSession(label: "surface-check"), p = try page(session: session)
        _ = try presentRoute(p)
        let sheet = try show(in: p)
        p.menus.action("showPopover", ["choices", "opener"])
        try assertAbove(try XCTUnwrap(p.views[5]), sheet)
        let reply = Agent(session: session).check([:])
        XCTAssertNil(reply["error"])
        let items = try XCTUnwrap(reply["items"] as? [[String: Any]])
        let text = try XCTUnwrap(items.first { $0["id"] as? Int == 6 })
        XCTAssertEqual(text["text"] as? String, "Choice")
        XCTAssertNil(text["cover"], "a finger reaches this popover above both presentations")
        XCTAssertNotNil(items.first { $0["id"] as? Int == 10 }, "the dialog is walked too")
        XCTAssertEqual(items.filter { $0["id"] as? Int == 6 }.count, 1)
    }

    func testSampleCompositesALiftedPopoverWithoutARouteModal() throws {
        guard hasScene else { throw missing("scene", "sample must composite the sheet-container layer") }
        let session = ExactApp.shared.makeSession(label: "surface-sample"), p = try page(session: session)
        _ = try show(in: p)
        p.menus.action("showPopover", ["choices", "opener"])
        let pop = try XCTUnwrap(p.views[5])
        let patch = UIView(frame: CGRect(x: 20, y: 20, width: 30, height: 30))
        patch.backgroundColor = .red
        pop.addSubview(patch)
        let point = patch.convert(CGPoint(x: 15, y: 15), to: p.viewport)
        let reply = Agent(session: session).sample(["points": [[Double(point.x), Double(point.y)]]])
        let values = try XCTUnwrap(reply["values"] as? [[Double]]), pixel = try XCTUnwrap(values.first)
        XCTAssertGreaterThan(pixel[0], 0.9)
        XCTAssertLessThan(pixel[1], 0.05)
        XCTAssertLessThan(pixel[2], 0.05)
    }

    func testKeyboardRepositionsThePopoverAndRestoresItsAnchor() throws {
        guard hasScene else { throw missing("scene", "keyboard placement runs in a presented sheet container") }
        let p = try page(), sheet = try show(in: p)
        p.apply(wireBatch([
            ["op": "create", "id": 40, "kind": "input", "props": ["value": "draft"]],
            ["op": "frame", "id": 40, "x": 10, "y": 60, "w": 160, "h": 40],
            ["op": "children", "id": 5, "ids": [6, 7, 8, 40]],
        ]))
        p.menus.action("showPopover", ["choices", "opener"])
        let pop = try XCTUnwrap(p.views[5]), field = try XCTUnwrap(p.views[40])
        let original = pop.center, box = pop.convert(pop.bounds, to: nil), top = box.maxY - 30
        p.editing = field
        for edge in [top, top - 30] {
            setKeyboard(p, top: edge)
            XCTAssertLessThanOrEqual(field.convert(field.bounds, to: nil).maxY, edge + 0.5)
            XCTAssertLessThanOrEqual(pop.convert(pop.bounds, to: nil).maxY, edge + 0.5)
        }
        setKeyboard(p, top: nil)
        XCTAssertEqual(pop.center.x, original.x, accuracy: 0.5)
        XCTAssertEqual(pop.center.y, original.y, accuracy: 0.5)
    }

    func testShareFromALiftedControlPresentsOverItsSheet() throws {
        guard hasScene else { throw missing("scene", "a share sheet must be presented from the lifted control") }
        let p = try page(), sheet = try show(in: p)
        p.menus.action("showPopover", ["choices", "opener"])
        let button = try XCTUnwrap(p.views[7]), anchor = p.shareAnchor(button.id)
        XCTAssertTrue(anchor.0 === button)
        let fallback = p.shareAnchor(nil)
        XCTAssertTrue(fallback.0 === sheet.view)
        XCTAssertEqual(fallback.1.midX, sheet.view.bounds.midX, accuracy: 0.5)
        XCTAssertEqual(fallback.1.midY, sheet.view.bounds.midY, accuracy: 0.5)
        let share = try XCTUnwrap(ShareSheet.present(["A choice"], title: nil, from: button, at: button.bounds, outcome: { _ in }) as? UIActivityViewController)
        settle { share.presentingViewController != nil && !share.isBeingPresented }
        XCTAssertTrue(share.presentingViewController === sheet, "presented by \(String(describing: share.presentingViewController)), the sheet presents \(String(describing: sheet.presentedViewController))")
        share.dismiss(animated: false)
        settle { sheet.presentedViewController == nil }
    }

    func testSurfaceEnumerationIncludesSheetsAndRetiringRoutesWithoutOtherSessions() throws {
        guard hasScene else { throw missing("scene", "surface lifetimes include UIKit's dismissal interval") }
        let p = try page(), route = try presentRoute(p), sheet = try show(in: p)
        p.menus.action("showPopover", ["choices", "opener"])
        let pop = try XCTUnwrap(p.views[5]), content = try XCTUnwrap(p.views[6])
        let surfaces = p.surfaces()
        XCTAssertEqual(surfaces.map(\.kind), [.document, .route, .dialog, .popover])
        XCTAssertTrue(p.mounts(sheet.scroller))
        XCTAssertTrue(p.port(of: content) === pop.superview)
        XCTAssertTrue(p.port(of: try XCTUnwrap(p.views[10])) === sheet.view)
        let foreign = Presenter()
        try XCTUnwrap(window?.rootViewController).view.addSubview(foreign.viewport)
        XCTAssertFalse(p.mounts(foreign.viewport))
        XCTAssertFalse(surfaces.contains { $0.view === foreign.viewport })
        foreign.viewport.removeFromSuperview()
        p.menus.action("hidePopover", ["choices"])
        p.menus.sheets.close(try XCTUnwrap(p.views[2]), animated: true)
        XCTAssertFalse(p.surfaces().contains { $0.view === sheet.view })
        XCTAssertTrue(p.surfaces(includingRetiring: true).contains { $0.view === sheet.view })
        settle { !p.menus.inTransition }
        p.modals.closeTop(animated: true)
        XCTAssertFalse(p.surfaces().contains { $0.view === route.view })
        XCTAssertTrue(p.surfaces(includingRetiring: true).contains { $0.view === route.view })
        settle { !p.modals.inTransition }
        XCTAssertEqual(p.surfaces(includingRetiring: true).count, 1)
    }

    /// UIKit's sheet does not take the status bar from the screen under it
    /// (`modalPresentationCapturesStatusBarAppearance`); a popover lifted over
    /// it counts as one lifted over the page does.
    func testStatusBarTakesALiftedPopoverButNotTheDialogSheetUnderIt() throws {
        guard hasScene else { throw missing("scene", "the declarations live on a dialog sheet and a popover above it") }
        let session = ExactApp.shared.makeSession(label: "surface-status"), p = try page(session: session)
        p.apply(wireBatch([
            ["op": "props", "id": 1, "set": ["statusBarStyle": "dark-content"]],
            ["op": "props", "id": 6, "set": ["statusBarStyle": "light-content", "text": "Choice"]],
            ["op": "props", "id": 10, "set": ["statusBarStyle": "light-content", "text": "Form content"]],
        ]))
        _ = try show(in: p)
        p.menus.action("showPopover", ["choices", "opener"])
        let parent = try XCTUnwrap(p.viewport.superview)
        let exact = ExactView(session: session)
        // Exercise the attached-session ownership filter, the page in the
        // window, without a layout pass that would boot a second wire tree.
        p.session = session
        parent.addSubview(exact)
        defer { p.session = nil; exact.removeFromSuperview(); parent.addSubview(p.viewport); session.view = nil }
        XCTAssertTrue(p.viewport.isDescendant(of: exact))
        XCTAssertNotNil(try XCTUnwrap(p.views[1]).window)
        XCTAssertTrue(session.view === exact)
        XCTAssertNotNil(PopoverLayer.containing(try XCTUnwrap(p.views[6]))?.sheet)
        XCTAssertEqual(p.statusBarChoice().source, 6)
        XCTAssertEqual(p.statusBarChoice().style, .lightContent)
        p.apply(wireBatch([["op": "props", "id": 6, "set": [String: String](), "clear": ["statusBarStyle"]]]))
        XCTAssertEqual(p.statusBarChoice().source, 1, "a declaration in the dialog's sheet does not count")
        XCTAssertEqual(p.statusBarChoice().style, .darkContent)
    }

    /// A live flight's layer on a dialog's sheet, and the slot it holds open
    /// in the dialog, are the flight's, not strays `layout agree` reports; a
    /// layer no flight owns still is one.
    func testAgreementAccountsForALiveFlightsLayerAndSlot() throws {
        guard hasScene else { throw missing("scene", "the flight flies in a presented sheet") }
        let session = ExactApp.shared.makeSession(label: "flight-agreement"), p = try page(session: session)
        let route = try presentRoute(p), sheet = try show(in: p)
        p.menus.action("showPopover", ["choices", "opener"])
        let content = try XCTUnwrap(p.views[6]), formText = try XCTUnwrap(p.views[10])
        var op = BatchOp(op: .flight, nodeID: formText.id)
        op.payload["from"] = NSNumber(value: content.id)
        p.beginFlight(op)
        let flight = try XCTUnwrap(p.flights[formText.id])
        p.presentFlight(formText.id, 0.5); p.flightsBatchApplied()
        XCTAssertTrue(flight.container?.superview === sheet.view)
        XCTAssertTrue(flight.slot?.superview === p.views[2], "the slot is held open in the dialog")
        let unowned = FlightLayer(frame: route.view.bounds)
        route.view.addSubview(unowned)
        defer { unowned.removeFromSuperview() }
        // Not complete: the walk goes into the live nodes, the dialog's included.
        var kernel = KernelFrames([])
        kernel.complete = false
        let report = AgreementReport(tolerance: 1)
        p.inspectAgreement(roots: p.surfaces(includingRetiring: true).map { ($0.view, $0.label) },
                           kernel: kernel, inFlight: false, report: report) { $0.convert($0.bounds, to: p.viewport) }
        XCTAssertTrue(report.walked.contains("dialog #2"))
        let layers = report.found.filter { $0["class"] as? String == String(describing: FlightLayer.self) }
        XCTAssertEqual(layers.count, 1, "only the layer no flight owns: \(report.found)")
        XCTAssertTrue(report.found.filter { $0["class"] as? String == "UIView" && $0["under"] as? Int == 2 }.isEmpty, "\(report.found)")
        p.landFlight(flight)
    }

    func testSheetSurfaceParticipatesInMotionAndAgreement() throws {
        guard hasScene else { throw missing("scene", "inspection must include a dialog's controller view") }
        let session = ExactApp.shared.makeSession(label: "surface-inspection"), p = try page(session: session)
        let sheet = try show(in: p), text = try XCTUnwrap(p.views[10]), agent = Agent(session: session)
        settle { !agent.nativeGeometryInFlight() }
        let move = CABasicAnimation(keyPath: "position")
        move.fromValue = NSValue(cgPoint: text.layer.position)
        move.toValue = NSValue(cgPoint: CGPoint(x: text.layer.position.x + 30, y: text.layer.position.y))
        move.duration = 30
        text.layer.add(move, forKey: "surface-test")
        XCTAssertTrue(agent.nativeGeometryInFlight())
        text.layer.removeAnimation(forKey: "surface-test")
        let stray = StraySurfaceView(frame: CGRect(x: 0, y: 0, width: 20, height: 20))
        sheet.view.addSubview(stray)
        let report = AgreementReport(tolerance: 1)
        p.inspectAgreement(roots: p.surfaces(includingRetiring: true).map { ($0.view, $0.label) },
                           kernel: KernelFrames([]), inFlight: false, report: report) { $0.convert($0.bounds, to: p.viewport) }
        XCTAssertGreaterThan(report.counts["stray"] ?? 0, 0, "\(report.found)")
        XCTAssertTrue(report.walked.contains("dialog #2"))
        XCTAssertEqual(report.found.filter { $0["class"] as? String == String(describing: type(of: stray)) }.count, 1)
    }

    private func addPageConfirmation(_ p: Presenter, prompt: Bool = false) {
        p.apply(wireBatch([
            ["op": "create", "id": 41, "kind": "view", "handlers": ["close"],
             "props": ["id": "confirm", "semanticTag": "dialog", "accessibilityRole": "alertdialog", "accessibilityLabel": "Continue?"]],
            ["op": "create", "id": 42, "kind": "button", "handlers": ["press"],
             "props": ["commandfor": "confirm", "command": "close"]],
            ["op": "create", "id": 43, "kind": "button", "props": ["commandfor": "confirm", "command": "close"]],
            ["op": "create", "id": 44, "kind": "text", "props": ["text": "Keep the change."]],
            ["op": "create", "id": 45, "kind": "input", "handlers": ["input"],
             "props": ["value": "original"]],
            ["op": "children", "id": 41, "ids": prompt ? [44, 45, 42, 43] : [44, 42, 43]],
            ["op": "children", "id": 1, "ids": [2, 30, 41]],
        ]))
    }

    func testASelectedPageConfirmationActionSurvivesRouteRetirementOnlyWhileValid() throws {
        guard hasScene else { throw missing("scene", "the route retires during the selected alert's dismissal") }
        for disabled in [false, true] {
            let p = try page(), route = try presentRoute(p, authoredInRoute: false)
            addPageConfirmation(p)
            p.dialogCommand("showModal", "confirm")
            settle { !p.menus.inTransition && route.presentedViewController is UIAlertController }
            let alert = try XCTUnwrap(route.presentedViewController as? UIAlertController)
            let action = try XCTUnwrap(p.views[42])
            var pressed: [UInt32] = [], closed: [UInt32] = []
            p.onPress = { pressed.append($0) }
            p.onClose = { closed.append($0) }
            XCTAssertEqual(p.menus.activate(action), true)
            XCTAssertTrue(p.menus.choosing)
            XCTAssertTrue(alert.isBeingDismissed)
            p.modals.closeTop(animated: true)
            if disabled { p.apply(wireBatch([["op": "props", "id": 42, "set": ["disabled": "true"]]])) }
            settle { !p.modals.inTransition && !p.menus.inTransition }
            flushCallbacks()
            XCTAssertEqual(pressed, disabled ? [] : [42])
            XCTAssertEqual(closed, [41])
            XCTAssertNil(p.menus.nativeConfirmation)
            XCTAssertNil(route.presentingViewController)
            XCTAssertNil(window?.rootViewController?.presentedViewController)
            tearDown()
        }
    }

    func testClosingAPendingNativeDialogSendsCloseOnceAndDoesNotReopen() throws {
        guard hasScene else { throw missing("scene", "a native dialog is waiting for its route to leave") }
        let p = try page(), route = try presentRoute(p, authoredInRoute: false)
        addPageConfirmation(p)
        let dialog = try XCTUnwrap(p.views[41])
        p.dialogCommand("showModal", "confirm")
        settle { !p.menus.inTransition && route.presentedViewController is UIAlertController }
        let alert = try XCTUnwrap(route.presentedViewController as? UIAlertController)
        var closed: [UInt32] = [], pressed: [UInt32] = []
        p.onClose = { closed.append($0) }
        p.onPress = { pressed.append($0) }
        p.modals.closeTop(animated: true)
        XCTAssertTrue(p.modals.inTransition)
        XCTAssertTrue(p.menus.isOpen(dialog))
        p.dialogCommand("close", "confirm")
        p.dialogCommand("close", "confirm")
        XCTAssertFalse(p.menus.isOpen(dialog))
        settle { !p.modals.inTransition && !p.menus.inTransition }
        flushCallbacks()
        XCTAssertEqual(closed, [41])
        XCTAssertEqual(pressed, [])
        XCTAssertNil(alert.presentingViewController)
        XCTAssertNil(p.menus.nativeConfirmation)
        XCTAssertNil(window?.rootViewController?.presentedViewController)
    }

    func testInteractiveDialogDismissalDuringRouteRetirementKeepsTheGesturesOutcome() throws {
        guard hasScene else { throw missing("scene", "UIKit must finish or cancel a sheet swipe while its route retires") }
        for (authoredInRoute, completed) in [(false, true), (true, true), (false, false)] {
            let p = try page(), route = try presentRoute(p, authoredInRoute: authoredInRoute)
            let dialog = try XCTUnwrap(p.views[2])
            dialog.handlers.formUnion(["close", "cancel"])
            let transition = SheetDismissal(), owner = SheetPresenter(route, transition: transition)
            XCTAssertTrue(p.menus.sheets.show(dialog, from: nil, over: owner))
            settle { !p.menus.inTransition }
            let sheet = try XCTUnwrap(p.menus.sheets.top as? DialogSheetController)
            let presentation = try XCTUnwrap(sheet.presentationController)
            var closed: [UInt32] = [], cancelled: [UInt32] = []
            p.onClose = { closed.append($0) }
            p.menus.sheets.onCancel = { cancelled.append($0) }
            sheet.dismiss(animated: true)
            settle { sheet.transitionCoordinator?.isInteractive == true && transition.started }
            let coordinator = try XCTUnwrap(sheet.transitionCoordinator)
            XCTAssertTrue(coordinator.animate(alongsideTransition: nil) { context in
                // A custom programmatic transition does not send the adaptive delegate's swipe callback.
                if !context.isCancelled { p.menus.sheets.presentationControllerDidDismiss(presentation) }
            })
            transition.interaction.update(0.4)
            p.modals.closeTop(animated: true)
            XCTAssertTrue(p.modals.inTransition)
            transition.interactive = false
            if completed { transition.interaction.finish() }
            else { transition.interaction.cancel() }
            settle { !p.modals.inTransition && !p.menus.inTransition }
            flushCallbacks()
            XCTAssertNil(route.presentingViewController)
            XCTAssertNil(sheet.presentingViewController)
            XCTAssertEqual(closed, completed ? [2] : [])
            XCTAssertEqual(cancelled, completed ? [2] : [])
            if completed {
                XCTAssertFalse(p.menus.sheets.owns(dialog))
                XCTAssertFalse(p.menus.isOpen(dialog))
                XCTAssertTrue(dialog.isHidden)
                XCTAssertNil(window?.rootViewController?.presentedViewController)
            } else {
                let replacement = try XCTUnwrap(p.menus.sheets.top)
                XCTAssertFalse(replacement === sheet)
                XCTAssertTrue(replacement.presentingViewController === window?.rootViewController)
                XCTAssertTrue(p.menus.isOpen(dialog))
                XCTAssertFalse(dialog.isHidden)
            }
            tearDown()
        }
    }

    func testClosingAReplacementWhileItPresentsResumesTheRemainingSheetAndConfirmation() throws {
        guard hasScene else { throw missing("scene", "the replacement sheet must be closed during its presentation") }
        let p = try page(), route = try presentRoute(p, authoredInRoute: false)
        addPageConfirmation(p)
        p.apply(wireBatch([
            ["op": "children", "id": 30, "ids": []],
            ["op": "children", "id": 1, "ids": [2, 20, 30, 41]],
        ]))
        let dialog = try XCTUnwrap(p.views[2]), second = try XCTUnwrap(p.views[20])
        dialog.handlers.insert("close")
        let firstSheet = try show(in: p), secondSheet = try show("second", in: p)
        p.dialogCommand("showModal", "confirm")
        settle { !p.menus.inTransition && secondSheet.presentedViewController is UIAlertController }
        let alert = try XCTUnwrap(secondSheet.presentedViewController as? UIAlertController)
        var closed: [UInt32] = []
        p.onClose = { closed.append($0) }
        p.modals.closeTop(animated: true)
        settle {
            guard let top = p.menus.sheets.top else { return false }
            return top !== firstSheet && top !== secondSheet && top.isBeingPresented
        }
        let replacement = try XCTUnwrap(p.menus.sheets.top)
        XCTAssertTrue(p.menus.sheets.topDialog === dialog)
        XCTAssertTrue(replacement.isBeingPresented)
        p.dialogCommand("close", "form")
        settle { !p.modals.inTransition && !p.menus.inTransition }
        flushCallbacks()
        XCTAssertEqual(closed, [2])
        XCTAssertNil(replacement.presentingViewController)
        XCTAssertNil(route.presentingViewController)
        XCTAssertFalse(p.menus.sheets.owns(dialog))
        XCTAssertTrue(dialog.isHidden)
        let remaining = try XCTUnwrap(p.menus.sheets.top)
        XCTAssertTrue(p.menus.sheets.topDialog === second)
        XCTAssertTrue(remaining.presentingViewController === window?.rootViewController)
        let resumedAlert = try XCTUnwrap(remaining.presentedViewController as? UIAlertController)
        XCTAssertFalse(resumedAlert === alert)
        XCTAssertTrue(p.menus.nativeConfirmation === p.views[41])
        XCTAssertTrue(p.menus.isOpen(second))
    }

    func testRelocatingAPromptPreservesUncommittedTextAndSelectionIncludingEmptyText() throws {
        guard hasScene else { throw missing("scene", "the prompt must be presented again after its route leaves") }
        for text in ["unsaved draft", ""] {
            let p = try page(), route = try presentRoute(p, authoredInRoute: false)
            addPageConfirmation(p, prompt: true)
            p.dialogCommand("showModal", "confirm")
            settle { !p.menus.inTransition && route.presentedViewController is UIAlertController }
            let alert = try XCTUnwrap(route.presentedViewController as? UIAlertController)
            let field = try XCTUnwrap(alert.textFields?.first), node = try XCTUnwrap(p.views[45])
            var typed: [String] = []
            p.onInput = { _, value in typed.append(value) }
            XCTAssertTrue(field.becomeFirstResponder())
            field.text = text
            field.sendActions(for: .editingChanged) // PromptField installs a UIAction.
            XCTAssertEqual(typed, [text])
            let selection = text.isEmpty ? NSRange(location: 0, length: 0) : NSRange(location: 2, length: 4)
            XCTAssertTrue(node.showSelection(selection))
            XCTAssertEqual(node.props["value"], "original", "the app has not written the edit into props")
            p.modals.closeTop(animated: true)
            settle { !p.modals.inTransition && !p.menus.inTransition }
            let replacement = try XCTUnwrap(window?.rootViewController?.presentedViewController as? UIAlertController)
            let restored = try XCTUnwrap(replacement.textFields?.first)
            XCTAssertFalse(replacement === alert)
            XCTAssertFalse(restored === field)
            XCTAssertTrue(node.headerField === restored)
            XCTAssertEqual(restored.text, text)
            XCTAssertTrue(restored.isFirstResponder)
            XCTAssertEqual(node.liveSelection, selection)
            XCTAssertEqual(node.props["value"], "original")
            tearDown()
        }
    }

    func testRelocatingADialogRestoresFocusInItsLiftedPopover() throws {
        guard hasScene else { throw missing("scene", "the focused popover is outside the dialog's UIKit subtree") }
        let p = try page(), route = try presentRoute(p, authoredInRoute: false)
        p.apply(wireBatch([
            ["op": "create", "id": 11, "kind": "input", "props": ["value": "kept draft"]],
            ["op": "frame", "id": 11, "x": 10, "y": 60, "w": 160, "h": 40],
            ["op": "children", "id": 5, "ids": [6, 7, 8, 11]],
        ]))
        let sheet = try show(in: p)
        p.menus.action("showPopover", ["choices", "opener"])
        let pop = try XCTUnwrap(p.views[5]), node = try XCTUnwrap(p.views[11]), dialog = try XCTUnwrap(p.views[2])
        let field = try XCTUnwrap(node.field)
        try assertAbove(pop, sheet)
        XCTAssertFalse(node.isDescendant(of: dialog))
        XCTAssertTrue(p.authored(node, inside: dialog))
        XCTAssertTrue(field.becomeFirstResponder())
        XCTAssertTrue(p.focusedNode === node)
        p.modals.closeTop(animated: true)
        settle { !p.modals.inTransition && !p.menus.inTransition }
        let replacement = try XCTUnwrap(p.menus.sheets.top as? DialogSheetController)
        XCTAssertFalse(replacement === sheet)
        XCTAssertNil(route.presentingViewController)
        try assertAbove(pop, replacement)
        XCTAssertTrue(field.isFirstResponder)
        XCTAssertTrue(p.focusedNode === node)
        XCTAssertEqual(field.text, "kept draft")
    }

}

#endif
