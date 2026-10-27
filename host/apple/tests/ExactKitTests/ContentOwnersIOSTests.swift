#if os(iOS)
import UIKit
import XCTest
@testable import ExactKit

/// LLP 1123 D2: every route is a content owner. Its root is laid out in its
/// controller's view, its host block (LLP 1115.002 D8.2), and the runner is
/// told where each route a window shows is: a presented one as a top-layer
/// box, as a dialog is, and any other by its placement in its parent, so
/// that it keeps its place among its siblings. A sheet mounts only its own
/// navigation; the document, the session's one viewport (D1), stays in the
/// session's view at its own size.
final class ContentOwnersIOSTests: XCTestCase {
    func testAScreenKeepsItsMountAndFocusWhenABackendChanges() throws {
        guard let scene = hostedWindowScene() else { throw missing("scene", "native focus needs the hosted responder chain") }
        let session = try fixture("screen-route-backend", scene.coordinateSpace.bounds.size, plan: "EXACT_EDGE_EFFECT_PLAN")
        let p = session.presenter
        XCTAssertNil(Agent(session: session).tap(["id": Int(try node(session, "tab-mixed").id)])["error"])
        until("the mixed document settles") { p.navigation.activeRoute?.node.props["testId"] == "route-mixed" && !p.navigation.inTransition }
        let route = try XCTUnwrap(p.navigation.activeRoute)
        let content = try node(session, "document-mixed"), authored = content.style
        let mount = try XCTUnwrap(route.node.superview)
        let field = try XCTUnwrap(try node(session, "mixed-field").field)
        defer { field.resignFirstResponder() }
        XCTAssertTrue(field.becomeFirstResponder())
        let focusMount = try XCTUnwrap(route.view as? PresentationMount)
        until("the focused field enters the route's visible focus search") {
            field.isFirstResponder && focusMount.focusItems(in: focusMount.bounds).contains { ($0 as? UIView) === field }
        }
        var unscrolling = authored
        unscrolling["overflow_x"] = .string("visible")
        unscrolling["overflow_y"] = .string("visible")
        content.applyStyle(unscrolling)
        XCTAssertTrue(route.node.superview === mount)
        XCTAssertNil(route.contentViewport)
        XCTAssertTrue(field.isFirstResponder)
        content.applyStyle(authored)
        let recreated = try XCTUnwrap(content.scrollView)
        XCTAssertTrue(route.contentScrollView(for: .top) === recreated)
        XCTAssertTrue(route.node.superview === mount)
        XCTAssertNil(route.contentViewport)
        XCTAssertTrue(field.isFirstResponder)
    }

    func testAMixedDocumentRevealsAFieldOutsideItsInnerScrollerAboveTheKeyboard() throws {
        guard let scene = hostedWindowScene() else { throw missing("scene", "mixed-document keyboard reveal needs a native window") }
        try assertMixedDocumentKeyboardReveal(policy: "resizes-content", in: scene)
    }

    func testAVisualViewportRevealsAFieldInANestedDocumentScroller() throws {
        guard let scene = hostedWindowScene() else { throw missing("scene", "mixed-document keyboard reveal needs a native window") }
        try assertMixedDocumentKeyboardReveal(policy: "resizes-visual", in: scene)
    }

    private func assertMixedDocumentKeyboardReveal(policy: String, in scene: UIWindowScene) throws {
        let session = try fixture("mixed-document-keyboard-\(policy)", scene.coordinateSpace.bounds.size, plan: "EXACT_EDGE_EFFECT_PLAN")
        let p = session.presenter, root = try XCTUnwrap(p.root.subviews.first as? NodeView)
        p.apply(wireBatch([["op": "props", "id": Int(root.id), "set": ["interactiveWidget": policy]]]))
        XCTAssertNil(Agent(session: session).tap(["id": Int(try node(session, "tab-mixed").id)])["error"])
        until("the mixed document settles") { p.navigation.activeRoute?.node.props["testId"] == "route-mixed" && !p.navigation.inTransition }
        let route = try XCTUnwrap(p.navigation.activeRoute)
        let document = try XCTUnwrap(try node(session, "document-mixed").scrollView)
        if policy == "resizes-visual" {
            let viewport = try XCTUnwrap(route.contentViewport)
            XCTAssertTrue(document.isDescendant(of: viewport))
        } else {
            XCTAssertNil(route.contentViewport)
        }
        let field = try XCTUnwrap(try node(session, "mixed-field").field)
        defer { field.resignFirstResponder() }
        XCTAssertTrue(field.becomeFirstResponder())
        until("document scrolling lifts the field above the keyboard") {
            guard let top = p.keyboardTop else { return false }
            return document.contentOffset.y > 0 && field.convert(field.bounds, to: self.window).maxY <= top + 1
        }
        XCTAssertTrue(field.isFirstResponder)
    }

    func testAnArrangementKeepsAuthoredChildrenAndRestoresTheirContainment() throws {
        guard #available(iOS 27.1, *) else { throw XCTSkip("arrangements require iOS 27.1") }
        let (presenter, root, controller, window) = try arrangementFixture()
        defer { window.isHidden = true }
        let first = try XCTUnwrap(presenter.views[2])
        let second = try XCTUnwrap(presenter.views[3], "an initially flat child becomes a native pane")
        let button = try XCTUnwrap(presenter.views[4])
        window.layoutIfNeeded()
        controller.view.layoutIfNeeded()
        XCTAssertTrue(controller.parent === window.rootViewController)
        XCTAssertTrue(first.isDescendant(of: controller.view))
        XCTAssertTrue(second.superview === controller.viewController(for: .secondary)?.view, "a collapsed pane keeps its authored content")
        XCTAssertTrue(button.superview === first.container, "arranging panes keeps the existing controls")
        let blocks = presenter.arrangements.blocks
        XCTAssertEqual(blocks.count, 2)
        XCTAssertEqual(blocks[2]?.placement?.within, root.id)
        XCTAssertEqual(blocks[3]?.placement?.within, root.id)
        XCTAssertNil(blocks[2]?.placement?.contentOffset, "a pane is a fixed allocation, not a scrollport")
        XCTAssertNil(blocks[3]?.placement?.contentOffset)
        XCTAssertTrue(Presenter.firstVerticalScroller(under: root) === first, "default discovery crosses native containment")
        presenter.placeChildren(root, [2, 3])
        XCTAssertTrue(first.isDescendant(of: controller.view), "a later children batch retains native containment")
        XCTAssertTrue(presenter.arrangements.set(1, controller: nil))
        XCTAssertTrue(first.superview === root.container)
        XCTAssertTrue(second.superview === root.container)
        XCTAssertTrue(button.superview === first.container)
        XCTAssertNil(controller.parent)
        XCTAssertTrue(presenter.arrangements.blocks.isEmpty)
    }

    func testAnArrangementMovesUnderTheControllerThatHoldsItsNode() throws {
        guard #available(iOS 27.1, *) else { throw XCTSkip("arrangements require iOS 27.1") }
        let (presenter, _, controller, window) = try arrangementFixture()
        defer { window.isHidden = true }
        let root = try XCTUnwrap(window.rootViewController)
        XCTAssertTrue(controller.parent === root)
        // A route adopts the content after the arrangement is made, and its bars give it a top safe area
        let route = UIViewController()
        route.additionalSafeAreaInsets.top = 50
        root.addChild(route)
        route.view.frame = root.view.bounds
        root.view.addSubview(route.view)
        route.didMove(toParent: root)
        route.view.addSubview(presenter.viewport)
        presenter.arrangements.sync()
        XCTAssertTrue(controller.parent === route, "the arrangement follows its node into the route's controller")
        window.layoutIfNeeded()
        controller.view.layoutIfNeeded()
        let pane = try XCTUnwrap(controller.viewController(for: .primary))
        XCTAssertEqual(pane.view.safeAreaInsets.top, 50, accuracy: 0.5, "the panes take the route's safe area, as UIKit gives a child its parent's")
    }

    func testAnArrangementTracksItsAuthoredChildren() throws {
        guard #available(iOS 27.1, *) else { throw XCTSkip("arrangements require iOS 27.1") }
        let (p, root, controller, window) = try arrangementFixture()
        defer { window.isHidden = true }
        let first = try XCTUnwrap(p.views[2]), removed = try XCTUnwrap(p.views[3])
        p.placeChildren(root, [3, 2])
        p.arrangements.sync()
        XCTAssertEqual(p.arrangements.entries[root.id]?.panes.map(\.id), [3, 2], "pane order follows the authored children")
        p.apply(wireBatch([
            ["op": "create", "id": 5, "kind": "view"],
            ["op": "create", "id": 8, "kind": "view"],
            ["op": "children", "id": 5, "ids": [8]],
            ["op": "children", "id": 1, "ids": [2, 5]],
            ["op": "destroy", "id": 3],
        ]))
        XCTAssertEqual(p.arrangements.entries[root.id]?.panes.map(\.id), [2, 5], "a flat replacement becomes the secondary pane")
        XCTAssertNil(removed.superview, "a destroyed pane never rejoins the document")
        XCTAssertEqual(p.flats.inspectionLayers(under: 5).map(\.leaves), [[8]], "a promoted pane paints its existing flat children in the same batch")
        p.apply(wireBatch([
            ["op": "create", "id": 9, "kind": "view", "props": ["testId": "third-child"]],
            ["op": "children", "id": 1, "ids": [2, 5, 9]],
        ]))
        XCTAssertNil(p.arrangements.entries[root.id], "three children use the authored layout")
        XCTAssertEqual(root.container.subviews.compactMap { ($0 as? NodeView)?.id }, [2, 5, 9], "fallback preserves the entire authored order")
        p.apply(wireBatch([
            ["op": "children", "id": 1, "ids": [2, 5]],
            ["op": "destroy", "id": 9],
        ]))
        XCTAssertEqual(p.arrangements.entries[root.id]?.panes.map(\.id), [2, 5], "the accepted request resumes when two children return")
        p.apply(wireBatch([
            ["op": "create", "id": 6, "kind": "view"],
            ["op": "create", "id": 7, "kind": "view"],
            ["op": "children", "id": 6, "ids": [2]],
            ["op": "children", "id": 1, "ids": [5, 7]],
            ["op": "roots", "ids": [1, 6]],
        ]))
        XCTAssertEqual(p.arrangements.entries[root.id]?.panes.map(\.id), [5, 7])
        XCTAssertTrue(first.superview === p.views[6]?.container, "retirement leaves a reparented pane with its new owner")
        XCTAssertTrue(p.arrangements.set(root.id, controller: nil))
        XCTAssertEqual(root.container.subviews.compactMap { ($0 as? NodeView)?.id }, [5, 7])
        XCTAssertNil(controller.parent)
        XCTAssertTrue(p.arrangements.blocks.isEmpty)
    }

    func testArrangingVisibleContentKeepsItsFocusedField() throws {
        guard #available(iOS 27.1, *) else { throw XCTSkip("arrangements require iOS 27.1") }
        guard let scene = hostedWindowScene() else { throw missing("scene", "focus handoff needs the native responder chain") }
        let (p, root, controller, window) = try arrangementFixture()
        defer { window.endEditing(true); window.isHidden = true }
        window.isHidden = true
        window.windowScene = scene
        window.frame = scene.coordinateSpace.bounds
        window.makeKeyAndVisible()
        XCTAssertTrue(p.arrangements.set(root.id, controller: nil))
        let first = try XCTUnwrap(p.views[2])
        first.frame = CGRect(x: 0, y: 0, width: 400, height: 500)
        let field = UITextField(frame: CGRect(x: 16, y: 100, width: 200, height: 44))
        first.container.addSubview(field)
        XCTAssertTrue(field.becomeFirstResponder())
        XCTAssertTrue(p.arrangements.set(root.id, controller: controller))
        XCTAssertTrue(field.isFirstResponder, "a visible pane stays in its window while UIKit adopts it")
        XCTAssertTrue(p.arrangements.set(root.id, controller: nil))
        XCTAssertTrue(field.isFirstResponder, "returning to authored containment keeps the same responder")
    }

    func testArrangedPeersScrollUnderTheBarsWithTheirOwnInsets() throws {
        guard #available(iOS 27.1, *) else { throw XCTSkip("arrangements require iOS 27.1") }
        guard let scene = hostedWindowScene() else { throw missing("scene", "arranged peers use the fold-aware window scene") }
        let (session, root, first, second, route, controller) = try arrangedPeers(in: scene)
        let p = session.presenter
        let scrolls = try [first, second].map { try XCTUnwrap($0.scrollView) }
        for (pane, scroll) in zip([first, second], scrolls) {
            XCTAssertEqual(scroll.contentInsetAdjustmentBehavior, .automatic)
            XCTAssertEqual(scroll.contentInset, .zero, "UIKit owns the inset; Exact adds no second copy")
            XCTAssertGreaterThan(scroll.adjustedContentInset.top, 0)
            XCTAssertEqual(scroll.adjustedContentInset.bottom, scroll.safeAreaInsets.bottom, accuracy: 1)
            XCTAssertEqual(pane.convert(pane.bounds, to: route.view).maxY, route.view.bounds.maxY, accuracy: 1)
            XCTAssertEqual(p.navigation.covers[pane.id], .inset(top: max(pane.scrollOrigin, scroll.safeAreaInsets.top), bottom: scroll.safeAreaInsets.bottom))
        }
        let holders = try XCTUnwrap(p.arrangements.entries[root.id]).holders
        for holder in holders {
            XCTAssertEqual(laidOut(session, holder.node.id)?.height, holder.view.bounds.height, "the kernel uses the full native pane height")
            XCTAssertEqual(p.owners.told[holder.node.id]?.block, .size(holder.view.bounds.size))
            let scroll = try XCTUnwrap(holder.node.scrollView)
            XCTAssertEqual(holder.node.scrollFloor(scroll), scroll.bounds.height - max(holder.node.scrollOrigin, scroll.safeAreaInsets.top) - scroll.safeAreaInsets.bottom, accuracy: 1)
        }
        let initialRow = try node(session, "first-row-1")
        for name in ["first-row-1", "second-row-1"] {
            let row = try node(session, name)
            XCTAssertGreaterThanOrEqual(row.convert(row.bounds, to: route.view).minY, route.view.safeAreaInsets.top - 1)
        }
        XCTAssertTrue(route.contentScrollView(for: .top) === scrolls[0], "the authored navigationScroll remains the title authority")
        XCTAssertEqual(route.navigationItem.navigationBarMinimization.safeAreaAdjustment, .disabled)
        p.apply(wireBatch([["op": "props", "id": Int(second.id), "set": ["scrollTop": "1000000"]]]))
        XCTAssertEqual(scrolls[1].contentOffset.y, max(-scrolls[1].adjustedContentInset.top,
                       scrolls[1].contentSize.height + scrolls[1].adjustedContentInset.bottom - scrolls[1].bounds.height), accuracy: 1,
                       "an authored scroll reaches the end of a peer that does not own the title")
        p.apply(wireBatch([["op": "props", "id": Int(second.id), "set": ["scrollTop": "0"]]]))
        let rightRow = try node(session, "second-row-1")
        let rightY = rightRow.convert(rightRow.bounds, to: route.view).minY
        let rightInset = scrolls[1].adjustedContentInset.top
        for top in [CGFloat(200), 0, 200, 0] {
            scrolls[0].setContentOffset(CGPoint(x: 0, y: top - first.scrollTopInset(scrolls[0])), animated: false)
            self.window?.layoutIfNeeded()
            until("native scrolling settles") { !Agent(session: session).nativeInFlight() }
            XCTAssertEqual(rightRow.convert(rightRow.bounds, to: route.view).minY, rightY, accuracy: 1, "scrolling one peer leaves the other's content in place")
            XCTAssertEqual(scrolls[1].adjustedContentInset.top, rightInset, accuracy: 1)
        }
        let top = scrolls[0].contentOffset.y + first.scrollTopInset(scrolls[0])
        scrolls[1].setContentOffset(CGPoint(x: 0, y: 160 - second.scrollTopInset(scrolls[1])), animated: false)
        XCTAssertEqual(scrolls[0].contentOffset.y + first.scrollTopInset(scrolls[0]), top, accuracy: 1)
        let row = try node(session, "second-row-1")
        XCTAssertLessThan(row.convert(row.bounds, to: route.view).maxY, route.view.safeAreaInsets.top, "the peer's rows pass beneath the heading")
        let peerTop = scrolls[1].contentOffset.y + second.scrollTopInset(scrolls[1])
        scrolls[0].setContentOffset(CGPoint(x: 0, y: 160 - first.scrollTopInset(scrolls[0])), animated: false)
        self.window?.layoutIfNeeded()
        until("the native title settles after the nominated pane scrolls") { !Agent(session: session).nativeInFlight() }
        XCTAssertEqual(scrolls[1].contentOffset.y + second.scrollTopInset(scrolls[1]), peerTop, accuracy: 1)
        scrolls[0].setContentOffset(CGPoint(x: 0, y: -first.scrollTopInset(scrolls[0])), animated: false)
        self.window?.layoutIfNeeded()
        until("the pane returns to its start") { !Agent(session: session).nativeInFlight() }
        XCTAssertGreaterThanOrEqual(initialRow.convert(initialRow.bounds, to: route.view).minY, route.view.safeAreaInsets.top - 1)
        for (name, scroll) in [("pane-second", scrolls[1]), ("pane-first", scrolls[0])] {
            route.node.props["navigationScroll"] = name
            p.navigation.prepareRoutes([route], in: try XCTUnwrap(route.navigationController))
            XCTAssertTrue(route.contentScrollView(for: .top) === scroll, "the named pane actually becomes the title authority")
            XCTAssertTrue(scrolls.allSatisfy { $0.contentInsetAdjustmentBehavior == .automatic }, "changing title authority keeps both peer insets")
            XCTAssertEqual(route.navigationItem.navigationBarMinimization.safeAreaAdjustment, .disabled, "projection preserves the native header policy")
        }
        let navigation = try XCTUnwrap(route.navigationController)
        _ = Agent(session: session).tap(["id": Int(try node(session, "tab-inline").id)])
        until("the arranged route is retained outside the window") { route.view.window == nil }
        p.arrangements.sync()
        p.navigation.prepareRoutes([route], in: navigation)
        XCTAssertTrue(scrolls.allSatisfy { $0.contentInsetAdjustmentBehavior == .automatic }, "retained peers keep their native inset policy")
        XCTAssertTrue(route.collapseScroll === first, "a retained route keeps its title authority")
        _ = Agent(session: session).tap(["id": Int(try node(session, "tab-panes").id)])
        until("the arranged route returns") { route.view.window != nil && !Agent(session: session).nativeInFlight() }
        route.node.props.removeValue(forKey: "navigationScroll")
        p.navigation.prepareRoutes([route], in: navigation)
        XCTAssertTrue(p.navigation.contentScroller(of: route) === first, "an unnamed route discovers the first authored pane")
        until("the unnamed route hands its top inset to the pane") { self.window?.layoutIfNeeded(); return route.collapseScroll === first }
        let reveal = try node(session, "first-row-30")
        p.reveal(reveal)
        let revealedTop = scrolls[0].contentOffset.y
        p.arrangements.sync()
        XCTAssertEqual(scrolls[0].contentOffset.y, revealedTop, accuracy: 1, "a finished adoption does not undo an element reveal")
        let savedTop = scrolls[1].contentOffset.y + second.scrollTopInset(scrolls[1])
        controller.setViewController(nil, for: .secondary)
        controller.view.layoutIfNeeded()
        p.arrangements.sync()
        until("the native-hidden pane leaves the kernel's bottom-edge chain") {
            self.window?.layoutIfNeeded()
            return p.navigation.covers[second.id] == .whole && abs(first.bounds.height - route.view.bounds.height) < 1
        }
        controller.setViewController(holders[1], for: .secondary)
        controller.view.layoutIfNeeded()
        p.arrangements.sync()
        until("the shown peer restores its inset and position") {
            self.window?.layoutIfNeeded()
            return p.navigation.covers[second.id] != .whole && scrolls[1].contentInsetAdjustmentBehavior == .automatic
                && abs(scrolls[1].contentOffset.y + second.scrollTopInset(scrolls[1]) - savedTop) < 1
        }
        let rootBox = root.kernelContentBox, rootPadding = root.kernelPadding
        root.applyKernelContentBox(root.bounds.inset(by: UIEdgeInsets(top: 16, left: 0, bottom: 0, right: 0)), padding: [16, nil, nil, nil])
        XCTAssertTrue(p.arrangements.scrollers(in: route.node, top: true).isEmpty)
        root.applyKernelContentBox(rootBox, padding: rootPadding)
        let wrapper = try XCTUnwrap(sequence(first: root as UIView, next: { $0.superview }).dropFirst().compactMap { $0 as? NodeView }.first)
        let wrapperBox = wrapper.kernelContentBox, wrapperPadding = wrapper.kernelPadding
        wrapper.applyKernelContentBox(wrapper.bounds.inset(by: UIEdgeInsets(top: 16, left: 0, bottom: 0, right: 0)), padding: [16, nil, nil, nil])
        XCTAssertTrue(p.arrangements.scrollers(in: route.node, top: true).isEmpty, "authored spacing above the arrangement keeps its clearance")
        wrapper.applyKernelContentBox(wrapperBox, padding: wrapperPadding)
        XCTAssertEqual(p.arrangements.scrollers(in: route.node, top: true).count, 2)
        let frame = holders[1].view.frame, paneFrame = second.frame
        holders[1].view.frame = frame.insetBy(dx: 0, dy: 16)
        second.frame = holders[1].view.bounds
        XCTAssertTrue(p.arrangements.scrollers(in: route.node, top: true).isEmpty, "an interior pane cannot claim the route's top edge")
        XCTAssertTrue(p.arrangements.scrollers(in: route.node, top: false).isEmpty, "an interior pane cannot claim the route's bottom edge")
        holders[1].view.frame = frame; second.frame = paneFrame
        second.retainScrollPosition(from: scrolls[1])
        second.applyStyle(["overflow_y": .string("hidden"), "overflow_x": .string("hidden")])
        XCTAssertTrue(p.arrangements.scrollers(in: route.node, top: true).isEmpty, "a non-scrolling peer keeps the route's clearance")
        second.applyStyle(["overflow_y": .string("auto"), "overflow_x": .string("hidden")])
        let replacement = try XCTUnwrap(second.scrollView)
        XCTAssertFalse(replacement === scrolls[1], "the new backend takes the queued logical position")
        p.apply(wireBatch([]))
        until("the replacement backend restores its queued position") {
            self.window?.layoutIfNeeded()
            return abs(replacement.contentOffset.y + second.scrollTopInset(replacement) - savedTop) < 1
        }
        controller.setViewController(nil, for: .secondary)
        controller.view.layoutIfNeeded()
        p.arrangements.sync()
        until("the secondary is hidden before retiring the arrangement") { p.navigation.covers[second.id] == .whole }
        XCTAssertTrue(p.arrangements.set(root.id, controller: nil))
        until("retired panes return their insets") {
            p.navigation.coversChanged()
            return first.scrollView?.contentInsetAdjustmentBehavior == .never && replacement.contentInsetAdjustmentBehavior == .never
                && p.navigation.covers[second.id] == nil
                && abs(replacement.contentOffset.y + second.scrollTopInset(replacement) - savedTop) < 1
        }
    }

    func testArrangedPeersShareOneHeaderBand() throws {
        guard #available(iOS 27.1, *) else { throw XCTSkip("arrangements require iOS 27.1") }
        guard let scene = hostedWindowScene() else { throw missing("scene", "arranged peers use the fold-aware window scene") }
        let (session, root, first, second, _, _) = try arrangedPeers(in: scene)
        let scrolls = try [first, second].map { try XCTUnwrap($0.scrollView) }
        let container = root.container
        let band = try XCTUnwrap(container.subviews.last as? UIVisualEffectView, "one material spans both panes' top edge")
        XCTAssertEqual(band.frame, CGRect(x: 0, y: 0, width: container.bounds.width, height: scrolls.map(\.safeAreaInsets.top).max() ?? 0))
        XCTAssertGreaterThan(band.frame.height, 0)
        XCTAssertFalse(band.isUserInteractionEnabled, "touches reach the panes beneath")
        XCTAssertTrue(scrolls.allSatisfy { $0.topEdgeEffect.isHidden }, "the panes' own edge effects give way to the band")
        XCTAssertEqual(band.alpha, 0, "with nothing under the bar the band is clear")
        scrolls[1].setContentOffset(CGPoint(x: 0, y: 160 - second.scrollTopInset(scrolls[1])), animated: false)
        XCTAssertEqual(band.alpha, 1, "content under the bar in either pane shows the band")
        XCTAssertTrue(session.presenter.arrangements.set(root.id, controller: nil))
        XCTAssertNil(band.superview, "restoring the authored layout removes the band")
        XCTAssertTrue(scrolls.allSatisfy { !$0.topEdgeEffect.isHidden }, "and gives each pane its edge effect back")
    }

    /// The edge-effect fixture's Panes tab, its two peers arranged side by side beneath the route's bars.
    @available(iOS 27.1, *)
    private func arrangedPeers(in scene: UIWindowScene) throws -> (ExactSession, NodeView, NodeView, NodeView, RouteController, UIArrangementViewController) {
        let session = try fixture("arranged-edges", scene.coordinateSpace.bounds.size, plan: "EXACT_EDGE_EFFECT_PLAN")
        let p = session.presenter
        let reply = Agent(session: session).tap(["id": Int(try node(session, "tab-panes").id)])
        XCTAssertNil(reply["error"], "\(reply)")
        let root = try node(session, "arrangement")
        let first = try node(session, "pane-first"), second = try node(session, "pane-second")
        let route = try XCTUnwrap(p.navigation.primaryNavigation?.topViewController as? RouteController)
        route.navigationItem.navigationBarMinimization.safeAreaAdjustment = .disabled
        let controller = UIArrangementViewController()
        controller.updateArrangement(.split.axes(.horizontal))
        XCTAssertTrue(p.arrangements.set(root.id, controller: controller))
        until("the route adopts the named pane and lays both peers beneath its bars") {
            self.window?.layoutIfNeeded()
            return route.collapseScroll === first && route.edgeScroll === first
                && abs(first.convert(first.bounds, to: route.view).minY) < 1
                && abs(second.convert(second.bounds, to: route.view).minY) < 1
                && p.arrangements.entries[root.id]?.holders.allSatisfy {
                    abs($0.node.bounds.width - $0.view.bounds.width) < 1 && abs($0.node.bounds.height - $0.view.bounds.height) < 1
                } == true
        }
        return (session, root, first, second, route, controller)
    }

    @available(iOS 27.1, *)
    private func arrangementFixture() throws -> (Presenter, NodeView, UIArrangementViewController, UIWindow) {
        let presenter = Presenter()
        let host = UIViewController()
        let window = UIWindow(frame: CGRect(x: 0, y: 0, width: 900, height: 600))
        window.rootViewController = host
        host.view.addSubview(presenter.viewport)
        presenter.viewport.frame = window.bounds
        window.makeKeyAndVisible()
        presenter.apply(wireBatch([
            ["op": "create", "id": 1, "kind": "view", "props": ["hatch": "arrangement-test"]],
            ["op": "create", "id": 2, "kind": "view", "style": ["overflow_y": "auto"]],
            ["op": "create", "id": 3, "kind": "view"],
            ["op": "create", "id": 4, "kind": "button", "props": ["text": "Same control"]],
            ["op": "children", "id": 2, "ids": [4]],
            ["op": "children", "id": 1, "ids": [2, 3]],
            ["op": "roots", "ids": [1]],
            ["op": "frame", "id": 1, "x": 0, "y": 0, "w": 900, "h": 600],
        ]))
        let root = try XCTUnwrap(presenter.views[1])
        let controller = UIArrangementViewController()
        controller.updateArrangement(.split.axes(.horizontal))
        XCTAssertTrue(presenter.arrangements.set(1, controller: controller))
        return (presenter, root, controller, window)
    }

    /// A touch resting on `target` at `point`, as UIKit hands a view one.
    private final class SheetTouch: UITouch {
        let target: UIView, point: CGPoint
        init(_ target: UIView, at point: CGPoint) { self.target = target; self.point = point; super.init() }
        override var view: UIView? { target }
        override func location(in view: UIView?) -> CGPoint { target.convert(point, to: view) }
    }

    private var window: UIWindow?
    private var host: UIViewController?
    private var sessions: [ExactSession] = []

    override func tearDown() {
        for session in sessions { session.destroy() }
        sessions = []
        window?.isHidden = true
        window = nil
        host = nil
        super.tearDown()
    }

    private func until(_ what: @autoclosure () -> String, _ seconds: Double = 5, _ done: () -> Bool) {
        let deadline = Date().addingTimeInterval(seconds)
        while !done(), Date() < deadline { RunLoop.main.run(mode: .default, before: Date().addingTimeInterval(0.02)) }
        XCTAssertTrue(done(), what())
    }

    /// A fixture booted in a window of the test host's scene, `size` large:
    /// UIKit presents a sheet only from a window a scene holds. The native
    /// fixture unless `plan` names another of the plans the test run builds.
    private func fixture(_ label: String, _ size: CGSize, plan key: String = "EXACT_FIXTURE_PLAN") throws -> ExactSession {
        let env = ProcessInfo.processInfo.environment
        let plan = try Data(contentsOf: URL(fileURLWithPath: try XCTUnwrap(env[key], "build.mjs --test --ios compiles \(key)")))
        let session = ExactApp.shared.makeSession(label: label)
        sessions.append(session)
        let view = ExactView(session: session)
        let host = UIViewController()
        let scene = UIApplication.shared.connectedScenes.first as? UIWindowScene
        let frame = CGRect(origin: .zero, size: size)
        let window = scene.map { UIWindow(windowScene: $0) } ?? UIWindow(frame: frame)
        window.frame = frame
        window.rootViewController = host
        window.makeKeyAndVisible()
        host.view.addSubview(view)
        self.window = window
        self.host = host
        XCTAssertNil(session.boot(plan: plan, size: size).error)
        view.frame = host.view.bounds
        view.layoutIfNeeded()
        until("the stack is installed") { session.presenter.navigation.primaryOwner != nil }
        // A sheet waits for the first drawn frame, which this window never draws.
        if session.firstDrawMs == nil { session.firstDrawMs = ExactEnv.wall() }
        return session
    }

    private func node(_ session: ExactSession, _ testId: String) throws -> NodeView {
        try XCTUnwrap(session.presenter.views.values.first { $0.props["testId"] == testId }, "no \(testId)")
    }

    /// A node's size as the kernel last laid it out (the agent's `frames`).
    private func laidOut(_ session: ExactSession, _ id: UInt32) -> CGSize? {
        guard let reply = try? JSONSerialization.jsonObject(with: Data(session.runtime.agent("{\"op\":\"frames\"}").utf8)) as? [String: Any],
              let row = (reply["nodes"] as? [[Any]])?.first(where: { ($0[0] as? NSNumber)?.uint32Value == id }),
              let width = row[4] as? NSNumber, let height = row[5] as? NSNumber else { return nil }
        return CGSize(width: width.doubleValue, height: height.doubleValue)
    }

    /// Where the runner is told a presented `route` is shown, if it is.
    private func shown(_ session: ExactSession, _ route: NodeView) -> CGRect? {
        session.presenter.openTopLayerFrames.first { $0.0 == route.id }?.1
    }

    /// Where the runner is told an ordinary `route` is shown in the page.
    private func placed(_ session: ExactSession, _ route: NodeView) -> BlockPlacement? {
        session.presenter.owners.told[route.id]?.placement
    }

    /// `view` in the local coordinates of the routes' owner, where an
    /// ordinary route's placement is.
    private func local(_ session: ExactSession, _ view: UIView) -> BlockPlacement? {
        let owner = session.presenter.navigation.container
        return owner.map { BlockPlacement(rect: view.convert(view.bounds, to: $0), within: $0.id, contentOffset: nil) }
    }

    /// Where a person sees `view`, in viewport points.
    private func seen(_ session: ExactSession, _ view: UIView) -> CGRect {
        let p = session.presenter
        return view.convert(view.bounds, to: p.viewport).offsetBy(dx: -p.viewport.contentOffset.x, dy: -p.viewport.contentOffset.y)
    }

    /// The sheet just presented, put at `frame`, where UIKit's transition
    /// puts a page sheet: this window finishes no presentation transition
    /// (ModalDetentIOSTests).
    private func mountSheet(at frame: CGRect = CGRect(x: 0, y: 60, width: 402, height: 814)) throws -> UIViewController {
        let controller = try XCTUnwrap(host?.presentedViewController, "the sheet is presented")
        if controller.view.window == nil { window?.addSubview(controller.view) }
        controller.view.frame = frame
        controller.view.layoutIfNeeded()
        return controller
    }

    /// The route is laid out at its controller's size and reported where
    /// its controller's view is: at the stack's root and pushed over it,
    /// each by its placement in its parent and never in the top layer, back
    /// again, and in a sheet, which is a top-layer box where the runner's
    /// modal boundary begins, as the host's `inertBehind` draws it.
    func testEachRouteIsLaidOutInItsControllersViewAndReportedWhereItIsShown() throws {
        let session = try fixture("owner-routes", CGSize(width: 402, height: 874))
        let p = session.presenter
        let home = try node(session, "route-home")
        let homeView = try XCTUnwrap(p.navigation.controllers[home.id]?.view)
        XCTAssertEqual(laidOut(session, home.id), homeView.bounds.size)
        XCTAssertEqual(placed(session, home), local(session, homeView), "the stack's root is placed where it is shown")
        XCTAssertNil(shown(session, home), "and keeps its place among its siblings")
        // Its parent's own transform is the runner's to apply, not the placement's
        let root = try XCTUnwrap(p.navigation.container)
        let before = try XCTUnwrap(placed(session, home))
        root.transform = CGAffineTransform(translationX: 0, y: 40)
        homeView.setNeedsLayout(); homeView.layoutIfNeeded()
        p.owners.publish()
        XCTAssertEqual(placed(session, home), before, "the parent's translation is not in the placement")
        root.transform = .identity

        p.press(try node(session, "detail").id)
        let detail = try node(session, "route-detail")
        until("the pushed route is placed") { self.placed(session, detail) != nil && !p.navigation.transitioning }
        let detailView = try XCTUnwrap(p.navigation.controllers[detail.id]?.view)
        XCTAssertEqual(placed(session, detail), local(session, detailView))
        XCTAssertNil(shown(session, detail))
        XCTAssertEqual(laidOut(session, detail.id), detailView.bounds.size)
        p.press(try node(session, "back").id)
        until("back on the stack's root") { p.navigation.activeRoute?.node === home && !p.navigation.transitioning }
        XCTAssertEqual(placed(session, home), local(session, homeView))
        XCTAssertNil(p.openTopLayer.modal, "nothing is presented")

        p.press(try node(session, "sheet").id)
        let sheet = try node(session, "route-sheet")
        let controller = try mountSheet()
        let sheetView = try XCTUnwrap(p.navigation.controllers[sheet.id]?.view)
        until("the sheet's route takes its controller's size") { self.laidOut(session, sheet.id) == sheetView.bounds.size }
        XCTAssertEqual(shown(session, sheet), seen(session, sheetView), "the sheet's route is a top-layer box where it is shown")
        XCTAssertNil(placed(session, sheet), "with no placement")
        XCTAssertNil(shown(session, home), "the screen under it is not one")
        let open = p.openTopLayer
        XCTAssertEqual(open.modal, open.frames.firstIndex { $0.0 == sheet.id }, "the sheet's route begins the presentation")
        XCTAssertNotNil(open.modal)
    }

    /// A tab's route is read where UIKit shows it under a tab panel turned a
    /// quarter turn, then scaled to nothing
    /// (`scripts/fixtures/tab-owner.contract`): the tab container holds the
    /// route beside its panel, so neither reaches the card, and `frame`,
    /// both point reads and the intersection observer agree with the card's
    /// view.
    func testANativeOwnerPublishesItsIntermediatePlacement() throws {
        guard hostedWindowScene() != nil else { throw missing("scene", "native owner motion requires a scene") }
        let s = try fixture("owner-animation", CGSize(width: 402, height: 874), plan: "EXACT_TAB_OWNER_PLAN")
        let p = s.presenter, route = try node(s, "route-first")
        let view = try XCTUnwrap(p.navigation.controllers[route.id]?.viewIfLoaded)
        let owner = try XCTUnwrap(p.navigation.container)
        p.owners.publish()
        let start = try XCTUnwrap(p.owners.told[route.id]?.placement?.rect).minX
        var finished = false
        UIView.animate(withDuration: 0.8, delay: 0, options: [.curveLinear]) {
            view.transform = CGAffineTransform(translationX: 160, y: 0)
            p.owners.publish()
        } completion: { _ in finished = true }
        until("the owner publishes an intermediate position", 2) {
            guard let x = p.owners.told[route.id]?.placement?.rect.minX else { return false }
            return x > start + 30 && x < start + 120
        }
        let layer = try XCTUnwrap(view.layer.presentation()), destination = try XCTUnwrap(owner.layer.presentation())
        let shown = layer.convert(layer.bounds, to: destination)
        XCTAssertEqual(try XCTUnwrap(p.owners.told[route.id]?.placement?.rect).minX, shown.minX, accuracy: 3)
        until("the owner finishes moving", 2) { finished }
        until("the geometry clock stops", 2) { !FrameClock.shared.wants(p.nativeGeometry) }
    }

    func testATabsRouteIsReadWhereUIKitShowsItUnderATurnedPanel() throws {
        let session = try fixture("owner-turned-tab", CGSize(width: 402, height: 874), plan: "EXACT_TAB_OWNER_PLAN")
        let p = session.presenter
        let card = try node(session, "card")
        let slots = { () -> [String: Any] in
            let state = try? JSONSerialization.jsonObject(with: Data(session.runtime.agent("{\"op\":\"state\"}").utf8)) as? [String: Any]
            return state?["slots"] as? [String: Any] ?? [:]
        }
        let read = { () -> [String] in
            p.press(try self.node(session, "read").id)
            let s = slots()
            return ["frame", "hit", "all", "seen"].map { s[$0] as? String ?? "" }
        }
        // The frame's numbers, which the runner composes in floating point
        let near = { (frame: String, rect: CGRect) -> Bool in
            let n = frame.split(separator: ",").compactMap { Double($0) }
            return n.count == 4 && zip(n, [rect.minX, rect.minY, rect.width, rect.height]).allSatisfy { abs($0 - $1) < 1e-6 }
        }
        let shown = seen(session, card)
        // The route fills the window, covered by the status bar, which the
        // card's `top` starts past (`cover::apply`) as below it on the web.
        let status = try XCTUnwrap(window).safeAreaInsets.top
        XCTAssertEqual(shown, CGRect(x: 16, y: 400 + status, width: 100, height: 40), "the route fills the window")
        until("the observers have run") { slots()["beacon"] as? String == "true" && slots()["seen"] as? String != "unseen" }
        let before = try read()
        XCTAssertTrue(near(before[0], shown), before[0])
        XCTAssertEqual(before[1], "card")
        XCTAssertTrue(before[2].hasPrefix("card,"), before[2])
        XCTAssertEqual(before[3], "false:0", "the card is left of the observed half")

        p.press(try node(session, "turn").id)
        let panel = try XCTUnwrap(p.views.values.first { $0.props["id"] == "panel-first" })
        until("the panel is turned") { abs(panel.layer.transform.m12 - 1) < 1e-9 }
        until("the observers have seen the turn") { slots()["beacon"] as? String == "false" }
        XCTAssertEqual(seen(session, card), shown, "UIKit shows the card where it was")
        let after = try read()
        XCTAssertTrue(near(after[0], shown), after[0])
        XCTAssertEqual(Array(after[1...]), Array(before[1...]), "both point reads and the observer agree")

        p.press(try node(session, "flatten").id)
        until("the panel is flattened") { panel.layer.transform.m11 == 0 && panel.layer.transform.m12 == 0 && panel.layer.transform.m22 == 0 }
        until("the observers have seen it") { slots()["beacon"] as? String == "true" }
        XCTAssertEqual(seen(session, card), shown, "UIKit shows the card where it was")
        let flat = try read()
        XCTAssertTrue(near(flat[0], shown), flat[0])
        XCTAssertEqual(Array(flat[1...]), Array(before[1...]), "with no inverse needed")
    }

    /// A wide document under a narrow sheet, as on the open iPhone Duo (951
    /// points wide, a 653-point page sheet): the screen behind keeps its
    /// size and the session's viewport, the sheet's route and its content
    /// take the sheet's width, the background is inert, and a finger reaches
    /// the sheet's content where it is shown.
    func testAWideDocumentKeepsItsSizeUnderANarrowSheet() throws {
        let session = try fixture("owner-wide-document", CGSize(width: 951, height: 669))
        let p = session.presenter
        let home = try node(session, "route-home")
        let document = p.viewport.frame
        let before = try XCTUnwrap(laidOut(session, home.id))
        XCTAssertEqual(before.width, 951, "the document is the session's view")

        p.press(try node(session, "sheet").id)
        let sheet = try node(session, "route-sheet")
        let controller = try mountSheet(at: CGRect(x: 149, y: 16, width: 653, height: 653))
        let sheetView = try XCTUnwrap(p.navigation.controllers[sheet.id]?.view)
        XCTAssertEqual(sheetView.bounds.width, 653, "the sheet's route controller fills the sheet")
        until("the sheet's route takes the sheet's size") { self.laidOut(session, sheet.id) == sheetView.bounds.size }
        XCTAssertEqual(laidOut(session, try node(session, "sheet-body").id)?.width, 653, "its content takes the sheet's width")
        XCTAssertTrue(sheet.isDescendant(of: controller.view), "the sheet mounts its own route")
        XCTAssertTrue(p.viewport.isDescendant(of: try XCTUnwrap(session.view)), "the document stays in the session's view")
        XCTAssertFalse(p.viewport.isDescendant(of: controller.view))
        XCTAssertEqual(p.viewport.frame, document, "the document keeps its viewport")
        XCTAssertEqual(laidOut(session, home.id), before, "and the screen behind its size")
        XCTAssertEqual(shown(session, sheet), seen(session, sheetView), "the sheet's route is reported where it is shown")
        XCTAssertEqual(shown(session, sheet)?.minX, 149)

        XCTAssertTrue(try node(session, "detail").inert, "the screen behind is inert")
        let ask = try node(session, "sheet-ask")
        XCTAssertFalse(ask.inert)
        let reach = p.reach(of: ask)
        XCTAssertEqual(reach, seen(session, controller.view), "a finger reaches the sheet's content in the sheet")
        let middle = ask.convert(CGPoint(x: ask.bounds.midX, y: ask.bounds.midY), to: nil)
        XCTAssertTrue(window?.hitTest(middle, with: nil)?.isDescendant(of: ask) == true, "and a touch at its middle lands on it")
    }

    /// LLP 1123 D1: the document is fitted by one rule, whether the root's
    /// routes have a native primary container, never by its bars or by a
    /// presentation. A primary stack whose bar an app's `navigation` hatch
    /// turned off still takes the whole view, and a sheet whose own stack
    /// shows a bar leaves that fit as it was.
    func testTheDocumentsFitIsTheRootContainersAlone() throws {
        let session = try fixture("owner-root-fit", CGSize(width: 402, height: 874), plan: "EXACT_NAVIGATION_BARS_PLAN")
        let p = session.presenter, view = try XCTUnwrap(session.view)
        let primary = try XCTUnwrap(p.navigation.primaryNavigation)
        // As the hatch's answer leaves it (`NativeHatches.navigationHatch`)
        try XCTUnwrap(p.navigation.stacks[ObjectIdentifier(primary)]).showsBar = false
        view.fit()
        XCTAssertEqual(p.viewport.frame, view.bounds, "the primary container takes the whole view, its bar or not")
        p.press(try node(session, "start-open-sheet").id)
        let controller = try mountSheet()
        XCTAssertTrue(p.navigation.presentedNavigations.contains { p.navigation.barShows($0) }, "the sheet's stack may show its bar")
        view.fit()
        XCTAssertEqual(p.viewport.frame, view.bounds, "the sheet leaves the document's fit alone")
    }

    /// Reports owed while a collection's slice is in flight (`whenIdle`)
    /// are sampled when they are told, so a later size told first is not
    /// overwritten by an earlier one waiting behind the slice: the route
    /// ends laid out at its controller's size.
    func testOwedReportsAreSampledWhenTheyAreTold() throws {
        let session = try fixture("owner-owed", CGSize(width: 402, height: 874))
        let p = session.presenter
        p.press(try node(session, "sheet").id)
        let sheet = try node(session, "route-sheet")
        let controller = try mountSheet()
        let sheetView = try XCTUnwrap(p.navigation.controllers[sheet.id]?.view)
        let resize = { (height: CGFloat) in
            controller.view.frame = CGRect(x: 0, y: 874 - height, width: 402, height: height)
            controller.view.layoutIfNeeded()
            sheetView.layoutIfNeeded()
        }
        resize(814)
        until("the sheet's route takes its size") { self.laidOut(session, sheet.id) == sheetView.bounds.size }
        // A slice in flight: what the presenter tells the runtime waits for it
        p.collections.onFill?(sheet.id, Data())
        XCTAssertTrue(session.fillInFlight)
        // First in line behind the slice, a size told at once when it runs
        session.whenIdle { resize(300) }
        // Then a size owed behind it
        resize(500)
        until("the slice lands") { !session.fillInFlight }
        XCTAssertEqual(sheetView.bounds.height, 300, accuracy: 0.5, "the size set last")
        until("the route is laid out at its controller's size") {
            self.laidOut(session, sheet.id) == sheetView.bounds.size && p.owners.told[sheet.id]?.block == .size(sheetView.bounds.size)
        }
    }

    /// Focus moving in a mounted sheet bubbles to the document's ancestors
    /// that observe it, through the route's logical parent, as keys and the
    /// clipboard do (`Presenter.eventParent`): entering, moving, blurring,
    /// and a focused field's removal with its sheet.
    func testFocusInASheetBubblesToTheDocument() throws {
        let session = try fixture("owner-focus", CGSize(width: 402, height: 874))
        let p = session.presenter
        p.press(try node(session, "sheet").id)
        let controller = try mountSheet()
        let root = try XCTUnwrap(p.navigation.container)
        root.handlers.formUnion(["focusin", "focusout"])
        var heard: [String] = []
        p.onFocusEvent = { at, kind, target, related in
            if at == root.id { heard.append("\(kind):\(target):\(related ?? "-")") }
        }
        let a = try XCTUnwrap(try node(session, "sheet-field-a").field)
        let b = try XCTUnwrap(try node(session, "sheet-field-b").field)
        XCTAssertTrue(a.becomeFirstResponder())
        XCTAssertTrue(b.becomeFirstResponder())
        _ = b.resignFirstResponder()
        until("the root hears entry, the move and the blur") { heard.count >= 4 }
        XCTAssertEqual(heard, ["focusin:sheet-a:-", "focusout:sheet-a:sheet-b", "focusin:sheet-b:sheet-a", "focusout:sheet-b:-"])
        heard.removeAll()
        XCTAssertTrue(a.becomeFirstResponder())
        p.press(try node(session, "close-sheet").id)
        until("the root hears the field leave with its sheet") { heard.last?.hasPrefix("focusout:sheet-a") == true }
    }

    /// Under `viewport-fit="cover"` the document's insets are its own, not a
    /// sheet's: a sheet's route is covered for the sheet's whole safe area
    /// (its inline bar, the home indicator of an edge-attached sheet), not
    /// for what is left of it after the document's insets, which would leave
    /// the route under its bar. A route in the document is still covered
    /// only past the insets the page pads with itself.
    func testASheetsRouteIsCoveredForItsOwnSafeArea() {
        let cover = UIEdgeInsets(top: 62, left: 0, bottom: 34, right: 0)
        let edges = { (safe: UIEdgeInsets, presented: Bool, whole: Bool, reached: CGFloat) in
            NavigationHost.routeEdges(safe: safe, document: cover, presented: presented, whole: whole, covered: !presented && !whole,
                                      collapses: false, reached: reached)
        }
        XCTAssertEqual(edges(UIEdgeInsets(top: 44, left: 0, bottom: 0, right: 0), true, true, 0),
                       .init(top: 44, right: 0, bottom: 0, left: 0), "a floating sheet's inline bar")
        XCTAssertEqual(edges(UIEdgeInsets(top: 56, left: 0, bottom: 34, right: 0), true, true, 34),
                       .init(top: 56, right: 0, bottom: 34, left: 0), "an edge-attached sheet's bar and home indicator")
        XCTAssertEqual(edges(UIEdgeInsets(top: 106, left: 0, bottom: 34, right: 0), false, false, 34),
                       .init(top: 44, right: 0, bottom: 0, left: 0), "a route in the document: its bar past the status bar the page pads for")
        // A tab's route under `viewport-fit="cover"`: the tab bar past the home indicator env() has, the bar's 49.
        XCTAssertEqual(edges(UIEdgeInsets(top: 106, left: 0, bottom: 83, right: 0), false, false, 83),
                       .init(top: 44, right: 0, bottom: 49, left: 0), "a covered page's route still clears the tab bar, which its env() does not hold")
        XCTAssertEqual(NavigationHost.routeEdges(safe: UIEdgeInsets(top: 106, left: 0, bottom: 83, right: 0), document: .zero, presented: false, whole: true,
                                                 collapses: false, reached: 83),
                       .init(top: 106, right: 0, bottom: 83, left: 0), "the whole view's route: the bar and the status bar")
    }

    /// A sheet's route ends only at a docked keyboard (LLP 1123 C26): the
    /// sheet's keyboard layout guide counts none without the keys, so a
    /// focused field with no keyboard up (a hardware keyboard, as in this
    /// test window) leaves the route its sheet's whole height. The guide
    /// resting on the bottom safe area is not keys. The keys themselves are
    /// the simulator's, checked live.
    func testASheetsRouteEndsOnlyAtTheKeys() throws {
        let session = try fixture("owner-keys", CGSize(width: 402, height: 874))
        let p = session.presenter
        p.press(try node(session, "sheet").id)
        let sheet = try node(session, "route-sheet")
        let controller = try mountSheet()
        let sheetView = try XCTUnwrap(p.navigation.controllers[sheet.id]?.view)
        until("the sheet's route takes the sheet's size") { self.laidOut(session, sheet.id) == sheetView.bounds.size }
        XCTAssertTrue(try XCTUnwrap(try node(session, "sheet-field-a").field).becomeFirstResponder())
        controller.view.setNeedsLayout()
        controller.view.layoutIfNeeded()
        p.owners.publish()
        RunLoop.main.run(until: Date().addingTimeInterval(0.5))
        XCTAssertEqual(laidOut(session, sheet.id), sheetView.bounds.size, "no keys, no end")
    }

    /// A scroller that ends a sheet's route reaches the sheet's bottom edge
    /// (LLP 1116 D2): UIKit insets its content by the sheet's bottom safe
    /// area, and nothing above it clips there, so its content shows through
    /// the inset down to the edge. A hand-built scroll view in a floating 45%
    /// sheet on iOS 27 has the same: the sheet's bottom safe area is 34 points
    /// in its own space, though it floats clear of the home indicator.
    func testASheetsEdgeScrollerReachesTheSheetsBottom() throws {
        guard let scene = UIApplication.shared.connectedScenes.first(where: { $0 is UIWindowScene }) as? UIWindowScene else {
            throw missing("scene", "a sheet is presented, with its safe area, only in a scene's window")
        }
        // The whole screen, so the window reaches the home indicator.
        let session = try fixture("owner-sheet-edge", scene.screen.bounds.size, plan: "EXACT_NAVIGATION_BARS_PLAN")
        try assertSheetScrollerReachesItsBottom(session, opener: "start-open-form", route: "route-form", scroller: "form-scroll")
    }

    /// The same under a `viewport-fit="cover"` document, whose own route's
    /// scroller keeps the cover (`reachBottom`): the document's insets are
    /// not the sheet's, so a sheet's scroller still reaches the sheet's edge.
    func testASheetsEdgeScrollerReachesItsBottomUnderACoverDocument() throws {
        guard let scene = UIApplication.shared.connectedScenes.first(where: { $0 is UIWindowScene }) as? UIWindowScene else {
            throw missing("scene", "a sheet is presented, with its safe area, only in a scene's window")
        }
        let session = try fixture("owner-cover-sheet", scene.screen.bounds.size, plan: "EXACT_COVER_SCROLL_PLAN")
        try assertSheetScrollerReachesItsBottom(session, opener: "open-picker", route: "route-picker", scroller: "picker-scroll")
    }

    private func assertSheetScrollerReachesItsBottom(_ session: ExactSession, opener: String, route routeID: String, scroller: String,
                                                     file: StaticString = #filePath, line: UInt = #line) throws {
        let p = session.presenter
        p.press(try node(session, opener).id)
        until("UIKit has presented the sheet") {
            guard let sheet = self.host?.presentedViewController else { return false }
            return sheet.view.window != nil && !sheet.isBeingPresented && sheet.transitionCoordinator == nil
        }
        let route = try node(session, routeID)
        let controller = try XCTUnwrap(p.navigation.controllers[route.id], file: file, line: line)
        let sheetView = try XCTUnwrap(controller.view, file: file, line: line)
        // Layout crosses the host ABI as f32; compare UIKit's bounds at that precision.
        until("the sheet's route takes the sheet's size: kernel \(String(describing: self.laidOut(session, route.id))), UIKit \(sheetView.bounds.size)") {
            let size = CGSize(width: CGFloat(Float(sheetView.bounds.width)), height: CGFloat(Float(sheetView.bounds.height)))
            return self.laidOut(session, route.id) == size
        }
        let form = try node(session, scroller), sv = try XCTUnwrap(form.scroll, file: file, line: line)
        XCTAssertTrue(controller.edgeScroll === form, "the scroller ends the route", file: file, line: line)
        let bottom = { form.convert(form.bounds, to: sheetView).maxY }
        until("the scroller reaches the sheet's bottom edge, inset by the sheet's bottom safe area") {
            sheetView.safeAreaInsets.bottom > 0 && abs(bottom() - sheetView.bounds.maxY) < 0.5
                && abs(sv.adjustedContentInset.bottom - sheetView.safeAreaInsets.bottom) < 0.5
        }
        XCTAssertGreaterThan(sheetView.safeAreaInsets.bottom, 0, "the sheet has a bottom safe area", file: file, line: line)
        XCTAssertEqual(bottom(), sheetView.bounds.maxY, accuracy: 0.5, "the scroller ends at the sheet's bottom edge", file: file, line: line)
        XCTAssertEqual(sv.adjustedContentInset.bottom, sheetView.safeAreaInsets.bottom, accuracy: 0.5, "UIKit's inset alone", file: file, line: line)
        var view: UIView? = sv
        while let v = view, v !== sheetView {
            if v.clipsToBounds {
                XCTAssertEqual(v.convert(v.bounds, to: sheetView).maxY, sheetView.bounds.maxY, accuracy: 0.5, "\(type(of: v)) clips at the sheet's edge",
                               file: file, line: line)
            }
            view = v.superview
        }
    }

    /// A sheet's editor is revealed once the keys have settled and the
    /// sheet's current blocks are told and laid out: a reveal waits behind a
    /// slice in flight, as the blocks do, and so scrolls in the geometry the
    /// field is shown in, never in one about to change.
    func testASheetsEditorIsRevealedAfterItsBlocksLand() throws {
        guard hostedWindowScene() != nil else {
            throw missing("scene", "keyboard ownership requires the hosted app responder chain")
        }
        let session = try fixture("owner-reveal", CGSize(width: 402, height: 874))
        let p = session.presenter
        p.press(try node(session, "sheet").id)
        let controller = try mountSheet()
        let scroller = try XCTUnwrap(try node(session, "sheet-scroll").scrollView)
        let field = try node(session, "sheet-field-c")
        until("the scroller is laid out") { scroller.bounds.height == 60 }
        XCTAssertEqual(scroller.contentOffset.y, 0)
        XCTAssertTrue(try XCTUnwrap(field.field).becomeFirstResponder())
        p.collections.onFill?(try node(session, "route-sheet").id, Data())
        XCTAssertTrue(session.fillInFlight)
        setKeyboard(p, top: 600)
        NotificationCenter.default.post(name: UIResponder.keyboardDidChangeFrameNotification, object: nil)
        if session.fillInFlight { XCTAssertEqual(scroller.contentOffset.y, 0, "the reveal waits for the slice") }
        until("the slice lands and the field is revealed") {
            !session.fillInFlight && scroller.contentOffset.y > 0 && scroller.bounds.contains(field.convert(field.bounds, to: scroller))
        }
        let shown = field.convert(field.bounds, to: scroller)
        XCTAssertTrue(scroller.bounds.contains(shown), "the field is in its scroller's view: \(shown) in \(scroller.bounds)")
    }

    /// A mounted sheet is out of its route's authored ancestry in UIKit, so
    /// what the ancestry hands down is handed to the sheet's mount: an
    /// authored `aria-hidden` above the route hides the sheet from
    /// assistive technology (WAI-ARIA tree exclusion), an authored `inert`
    /// makes it inert and ends the editing in it, and a `color-scheme` sets
    /// its appearance; each is taken back when the ancestor drops it.
    func testASheetInheritsWhatItsLogicalAncestorsHandDown() throws {
        let session = try fixture("owner-inherit", CGSize(width: 402, height: 874))
        let p = session.presenter
        p.press(try node(session, "sheet").id)
        let controller = try mountSheet()
        let root = try XCTUnwrap(p.navigation.container)
        let field = try node(session, "sheet-field-a")
        let above = { sequence(first: field as UIView, next: \.superview).dropFirst() }
        let props = { (set: [String: String], clear: [String]) in
            p.apply(wireBatch([["op": "props", "id": Int(root.id), "set": set, "clear": clear]]))
        }
        XCTAssertFalse(above().contains { $0.accessibilityElementsHidden })
        props(["accessibilityElementsHidden": "true"], [])
        XCTAssertTrue(above().contains { $0.accessibilityElementsHidden }, "aria-hidden above the route hides the sheet")
        props([:], ["accessibilityElementsHidden"])
        XCTAssertFalse(above().contains { $0.accessibilityElementsHidden })
        // Focused first, then made inert by a batch no touch caused, as a
        // timer's would be: the editor resigns and focus leaves it
        root.handlers.insert("focusout")
        var heard: [String] = []
        p.onFocusEvent = { at, kind, target, _ in
            if at == root.id { heard.append("\(kind):\(target)") }
        }
        let editor = try XCTUnwrap(field.field)
        XCTAssertTrue(editor.becomeFirstResponder())
        DispatchQueue.main.async { props(["inert": "true"], []) }
        until("the inert ancestor ends the sheet's editing") { !editor.isFirstResponder }
        until("focus leaves the field") { heard == ["focusout:sheet-a"] }
        XCTAssertTrue(field.inert, "inert above the route reaches the sheet's nodes")
        XCTAssertTrue(above().contains { !$0.isUserInteractionEnabled }, "and takes the sheet's touches")
        props([:], ["inert"])
        XCTAssertFalse(field.inert)
        XCTAssertFalse(above().contains { !$0.isUserInteractionEnabled })
        p.apply(wireBatch([["op": "style", "id": Int(root.id), "style": ["color_scheme": "dark"]]]))
        XCTAssertEqual(field.traitCollection.userInterfaceStyle, .dark, "the root's color-scheme reaches the sheet")
        p.apply(wireBatch([["op": "style", "id": Int(root.id), "style": ["color_scheme": "light"]]]))
        XCTAssertEqual(field.traitCollection.userInterfaceStyle, .light)
    }

    /// A node that leaves with an exit animation takes its subtree with it,
    /// a presented route's content included, which the route's sheet mounts
    /// outside the node's view, a route pushed in the sheet, which sits
    /// beside the sheet's first route, and a card flying in the sheet, which
    /// its flight lifts beside its route: the sheet's editor resigns, the
    /// flight lands and their nodes are released with the rest, as the exit
    /// starts and as it ends.
    func testALeavingAncestorTakesItsPresentedRouteWithIt() throws {
        let session = try fixture("owner-exit", CGSize(width: 402, height: 874))
        let p = session.presenter
        p.press(try node(session, "sheet").id)
        let controller = try mountSheet()
        let route = try node(session, "route-sheet"), field = try node(session, "sheet-field-a")
        p.press(try node(session, "sheet-detail").id)
        let detail = try node(session, "route-detail")
        until("Detail is pushed in the sheet") { p.navigation.controllers[detail.id] != nil }
        XCTAssertFalse(detail.isDescendant(of: route), "beside the sheet's first route")
        let pushed = sequence(first: [detail as UIView]) { level in
            let next = level.flatMap(\.subviews)
            return next.isEmpty ? nil : next
        }.joined().compactMap { $0 as? NodeView }.filter { p.views[$0.id] === $0 }
        XCTAssertGreaterThan(pushed.count, 1)
        // A shared-element flight in the sheet's route, as `fly` makes one
        // (FlightsIOSTests): its card and the card's child fly in a layer
        // beside the route
        let kids = route.container.subviews.compactMap { ($0 as? NodeView).map { Int($0.id) } }
        p.apply(wireBatch([
            ["op": "create", "id": 9001, "kind": "view", "props": ["testId": "leaver"]], ["op": "create", "id": 9002, "kind": "view", "props": [:]],
            ["op": "children", "id": Int(route.id), "ids": kids + [9001, 9002]],
            ["op": "frame", "id": 9001, "x": 0.0, "y": 100.0, "w": 300.0, "h": 200.0],
            ["op": "frame", "id": 9002, "x": 0.0, "y": 0.0, "w": 300.0, "h": 600.0]]))
        p.apply(wireBatch([
            ["op": "flight", "id": 9003, "from": 9001], ["op": "destroy", "id": 9001],
            ["op": "create", "id": 9003, "kind": "view", "props": ["testId": "card"]],
            ["op": "create", "id": 9004, "kind": "view", "props": ["testId": "card-child"]],
            ["op": "children", "id": 9002, "ids": [9003]], ["op": "children", "id": 9003, "ids": [9004]],
            ["op": "frame", "id": 9003, "x": 20.0, "y": 300.0, "w": 200.0, "h": 150.0],
            ["op": "frame", "id": 9004, "x": 0.0, "y": 0.0, "w": 100.0, "h": 50.0],
            ["op": "present", "id": 9003, "property": "flight", "x": 0.5, "y": 0.0]]))
        let card = try XCTUnwrap(p.views[9003]), flying = try XCTUnwrap(p.views[9004])
        XCTAssertNotNil(p.flights[9003], "the card flies")
        XCTAssertFalse(card.isDescendant(of: route), "beside its route")
        let editor = try XCTUnwrap(field.field)
        XCTAssertTrue(editor.becomeFirstResponder())
        let parent = try XCTUnwrap(p.eventParent(of: route) as? NodeView)
        p.beginExit(parent.id)
        XCTAssertNil(p.flights[9003], "its flight lands")
        XCTAssertFalse(editor.isFirstResponder, "its editor resigns")
        XCTAssertNil(p.views[route.id], "the route leaves with it")
        XCTAssertNil(p.views[field.id])
        XCTAssertEqual(pushed.filter { p.views[$0.id] != nil }.map(\.id), [], "and the route pushed in the sheet")
        XCTAssertEqual([card, flying].filter { p.views[$0.id] != nil }.map(\.id), [], "and the card that flew")
        XCTAssertTrue(p.endExit(parent.id))
        XCTAssertEqual((pushed + [route, field, card, flying]).filter { p.views[$0.id] != nil }.map(\.id), [], "none returns as the exit ends")
    }

    /// A presented route's logical ancestors hear its touches through its
    /// sheet's mount (`MountRelay`), as a DOM event bubbles to them: the root
    /// is the listener for a touch on the sheet's content while nothing in
    /// the sheet listens, and none of the mount's once a nearer node in the
    /// sheet does, whose own recognizer takes the touch.
    func testASheetsTouchesReachItsLogicalAncestors() throws {
        let session = try fixture("owner-relay", CGSize(width: 402, height: 874))
        let p = session.presenter
        p.press(try node(session, "sheet").id)
        let controller = try mountSheet()
        let relay = try XCTUnwrap(controller.view.subviews.compactMap { ($0 as? PresentationMount)?.relay }.first, "the mount carries a relay")
        let root = try XCTUnwrap(p.navigation.container)
        let body = try node(session, "sheet-body")
        let touched = try node(session, "sheet-menu")
        root.handlers.formUnion(["pointerdown", "pointerup", "contextmenu", "dblclick"])
        XCTAssertTrue(relay.listener(touched) { $0.wantsPointer } === root, "the root observes the sheet's touches")
        XCTAssertTrue(relay.listener(touched) { $0.handlers.contains("contextmenu") } === root)
        XCTAssertTrue(relay.listener(touched) { $0.handlers.contains("dblclick") } === root)
        body.handlers.formUnion(["pointerdown", "contextmenu"])
        XCTAssertNil(relay.listener(touched) { $0.wantsPointer }, "a nearer listener in the sheet takes the touch")
        XCTAssertNil(relay.listener(touched) { $0.handlers.contains("contextmenu") })
        XCTAssertTrue(relay.listener(touched) { $0.handlers.contains("dblclick") } === root)
    }

    /// What a presented route's content asks of its ancestors, it asks of
    /// its logical ones (`eventParent`), the sheet's mount being out of the
    /// document: the press it bubbles to (an agent's or VoiceOver's
    /// activation, `activationTarget`, and a touch's release), the menu its
    /// long press opens (`contextMenuSource`), and the status bar a forced
    /// `color-scheme` above it asks for. Whether the press lands is decided on
    /// the sheet's content the finger reached, as UIKit hit-tested it,
    /// wherever the root's or the route's own box is.
    func testASheetsContentReadsItsLogicalAncestors() throws {
        let session = try fixture("owner-logical", CGSize(width: 402, height: 874))
        let p = session.presenter
        p.press(try node(session, "sheet").id)
        let controller = try mountSheet()
        let root = try XCTUnwrap(p.navigation.container)
        let body = try node(session, "sheet-body")
        root.handlers.insert("press")
        let inside = body.convert(CGPoint(x: body.bounds.midX, y: 4), to: nil)
        XCTAssertTrue(body.activationTarget(at: inside) === root, "the root takes a press bubbling from the sheet")
        root.transform = CGAffineTransform(translationX: 1000, y: 0)
        XCTAssertTrue(body.activationTarget(at: inside) === root, "with the root translated off the sheet")
        var pressed: [UInt32] = []
        let deliver = p.onPress
        p.onPress = { pressed.append($0); deliver?($0) }
        let touch = SheetTouch(body, at: CGPoint(x: body.bounds.midX, y: 4))
        body.touchesBegan([touch], with: nil)
        body.touchesEnded([touch], with: nil)
        XCTAssertEqual(pressed, [root.id], "and a touch's release presses it")
        root.transform = .identity
        // Nor is the route's own box: on a child overflowing a narrow route,
        // where UIKit still delivers the touch, the press lands too
        let route = try node(session, "route-sheet")
        route.frame.size.width = 100
        let overflow = CGPoint(x: body.bounds.maxX - 20, y: 4)
        XCTAssertFalse(route.bounds.contains(body.convert(overflow, to: route)), "past the route's edge")
        XCTAssertTrue(body.activationTarget(at: body.convert(overflow, to: nil)) === root, "an activation on the overflowing child")
        let past = SheetTouch(body, at: overflow)
        body.touchesBegan([past], with: nil)
        body.touchesEnded([past], with: nil)
        XCTAssertEqual(pressed, [root.id, root.id], "and a touch's release there")
        p.onPress = deliver
        p.apply(wireBatch([["op": "props", "id": Int(root.id), "set": ["contextPopover": "a-menu"]]]))
        XCTAssertTrue(body.contextMenuSource === root, "the root's menu is the sheet content's")
        root.applyStyle(["color_scheme": "dark"])
        XCTAssertEqual(Presenter.schemeStatusBar(body), .lightContent, "the root's dark scheme asks for a light bar")
    }

    /// A route resized while main serves a callback the runtime waits on (a
    /// synchronous `native.call`), when no runtime call can run: its report
    /// waits for the callback to return, and the runner lays the route out
    /// at its new size, instead of the size being recorded as told and lost.
    func testAReportFromAServedCallbackReachesTheRunner() throws {
        let session = try fixture("owner-served", CGSize(width: 402, height: 874))
        let p = session.presenter
        p.press(try node(session, "sheet").id)
        let sheet = try node(session, "route-sheet")
        let controller = try mountSheet()
        let sheetView = try XCTUnwrap(p.navigation.controllers[sheet.id]?.view)
        until("the sheet's route takes its size") { self.laidOut(session, sheet.id) == sheetView.bounds.size }
        Owner.shared.sync {
            Owner.shared.callMain {
                controller.view.frame = CGRect(x: 0, y: 374, width: 402, height: 500)
                controller.view.layoutIfNeeded()
                sheetView.layoutIfNeeded()
            }
        }
        XCTAssertEqual(sheetView.bounds.height, 500)
        until("the runner lays the route out at its new size") { self.laidOut(session, sheet.id) == sheetView.bounds.size }
    }

    /// A route's safe area changed while main serves a callback the runtime
    /// waits on: its cover is told once the callback returns, and only then
    /// recorded as delivered.
    func testACoverFromAServedCallbackReachesTheRunner() throws {
        guard let scene = hostedWindowScene() else {
            throw missing("scene", "native safe-area changes reach the runner after a served callback")
        }
        let session = try fixture("cover-served", scene.screen.bounds.size, plan: "EXACT_PUSHED_TITLE_PLAN")
        let p = session.presenter, agent = Agent(session: session)
        p.press(try node(session, "home-open-viewport").id)
        XCTAssertEqual(agent.clock(["settle": true])["settled"] as? Bool, true)
        let nav = try XCTUnwrap(p.navigation.primaryNavigation)
        let route = try XCTUnwrap(nav.topViewController as? RouteController)
        // The route's child that contains viewport-info-open: a left cover moves it right
        var child = try node(session, "viewport-info-open").id
        while let parent = (agent.runnerNode(child)["parent"] as? NSNumber)?.uint32Value, parent != route.node.id { child = parent }
        func left() -> CGFloat? {
            guard let frame = agent.runnerNode(child)["frame"] as? [String: Any], let x = frame["x"] as? NSNumber else { return nil }
            return CGFloat(x.doubleValue)
        }
        func wantedLeft() -> CGFloat {
            if case .edges(let edges)? = p.navigation.covers[route.node.id] { return edges.left }
            return 0
        }
        let insets = route.additionalSafeAreaInsets, frame = route.view.frame
        for extra in [CGFloat(31), 0] {
            let previous = wantedLeft()
            Owner.shared.sync {
                Owner.shared.callMain {
                    route.additionalSafeAreaInsets = UIEdgeInsets(top: insets.top, left: insets.left + extra, bottom: insets.bottom, right: insets.right)
                    route.view.layoutIfNeeded()
                    XCTAssertEqual(route.view.frame, frame, "only the safe area changes, without resizing the owner")
                    XCTAssertNotEqual(wantedLeft(), previous, "the native change produces a different cover inside the served callback")
                }
            }
            until("the runner applies the sampled left cover \(wantedLeft()); the route's child starts at \(String(describing: left()))") {
                left().map { abs($0 - wantedLeft()) < 0.5 } == true
            }
            XCTAssertEqual(p.navigation.delivered[route.node.id], p.navigation.covers[route.node.id])
        }
    }

    /// A retained controller can leave the native stack and return below
    /// its top without appearing. It remains a native owner, not the list
    /// beside a Mac detail pane (runner `head::beside_a_pane`).
    func testAnOffscreenRouteReintroducedIntoItsStackRegainsItsBlock() throws {
        let session = try fixture("owner-reintroduced", CGSize(width: 402, height: 874), plan: "EXACT_NAVIGATION_BARS_PLAN")
        let p = session.presenter
        let home = try node(session, "route-start")
        let controller = try XCTUnwrap(p.navigation.controllers[home.id])
        let homeView = try XCTUnwrap(controller.viewIfLoaded)
        p.press(try node(session, "start-open-detail").id)
        let detail = try node(session, "route-detail")
        until("the detail covers home") { self.placed(session, detail) != nil && !p.navigation.transitioning && homeView.window == nil }
        // The authored temporary route is before the retained stack, so the
        // production projection really releases the old owners' blocks.
        p.press(try node(session, "detail-park-owners").id)
        let parked = try node(session, "route-parked")
        until("the temporary route replaces the native stack") { p.navigation.activeRoute?.node === parked && !p.navigation.transitioning }
        until("the departed owner gives its block back") { p.owners.told[home.id] == nil }
        XCTAssertTrue(p.navigation.controllers[home.id] === controller, "the authored route retains its controller")
        XCTAssertFalse(homeView.bounds.isEmpty)
        p.press(try node(session, "restore-owners").id)
        until("detail returns with home covered") { p.navigation.activeRoute?.node === detail && !p.navigation.transitioning }
        until("the reintroduced owner has its real block") { p.owners.told[home.id]?.block == .size(homeView.bounds.size) }
        XCTAssertNil(homeView.window, "the covered route was not shown to restore its block")
        XCTAssertNil(placed(session, home), "no offscreen placement is invented")
        let tree = try XCTUnwrap(try JSONSerialization.jsonObject(with: Data(session.agent(#"{"op":"tree"}"#).utf8)) as? [String: Any])
        let rows = try XCTUnwrap(tree["nodes"] as? [[String: Any]])
        XCTAssertEqual(rows.first { ($0["id"] as? NSNumber)?.uint32Value == home.id }?["inactive"] as? Bool, true)
        XCTAssertNil(rows.first { ($0["id"] as? NSNumber)?.uint32Value == detail.id }?["inactive"])
        XCTAssertNil(Agent(session: session).tap(["id": Int(try node(session, "detail-open-plain").id)])["error"], "the shown route remains normally reachable")
        until("the shown control navigated normally") { p.navigation.activeRoute?.node.props["testId"] == "route-plain" && !p.navigation.transitioning }
    }

    /// A runtime replaced while a report waits behind a slice in flight
    /// (the plan booted again, as a dev reload does): the old runtime's
    /// slice lands and its waiters are dropped, but what the new runtime's
    /// routes report is not, and they keep reporting as they change.
    func testAReloadDuringASliceLeavesTheRoutesReporting() throws {
        let session = try fixture("owner-reload", CGSize(width: 402, height: 874))
        let p = session.presenter
        p.press(try node(session, "sheet").id)
        let sheet = try node(session, "route-sheet")
        let controller = try mountSheet()
        let sheetView = try XCTUnwrap(p.navigation.controllers[sheet.id]?.view)
        until("the sheet's route takes its size") { self.laidOut(session, sheet.id) == sheetView.bounds.size }
        // A slice in flight, and a report waiting behind it
        p.collections.onFill?(sheet.id, Data())
        XCTAssertTrue(session.fillInFlight)
        controller.view.frame = CGRect(x: 0, y: 374, width: 402, height: 500)
        controller.view.layoutIfNeeded()
        sheetView.layoutIfNeeded()
        let before = try node(session, "route-home")
        let plan = try Data(contentsOf: URL(fileURLWithPath: try XCTUnwrap(ProcessInfo.processInfo.environment["EXACT_FIXTURE_PLAN"])))
        XCTAssertNil(session.boot(plan: plan, size: CGSize(width: 402, height: 874)).error)
        until("the plan booted again") { (try? self.node(session, "route-home")).map { $0 !== before } == true }
        until("the old slice is gone") { !session.fillInFlight }
        let home = try node(session, "route-home")
        until("the new route has its block") { p.owners.told[home.id] != nil }
        let homeView = try XCTUnwrap(p.navigation.controllers[home.id]?.view)
        homeView.frame.size.width = 380
        homeView.layoutIfNeeded()
        until("and follows its controller") { p.owners.told[home.id]?.block == .size(homeView.bounds.size) }
    }

    /// The root no longer projecting its routes (its `navigationBack` gone)
    /// takes the stack down: the routes that stay give their blocks and
    /// placements back, and are laid out in their parent again, under
    /// `viewport-fit="cover"` as under `auto`.
    func testRoutesGiveTheirBlocksBackWhenTheStackGoes() throws {
        let session = try fixture("owner-teardown", CGSize(width: 402, height: 874))
        let p = session.presenter
        let root = try XCTUnwrap(p.navigation.container)
        p.apply(wireBatch([["op": "props", "id": Int(root.id), "set": ["viewportFit": "cover"]]]))
        let home = try node(session, "route-home")
        until("the route has its block") { p.owners.told[home.id] != nil }
        p.apply(wireBatch([["op": "props", "id": Int(root.id), "set": [String: String](), "clear": ["navigationBack"]]]))
        until("the stack is down") { p.navigation.primaryOwner == nil }
        XCTAssertTrue(p.owners.told.isEmpty, "every block given back: \(p.owners.told.keys.sorted())")
        XCTAssertTrue(home.isDescendant(of: root), "the route is its parent's again")
    }
}
#endif
