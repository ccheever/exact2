#if os(iOS)
import UIKit
import ObjectiveC.runtime
import XCTest
@testable import ExactKit
@testable import ExactMarkdown

/// LLP 1086.000.000 D2: the agent's `reveal` counts what a person sees as in
/// view, so a form's lower action under the software keyboard is scrolled
/// above it before a tap, as a finger would scroll the form, instead of
/// being left there for the tap to refuse.
final class RevealKeyboardIOSTests: XCTestCase {
    private func session(_ label: String, widget: String?, scroller: Bool) -> (ExactSession, UIWindow) {
        let session = ExactApp.shared.makeSession(label: label)
        let p = session.presenter
        p.viewport.frame = CGRect(x: 0, y: 0, width: 400, height: 800)
        let window = UIWindow(frame: p.viewport.frame)
        window.addSubview(p.viewport)
        window.isHidden = false
        // The action (3, a view by its testId, with a child of its own) sits at
        // y 700–740: in the root page, or in a 2000-point scroller (2).
        var ops: [[String: Any]] = [["op": "create", "id": 1, "kind": "view"]]
        if let widget { ops.append(["op": "props", "id": 1, "set": ["interactiveWidget": widget]]) }
        ops += [
            ["op": "create", "id": 3, "kind": "view", "props": ["testId": "action"]],
            ["op": "create", "id": 4, "kind": "view"],
            ["op": "children", "id": 3, "ids": [4]],
        ]
        if scroller {
            ops += [
                ["op": "create", "id": 2, "kind": "view", "style": ["overflow_y": "scroll"]],
                ["op": "create", "id": 5, "kind": "view"],
                ["op": "children", "id": 5, "ids": [3]],
                ["op": "children", "id": 2, "ids": [5]],
                ["op": "children", "id": 1, "ids": [2]],
            ]
        } else {
            ops.append(["op": "children", "id": 1, "ids": [3]])
        }
        ops += [
            ["op": "roots", "ids": [1]],
            ["op": "frame", "id": 1, "x": 0.0, "y": 0.0, "w": 400.0, "h": 800.0],
            ["op": "frame", "id": 3, "x": 16.0, "y": 700.0, "w": 368.0, "h": 40.0],
            ["op": "frame", "id": 4, "x": 0.0, "y": 0.0, "w": 368.0, "h": 40.0],
        ]
        if scroller {
            ops += [
                ["op": "frame", "id": 2, "x": 0.0, "y": 0.0, "w": 400.0, "h": 800.0],
                ["op": "content", "id": 2, "w": 400.0, "h": 2000.0],
                ["op": "frame", "id": 5, "x": 0.0, "y": 0.0, "w": 400.0, "h": 2000.0],
            ]
        }
        p.apply(wireBatch(ops))
        return (session, window)
    }

    /// The action's middle, in the window.
    private func middle(_ session: ExactSession) throws -> CGFloat {
        let v = try XCTUnwrap(session.presenter.views[3])
        return v.convert(CGPoint(x: v.bounds.midX, y: v.bounds.midY), to: nil).y
    }

    func testARevealDoesNotLoseAOnePixelCorrectionAfterRounding() throws {
        let (session, window) = session("reveal-one-pixel", widget: "overlays-content", scroller: true)
        defer { window.isHidden = true; session.destroy() }
        let p = session.presenter, scroll = try XCTUnwrap(p.views[2]?.scroll)
        scroll.traitOverrides.displayScale = 2
        scroll.updateTraitsIfNeeded()
        XCTAssertEqual(scroll.traitCollection.displayScale, 2)
        // The field ends at 740; its eight-point clearance ends at 748.
        // A port ending at 747.4 therefore asks for 0.6 points, one pixel rounded.
        scroll.contentInset.bottom = 52.6
        scroll.contentOffset = .zero
        let field = try XCTUnwrap(p.views[3])
        p.reveal(field, rect: field.bounds)
        XCTAssertEqual(scroll.contentOffset.y, 0.5, accuracy: 1e-6)
    }

    func testRevealScrollsAnActionUnderTheKeyboardAboveItUnderResizesVisual() throws {
        let (session, window) = session("reveal-keyboard-visual", widget: "resizes-visual", scroller: false)
        defer { window.isHidden = true; session.destroy() }
        let p = session.presenter
        // A keyboard whose top is 340 points above the window's bottom: the
        // web's `resizes-visual`, the opt-out, makes its overlap the viewport's inset.
        setKeyboard(p, top: 460)
        XCTAssertEqual(p.viewport.contentInset.bottom, 340)
        XCTAssertEqual(try middle(session), 720, accuracy: 0.5, "under the keyboard, inside the viewport's bounds")
        let reply = Agent(session: session).reveal(["id": 3])
        XCTAssertNil(reply["error"])
        XCTAssertEqual(reply["scrolled"] as? Bool, true, "\(reply)")
        XCTAssertLessThan(try middle(session), 460, "above the keyboard: \(reply)")
        // Already in view: nothing moves.
        let again = Agent(session: session).reveal(["id": 3])
        XCTAssertEqual(again["scrolled"] as? Bool, false, "\(again)")
    }

    func testRevealUsesTheKeyboardsTopUnderOverlaysContent() throws {
        let (session, window) = session("reveal-keyboard-overlays", widget: "overlays-content", scroller: true)
        defer { window.isHidden = true; session.destroy() }
        let p = session.presenter
        setKeyboard(p, top: 460)
        XCTAssertEqual(p.viewport.contentInset.bottom, 0, "overlays-content insets nothing")
        let reply = Agent(session: session).reveal(["id": 3])
        XCTAssertEqual(reply["scrolled"] as? Bool, true, "\(reply)")
        XCTAssertLessThan(try middle(session), 460, "above the keyboard: \(reply)")
    }

    /// A target wider than its horizontal scroller, its middle off the left:
    /// centring brings the middle in, where aligning its far edge (the
    /// right, past the port) would push it further out.
    func testRevealCentresATargetWiderThanItsScroller() throws {
        let session = ExactApp.shared.makeSession(label: "reveal-wide")
        let p = session.presenter
        p.viewport.frame = CGRect(x: 0, y: 0, width: 400, height: 800)
        let window = UIWindow(frame: p.viewport.frame)
        window.addSubview(p.viewport)
        window.isHidden = false
        defer { window.isHidden = true; session.destroy() }
        p.apply(wireBatch([
            ["op": "create", "id": 1, "kind": "view"],
            ["op": "create", "id": 2, "kind": "view", "style": ["overflow_x": "scroll"]],
            ["op": "create", "id": 5, "kind": "view"],
            ["op": "create", "id": 3, "kind": "view", "props": ["testId": "action"]],
            ["op": "create", "id": 4, "kind": "view"],
            ["op": "children", "id": 3, "ids": [4]],
            ["op": "children", "id": 5, "ids": [3]],
            ["op": "children", "id": 2, "ids": [5]],
            ["op": "children", "id": 1, "ids": [2]],
            ["op": "roots", "ids": [1]],
            ["op": "frame", "id": 1, "x": 0.0, "y": 0.0, "w": 400.0, "h": 800.0],
            ["op": "frame", "id": 2, "x": 0.0, "y": 0.0, "w": 400.0, "h": 100.0],
            ["op": "content", "id": 2, "w": 2000.0, "h": 100.0],
            ["op": "frame", "id": 5, "x": 0.0, "y": 0.0, "w": 2000.0, "h": 100.0],
            ["op": "frame", "id": 3, "x": 0.0, "y": 0.0, "w": 1500.0, "h": 100.0],
            ["op": "frame", "id": 4, "x": 0.0, "y": 0.0, "w": 1500.0, "h": 100.0],
        ]))
        let scroller = try XCTUnwrap(p.views[2]?.scroll)
        scroller.contentOffset.x = 1000
        let v = try XCTUnwrap(p.views[3])
        let reply = Agent(session: session).reveal(["id": 3])
        XCTAssertEqual(reply["scrolled"] as? Bool, true, "\(reply)")
        let middle = v.convert(CGPoint(x: v.bounds.midX, y: v.bounds.midY), to: nil).x
        XCTAssertTrue((0..<400).contains(middle), "its middle is in view: \(middle), \(reply)")
    }

    /// LLP 1116.003: in view is clear of a bar over the scroller, whether
    /// UIKit insets it or not: iOS 26's bars float over the content, and a
    /// scroller UIKit does not inset runs under them, its first rows hidden.
    func testThePortEndsAtABarOverTheScroller() throws {
        let session = ExactApp.shared.makeSession(label: "reveal-bar")
        defer { session.destroy() }
        let content = UIViewController()
        let nav = UINavigationController(rootViewController: content)
        let window = UIWindow(frame: CGRect(x: 0, y: 0, width: 402, height: 874))
        window.rootViewController = nav
        window.isHidden = false
        defer { window.isHidden = true }
        let sv = UIScrollView(frame: content.view.bounds)
        sv.contentInsetAdjustmentBehavior = .never
        sv.contentSize = CGSize(width: 402, height: 3000)
        content.view.addSubview(sv)
        nav.view.layoutIfNeeded()
        let bar = nav.navigationBar.convert(nav.navigationBar.bounds, to: sv)
        XCTAssertGreaterThan(bar.maxY, 0, "the bar is over the scroller")
        XCTAssertEqual(sv.adjustedContentInset.top, 0, "and UIKit does not inset it")
        let port = Agent(session: session).visiblePort(sv)
        XCTAssertGreaterThanOrEqual(port.minY, bar.maxY - 0.5, "the port starts under the bar: \(port), \(bar)")
        XCTAssertEqual(port.maxY, sv.bounds.maxY, accuracy: 0.5, "and ends at the scroller's end")
    }

    private func liveFixture(_ label: String, policy: String, plan: String = "EXACT_FIXTURE_PLAN") throws -> (ExactSession, ExactView, UIWindow, UITextField) {
        let scene = try XCTUnwrap(UIApplication.shared.connectedScenes.first as? UIWindowScene)
        let session = ExactApp.shared.makeSession(label: label)
        let view = ExactView(session: session)
        let host = UIViewController()
        let window = UIWindow(windowScene: scene)
        window.frame = window.screen.bounds
        window.rootViewController = host
        window.makeKeyAndVisible()
        view.frame = host.view.bounds
        view.autoresizingMask = [.flexibleWidth, .flexibleHeight]
        host.view.addSubview(view)
        let path = try XCTUnwrap(ProcessInfo.processInfo.environment[plan])
        XCTAssertNil(session.boot(plan: try Data(contentsOf: URL(fileURLWithPath: path)), size: view.bounds.size).error)
        let p = session.presenter
        let root = try XCTUnwrap(p.root.subviews.first as? NodeView)
        p.apply(wireBatch([["op": "props", "id": Int(root.id), "set": ["interactiveWidget": policy]]]))
        view.fit()
        let field = UITextField(frame: CGRect(x: 20, y: 80, width: 180, height: 40))
        p.documentHost.addSubview(field)
        addTeardownBlock { window.endEditing(true); window.isHidden = true; session.destroy() }
        return (session, view, window, field)
    }

    private func until(_ description: String, _ ready: () -> Bool, file: StaticString = #filePath, line: UInt = #line) {
        let deadline = Date().addingTimeInterval(10)
        while !ready(), Date() < deadline { RunLoop.main.run(mode: .default, before: Date().addingTimeInterval(0.01)) }
        XCTAssertTrue(ready(), description, file: file, line: line)
    }

    func testKeyboardNotificationsDoNotBootAnUnmountedSession() {
        let session = ExactApp.shared.makeSession(label: "keyboard-unmounted-notification")
        defer { session.destroy() }
        let view = ExactView(session: session)
        view.bounds = CGRect(x: 0, y: 0, width: 400, height: 800)
        XCTAssertFalse(session.booted)
        NotificationCenter.default.post(name: UIResponder.keyboardWillChangeFrameNotification, object: nil,
                                        userInfo: [UIResponder.keyboardAnimationDurationUserInfoKey: 0.0])
        XCTAssertFalse(session.booted, "another session's keyboard must not boot this view")
        NotificationCenter.default.post(name: UIResponder.keyboardDidChangeFrameNotification, object: nil)
        XCTAssertFalse(session.booted, "settling the keyboard must not boot this view")
        XCTAssertTrue(view.inspectionOwned.isEmpty, "an unmounted view needs no keyboard probe")
        let guides = view.layoutGuides.filter { $0 is UIKeyboardLayoutGuide }.count
        _ = Agent(session: session).stateSections()
        XCTAssertEqual(view.layoutGuides.filter { $0 is UIKeyboardLayoutGuide }.count, guides, "observing state does not create a keyboard guide")
    }

    func testKeyboardLayoutResolvesTheGuideAfterMovingItsContainer() throws {
        guard hostedWindowScene() != nil else {
            throw missing("scene", "keyboard guide layout needs the hosted app scene")
        }
        let (session, view, _, field) = try liveFixture("keyboard-layout-order", policy: "resizes-visual")
        let p = session.presenter
        field.keyboardType = .numberPad
        XCTAssertTrue(field.becomeFirstResponder())
        until("the keyboard occupies this session") { p.keyboardInset > 0 }
        let top = try XCTUnwrap(p.keyboardTop), overlap = p.keyboardInset
        var frame = view.frame
        frame.origin.y += 17
        frame.size.height -= 17
        view.frame = frame
        p.keyboardChanged(Notification(name: UIResponder.keyboardWillChangeFrameNotification,
                                       userInfo: [UIResponder.keyboardAnimationDurationUserInfoKey: 0.0]))
        let probe = try XCTUnwrap(view.inspectionOwned.first)
        XCTAssertEqual(view.frame, frame)
        XCTAssertEqual(try XCTUnwrap(p.keyboardTop), probe.convert(.zero, to: view.window).y, accuracy: 0.5,
                       "Exact agrees with the native guide constraint during a focus handoff")
        XCTAssertEqual(try XCTUnwrap(p.keyboardTop), top, accuracy: 0.5, "the keyboard stayed in the same window position")
        XCTAssertEqual(p.keyboardInset, overlap, accuracy: 0.5, "the container's bottom stayed in the same position")
    }

    func testLiveKeyboardBatchAndDismissalUseCurrentGeometryForEveryPolicy() throws {
        guard hostedWindowScene() != nil else {
            throw missing("scene", "keyboard ownership requires the hosted app responder chain")
        }
        for policy in ["resizes-content", "resizes-visual", "overlays-content"] {
            let (session, view, window, field) = try liveFixture("keyboard-batch-\(policy)", policy: policy)
            let p = session.presenter, full = p.viewport.frame
            XCTAssertTrue(field.becomeFirstResponder())
            until("the real keyboard occupies this session") { p.hasKeyboardEditor && p.keyboardInset > 0 }
            let occupied = try XCTUnwrap(p.keyboardTop)
            // Notification payloads cannot replace guide geometry, including a stale hide
            p.keyboardChanged(Notification(name: UIResponder.keyboardWillChangeFrameNotification, userInfo: [
                UIResponder.keyboardFrameEndUserInfoKey: CGRect(x: 0, y: window.bounds.maxY, width: window.bounds.width, height: 300),
                UIResponder.keyboardAnimationDurationUserInfoKey: 0.0,
            ]))
            XCTAssertEqual(p.keyboardTop, occupied)
            let hidden = expectation(forNotification: UIResponder.keyboardDidHideNotification, object: nil)
            p.onCommand = { _, _, _ in
                XCTAssertTrue(p.applying)
                p.keyboardChanged(Notification(name: UIResponder.keyboardWillChangeFrameNotification, userInfo: [
                    UIResponder.keyboardFrameEndUserInfoKey: window.bounds,
                    UIResponder.keyboardAnimationDurationUserInfoKey: 0.0,
                ]))
                field.resignFirstResponder()
            }
            p.apply(wireBatch([["op": "command", "name": "keyboard-batch", "args": []]]))
            p.onCommand = nil
            XCTAssertFalse(p.hasKeyboardEditor)
            XCTAssertNil(p.keyboardTop, "a queued resize rereads ownership after its editor resigns")
            XCTAssertEqual(p.keyboardInset, 0)
            XCTAssertEqual(p.viewport.contentInset.bottom, 0)
            XCTAssertEqual(p.viewport.verticalScrollIndicatorInsets.bottom, 0)
            wait(for: [hidden], timeout: 10)
            XCTAssertEqual(view.keyboardLayoutGuide.layoutFrame.height, 0)
            XCTAssertEqual(p.viewport.frame, full)
            window.isHidden = true
            session.destroy()
        }
    }

    func testTextAndPasswordHandoffsFollowTheKeyboardLayoutGuide() throws {
        guard hostedWindowScene() != nil else {
            throw missing("scene", "focus handoffs need the hosted app keyboard and layout cycle")
        }
        let (session, view, window, field) = try liveFixture("keyboard-live-guide", policy: "resizes-content", plan: "EXACT_HEADLESS_PLAN")
        let p = session.presenter
        field.textContentType = .username
        let password = UITextField(frame: CGRect(x: 20, y: 140, width: 180, height: 40))
        password.isSecureTextEntry = true
        password.textContentType = .password
        p.viewport.addSubview(password)
        let shown = expectation(forNotification: UIResponder.keyboardDidShowNotification, object: nil)
        XCTAssertTrue(field.becomeFirstResponder())
        wait(for: [shown], timeout: 10)

        var frames = 0, compared = 0
        var largestGap: CGFloat = 0
        let sampler = KeyboardGuideSampler {
            frames += 1
            guard p.hasKeyboardEditor, let top = p.keyboardTop else { return }
            let guide = view.keyboardLayoutGuide.layoutFrame
            guard guide.height > 0 else { return }
            compared += 1
            let live = view.convert(CGPoint(x: 0, y: guide.minY), to: window).y
            largestGap = max(largestGap, abs(top - live))
        }
        let display = CADisplayLink(target: sampler, selector: #selector(KeyboardGuideSampler.sample))
        display.add(to: .main, forMode: .common)
        defer { display.invalidate() }
        for target in [password, field, password] {
            let before = frames
            XCTAssertTrue(target.becomeFirstResponder())
            until("display frames observe the focus handoff") { frames >= before + 12 }
        }
        XCTAssertGreaterThan(compared, 0, "display frames compare the editing session with its live guide")
        XCTAssertEqual(largestGap, 0, accuracy: 0.5, "the session follows UIKit's guide through native keyboard height changes")
        let hidden = expectation(forNotification: UIResponder.keyboardDidHideNotification, object: nil)
        XCTAssertTrue(password.resignFirstResponder())
        wait(for: [hidden], timeout: 10)
        XCTAssertEqual(p.keyboardInset, 0)
    }

    func testChangingPolicyWhileEditingClearsThePreviousViewportInset() throws {
        guard hostedWindowScene() != nil else {
            throw missing("scene", "keyboard policy changes need the hosted app responder lifecycle")
        }
        let (session, _, _, field) = try liveFixture("keyboard-policy-change", policy: "resizes-visual", plan: "EXACT_HEADLESS_PLAN")
        let p = session.presenter
        XCTAssertFalse(p.navigation.wantsWholeView, "this page has no native navigation")
        let shown = expectation(forNotification: UIResponder.keyboardDidShowNotification, object: nil)
        XCTAssertTrue(field.becomeFirstResponder())
        wait(for: [shown], timeout: 10)
        until("the visual policy insets the ordinary page") { p.keyboardInset > 0 && p.viewport.contentInset.bottom > 0 }
        let root = try XCTUnwrap(p.root.subviews.first as? NodeView)
        p.pinnedRoom.grow(p.viewport, by: 24)
        for policy in ["resizes-content", "overlays-content", "resizes-visual", "resizes-content"] {
            p.apply(wireBatch([["op": "props", "id": Int(root.id), "set": ["interactiveWidget": policy]]]))
            until("\(policy) removes only the previous policy's inset") {
                let range = policy == "resizes-visual" ? p.keyboardInset : 0
                return p.keyboardInset > 0 && abs(p.viewport.contentInset.bottom - range - 24) < 0.5
                    && abs(p.viewport.verticalScrollIndicatorInsets.bottom - range) < 0.5
            }
        }
        let hidden = expectation(forNotification: UIResponder.keyboardDidHideNotification, object: nil)
        field.resignFirstResponder()
        wait(for: [hidden], timeout: 10)
        until("dismissal leaves no inset from the earlier policy") { p.keyboardInset == 0 }
        XCTAssertEqual(p.viewport.contentInset.bottom, 0)
        XCTAssertEqual(p.viewport.verticalScrollIndicatorInsets.bottom, 0)
    }

    func testLiveKeyboardHandoffAndRemountDoNotLeaveTheOtherSessionInset() throws {
        guard hostedWindowScene() != nil else {
            throw missing("scene", "keyboard ownership requires the hosted app responder chain")
        }
        let (a, av, window, af) = try liveFixture("keyboard-owner-a", policy: "resizes-visual")
        let host = try XCTUnwrap(window.rootViewController)
        let aRoot = try XCTUnwrap(a.presenter.root.subviews.first as? NodeView)
        a.presenter.apply(wireBatch([["op": "props", "id": Int(aRoot.id), "set": ["viewportFit": "cover"]]]))
        av.fit()
        let b = ExactApp.shared.makeSession(label: "keyboard-owner-b"), bv = ExactView(session: b)
        bv.frame = av.frame
        host.view.addSubview(bv)
        let path = try XCTUnwrap(ProcessInfo.processInfo.environment["EXACT_FIXTURE_PLAN"])
        XCTAssertNil(b.boot(plan: try Data(contentsOf: URL(fileURLWithPath: path)), size: bv.bounds.size).error)
        let root = try XCTUnwrap(b.presenter.root.subviews.first as? NodeView)
        b.presenter.apply(wireBatch([["op": "props", "id": Int(root.id), "set": ["interactiveWidget": "resizes-visual"]]]))
        bv.fit()
        let bf = UITextField(frame: af.frame)
        b.presenter.viewport.addSubview(bf)
        defer { bf.resignFirstResponder(); b.destroy() }
        XCTAssertTrue(af.becomeFirstResponder())
        until("only A avoids A's keyboard") { a.presenter.keyboardInset > 0 && b.presenter.keyboardInset == 0 }
        XCTAssertGreaterThan(bv.keyboardLayoutGuide.layoutFrame.height, 0, "geometry alone cannot establish ownership")
        XCTAssertTrue(bf.becomeFirstResponder())
        until("only B avoids B's keyboard") { a.presenter.keyboardInset == 0 && b.presenter.keyboardInset > 0 }
        XCTAssertNil(a.presenter.keyboardTop)
        XCTAssertTrue(af.becomeFirstResponder())
        until("ownership returns to A") { a.presenter.keyboardInset > 0 && b.presenter.keyboardInset == 0 }
        let lastSize = a.presenter.viewportSize, lastInsets = a.presenter.insets, lastScreen = a.screenSize
        av.removeFromSuperview()
        XCTAssertNil(a.presenter.keyboardTop)
        XCTAssertEqual(a.presenter.keyboardInset, 0)
        XCTAssertEqual(a.presenter.viewportSize, lastSize, "unmounting preserves the last document layout")
        XCTAssertEqual(a.presenter.insets, lastInsets, "an absent window cannot replace the last safe area")
        XCTAssertEqual(a.screenSize, lastScreen)
        af.resignFirstResponder()
        host.view.addSubview(av)
        av.fit()
        XCTAssertNil(a.presenter.keyboardTop)
        XCTAssertEqual(a.presenter.keyboardInset, 0)
        XCTAssertTrue(af.becomeFirstResponder())
        until("a remounted session observes the keyboard") { a.presenter.keyboardInset > 0 }
        af.resignFirstResponder()
        until("the remounted session releases its inset") { a.presenter.keyboardInset == 0 }
    }

    func testSheetBlocksRepublishWhenOwnershipChangesWithoutGuideMovement() throws {
        guard hostedWindowScene() != nil else {
            throw missing("scene", "the sheet needs the hosted app presentation hierarchy")
        }
        let (session, view, window, _) = try liveFixture("keyboard-sheet-ownership", policy: "resizes-visual")
        let p = session.presenter
        if session.firstDrawMs == nil { session.firstDrawMs = ExactEnv.wall() }
        let opener = try XCTUnwrap(p.views.values.first { $0.props["testId"] == "sheet" })
        p.press(opener.id)
        until("the sheet is presented") { p.modals.coordinateView?.window != nil && !p.modals.inTransition }
        let sheet = try XCTUnwrap(p.modals.coordinateView)
        let route = try XCTUnwrap(p.views.values.first { $0.props["testId"] == "route-sheet" })
        func height() -> CGFloat? {
            guard let reply = try? JSONSerialization.jsonObject(with: Data(session.runtime.agent("{\"op\":\"frames\"}").utf8)) as? [String: Any],
                  let row = (reply["nodes"] as? [[Any]])?.first(where: { ($0[0] as? NSNumber)?.uint32Value == route.id }),
                  let height = row[5] as? NSNumber else { return nil }
            return CGFloat(height.doubleValue)
        }
        until("the full sheet block is published") { (height() ?? 0) > 100 }
        let full = try XCTUnwrap(height()), frame = view.frame
        setKeyboard(p, top: sheet.convert(CGPoint(x: 0, y: sheet.bounds.midY), to: window).y)
        until("the sheet's runtime block ends at the keyboard") { (height() ?? full) < full - 10 }
        let occupied = sheet.keyboardLayoutGuide.layoutFrame
        let fixture = try XCTUnwrap(objc_getAssociatedObject(view, &keyboardFixtureKey) as? KeyboardGuideFixture)
        let other = Presenter(), foreign = UITextField()
        other.viewport.addSubview(foreign)
        fixture.responder = foreign
        view.setNeedsLayout()
        view.layoutIfNeeded()
        until("the runtime receives the full sheet block after losing ownership") { height() == full }
        XCTAssertEqual(view.frame, frame, "the document did not resize")
        XCTAssertEqual(sheet.keyboardLayoutGuide.layoutFrame, occupied, "the guide did not change")
        XCTAssertNil(p.keyboardTop)
        XCTAssertEqual(p.keyboardInset, 0)
    }

    func testMarkdownLinkPromptKeepsLiveSessionOwnership() throws {
        guard hostedWindowScene() != nil else {
            throw missing("scene", "keyboard ownership requires the hosted app responder chain")
        }
        let (session, _, window, _) = try liveFixture("keyboard-markdown-prompt", policy: "resizes-visual")
        let p = session.presenter
        ExactMarkdown.install()
        p.apply(wireBatch([
            ["op": "create", "id": 4000001, "kind": "textarea", "props": ["markup": "markdown", "value": "A link"]],
            ["op": "frame", "id": 4000001, "x": 20.0, "y": 100.0, "w": 200.0, "h": 100.0],
        ]))
        let node = try XCTUnwrap(p.views[4000001])
        p.viewport.addSubview(node)
        let editor = try XCTUnwrap(node.textArea)
        XCTAssertTrue(editor.becomeFirstResponder())
        until("the Markdown editor has a keyboard") { p.keyboardInset > 0 }
        node.markdownEditLink()
        let host = try XCTUnwrap(window.rootViewController)
        until("the native link prompt is editing") {
            (host.presentedViewController as? UIAlertController)?.textFields?.first?.isFirstResponder == true
        }
        XCTAssertFalse(p.mounts(try XCTUnwrap(p.keyboardPrompt)), "the prompt is outside this session's view mounts")
        XCTAssertTrue(p.hasKeyboardEditor)
        until("the prompt's keyboard geometry arrives") { p.keyboardInset > 0 }
        XCTAssertGreaterThan(p.keyboardInset, 0)
        host.dismiss(animated: true)
        until("the prompt is dismissed") { host.presentedViewController == nil }
        window.endEditing(true)
        until("dismissing the prompt releases keyboard ownership") { !p.hasKeyboardEditor && p.keyboardInset == 0 }
        XCTAssertNil(p.keyboardTop)
    }

    private func spin(_ seconds: Double) { RunLoop.main.run(until: Date().addingTimeInterval(seconds)) }

    /// LLP 1116 D2: keyboard avoidance is the default, as in a hand-built
    /// app. A root that names no `interactive-widget` ends the viewport at
    /// the keyboard's top, so the routes end above the keys, a scroller's
    /// last rows and the field in it reachable there; `resizes-visual` opts
    /// out to an inset on the route's scrollport.
    func testTheDefaultEndsTheViewportAtTheKeyboardsTop() throws {
        guard hostedWindowScene() != nil else {
            throw missing("scene", "keyboard ownership requires the hosted app responder chain")
        }
        let env = ProcessInfo.processInfo.environment
        let plan = try Data(contentsOf: URL(fileURLWithPath: try XCTUnwrap(env["EXACT_FIXTURE_PLAN"], "build.mjs --test --ios compiles the fixture's plan")))
        let session = ExactApp.shared.makeSession(label: "keyboard-default")
        let view = ExactView(session: session)
        let host = UIViewController()
        let scene = UIApplication.shared.connectedScenes.first as? UIWindowScene
        let window = scene.map { UIWindow(windowScene: $0) } ?? UIWindow(frame: CGRect(x: 0, y: 0, width: 402, height: 874))
        window.frame = CGRect(x: 0, y: 0, width: 402, height: 874)
        window.rootViewController = host
        window.makeKeyAndVisible()
        host.view.addSubview(view)
        defer { session.destroy(); window.isHidden = true }
        XCTAssertNil(session.boot(plan: plan, size: CGSize(width: 402, height: 874)).error)
        view.frame = host.view.bounds
        view.layoutIfNeeded()
        spin(0.3)
        let p = session.presenter
        XCTAssertNil(p.interactiveWidget, "the fixture's root names none")
        XCTAssertEqual(p.keyboardPolicy, "resizes-content")
        XCTAssertEqual((Agent(session: session).stateSections()["keyboard"] as? [String: Any])?["policy"] as? String, "resizes-content", "the agent reports it")
        let route = try XCTUnwrap(p.views.values.first { $0.props["testId"] == "route-home" })
        let bottom = { route.convert(route.bounds, to: nil).maxY }
        XCTAssertEqual(bottom(), 874, accuracy: 0.5, "the whole window without a keyboard")
        setKeyboard(p, top: 574)
        spin(0.3)
        XCTAssertEqual(p.viewportSize, window.bounds.size, "native navigation keeps the application root full-size")
        XCTAssertEqual(p.viewport.frame, window.bounds, "the native container keeps the whole window")
        XCTAssertEqual(p.viewport.contentInset.bottom, 0, "nothing to pan")
        XCTAssertEqual(p.keyboardInset, 300, accuracy: 0.5, "the overlap, against the viewport without it")
        XCTAssertEqual(bottom(), 574, accuracy: 0.5, "the route ends above the keys")
        setKeyboard(p, top: nil)
        XCTAssertNil(p.keyboardTop)
        XCTAssertEqual(p.keyboardInset, 0)
        spin(0.3)
        XCTAssertEqual(bottom(), 874, accuracy: 0.5)
        // The opt-out: the web's inset, the layout viewport left alone.
        let root = try XCTUnwrap(p.root.subviews.first as? NodeView)
        p.apply(wireBatch([["op": "props", "id": Int(root.id), "set": ["interactiveWidget": "resizes-visual"]]]))
        XCTAssertEqual(p.keyboardPolicy, "resizes-visual")
        setKeyboard(p, top: 574)
        spin(0.3)
        let scrollport = try XCTUnwrap(p.navigation.controllers[route.id]?.contentViewport)
        XCTAssertEqual(scrollport.contentInset.bottom, 300, accuracy: 0.5, "the route's scrollport takes the overlap")
        XCTAssertEqual(p.viewport.contentInset.bottom, 0, "the outer page gains no keyboard range")
        XCTAssertEqual(bottom(), 874, accuracy: 0.5, "laid out as without a keyboard")
        setKeyboard(p, top: nil)
        for policy in ["resizes-content", "overlays-content"] {
            p.apply(wireBatch([["op": "props", "id": Int(root.id), "set": ["interactiveWidget": policy]]]))
            setKeyboard(p, top: 574)
            p.hostDocument(fixed: false)
            XCTAssertEqual(p.viewport.contentInset.bottom, 0, "\(policy) adds no scroll range when native ownership ends")
            p.navigation.fitOwner()
            XCTAssertEqual(p.viewport.contentInset.bottom, 0, "\(policy) adds no scroll range when ownership returns")
            setKeyboard(p, top: nil)
        }
    }



    func testWithoutAKeyboardAnActionInsideTheViewportStaysPut() throws {
        let (session, window) = session("reveal-keyboard-none", widget: nil, scroller: true)
        defer { window.isHidden = true; session.destroy() }
        let reply = Agent(session: session).reveal(["id": 3])
        XCTAssertEqual(reply["scrolled"] as? Bool, false, "\(reply)")
        XCTAssertEqual(try middle(session), 720, accuracy: 0.5)
    }
    func testThePageScrollerDismissesTheKeyboardInteractively() {
        let (session, window) = session("page-dismissal", widget: nil, scroller: false)
        defer { window.isHidden = true; session.destroy() }
        XCTAssertEqual(session.presenter.viewport.keyboardDismissMode, .interactive)
    }


    func testKeyboardRevealLeavesAnInteractiveDragAlone() throws {
        guard let scene = hostedWindowScene() else { throw missing("scene", "keyboard reveal needs a mounted native editor") }
        let (session, window) = session("keyboard-drag-reveal", widget: "resizes-visual", scroller: true)
        defer { window.endEditing(true); window.isHidden = true; session.destroy() }
        let p = session.presenter, node = try XCTUnwrap(p.views[3]), owner = try XCTUnwrap(p.views[2])
        window.isHidden = true
        window.windowScene = scene
        let host = UIViewController()
        window.rootViewController = host
        host.view.addSubview(p.viewport)
        let scroll = KeyboardDragScroll(frame: CGRect(x: 0, y: 0, width: 400, height: 400))
        scroll.contentInsetAdjustmentBehavior = .never
        scroll.contentSize = CGSize(width: 400, height: 2000)
        scroll.keyboardDismissMode = .interactive
        p.viewport.addSubview(scroll)
        scroll.addSubview(node)
        owner.scroll = scroll
        let field = UITextField(frame: node.bounds)
        node.addSubview(field)
        window.makeKeyAndVisible()
        XCTAssertTrue(field.becomeFirstResponder())
        setKeyboard(p, top: 460)
        for (tracking, dragging) in [(true, false), (false, true)] {
            scroll.simulatedTracking = tracking
            scroll.simulatedDragging = dragging
            scroll.contentOffset = .zero
            p.revealEditorAsTheKeysMove()
            p.revealSettledEditor()
            XCTAssertEqual(scroll.contentOffset, .zero, "tracking \(tracking), dragging \(dragging): the finger owns the offset")
        }
        scroll.simulatedTracking = false
        scroll.simulatedDragging = false
        p.revealEditorAsTheKeysMove()
        XCTAssertGreaterThan(scroll.contentOffset.y, 0, "automatic reveal resumes after the drag")
        scroll.contentOffset = .zero
        scroll.simulatedTracking = true
        scroll.keyboardDismissMode = .onDrag
        p.revealEditorAsTheKeysMove()
        XCTAssertGreaterThan(scroll.contentOffset.y, 0, "a scroller that dismisses the keyboard on drag does not stop the reveal")
    }

    func testAHostedNativeEditorIsRevealedWhenTheKeyboardMoves() throws {
        guard let scene = UIApplication.shared.connectedScenes.first as? UIWindowScene else { throw missing("scene", "UIKit's responder search needs the native editor in an app scene") }
        let (session, window) = session("native-editor-reveal", widget: "resizes-visual", scroller: true)
        defer { window.endEditing(true); window.isHidden = true; session.destroy() }
        let p = session.presenter, node = try XCTUnwrap(p.views[3])
        window.isHidden = true
        window.windowScene = scene
        let host = UIViewController()
        window.rootViewController = host
        host.view.addSubview(p.viewport)
        let field = UITextField(frame: CGRect(x: 0, y: 0, width: 180, height: 40))
        node.addSubview(field)
        window.makeKeyAndVisible()
        XCTAssertTrue(field.becomeFirstResponder())
        XCTAssertTrue(p.hasKeyboardEditor)
        setKeyboard(p, top: 460)
        XCTAssertTrue(p.editor === node)
        XCTAssertLessThanOrEqual(field.convert(field.bounds, to: window).maxY, 460)
    }

}
private final class KeyboardGuideSampler: NSObject {
    let inspect: () -> Void
    init(_ inspect: @escaping () -> Void) { self.inspect = inspect }
    @objc func sample() { inspect() }
}

private final class KeyboardInputFixture: UIView, UIKeyInput {
    var hasText: Bool { false }
    func insertText(_ text: String) {}
    func deleteBackward() {}
}

extension XCTestCase {
    /// `responder`: the editor the keys are for, if not the fixture's own.
    func setKeyboard(_ presenter: Presenter, top: CGFloat?, responder: UIView? = nil) {
        guard let window = presenter.viewport.window, let parent = presenter.viewport.superview else { return }
        if let view = presenter.session?.view {
            if objc_getAssociatedObject(view, &keyboardFixtureKey) == nil {
                let fixture = KeyboardGuideFixture(presenter)
                objc_setAssociatedObject(view, &keyboardFixtureKey, fixture, .OBJC_ASSOCIATION_RETAIN_NONATOMIC)
                addTeardownBlock { [weak view] in
                    fixture.restore()
                    if let view { objc_setAssociatedObject(view, &keyboardFixtureKey, nil, .OBJC_ASSOCIATION_RETAIN_NONATOMIC) }
                }
            }
            let fixture = objc_getAssociatedObject(view, &keyboardFixtureKey) as! KeyboardGuideFixture
            fixture.top = top
            if let responder { fixture.responder = responder }
            if top != nil { XCTAssertTrue(presenter.hasKeyboardEditor, "the fixture supplies a mounted keyboard responder") }
            presenter.refitKeyboard()
            return
        }
        // These fixtures exercise consumers without a session view or runtime layout.
        // Set their derived keyboard state as we set their viewport and node frames.
        presenter.keyboardTop = top
        let frame = parent.convert(presenter.pageFrame, to: window)
        let overlap = top.map { min(max(0, frame.maxY - max($0, frame.minY)), frame.height) } ?? 0
        if presenter.keyboardPolicy == "overlays-content" {
            presenter.keyboardInset = overlap
            presenter.keyboardToolbars.ride(overlap: overlap)
        } else {
            presenter.setKeyboardInset(overlap)
        }
        if top == nil { presenter.pinnedRoom.release() }
        presenter.navigation.primaryRoutes.forEach { $0.fitContentViewport() }
        presenter.menus.rehostPopovers()
        presenter.revealEditorAsTheKeysMove()
    }
}

private var keyboardFixtureKey: UInt8 = 0
// Supply keyboard geometry for single-session consumer tests; lifecycle and handoff tests use UIKit.
private final class KeyboardGuideFixture {
    var top: CGFloat?
    var responder: UIView?
    private let input = KeyboardInputFixture()
    private weak var presenter: Presenter?
    private let frameMethod: Method
    private let actionMethod: Method
    private let frameOriginal: IMP
    private let actionOriginal: IMP
    private var frameReplacement: IMP!
    private var actionReplacement: IMP!

    init(_ presenter: Presenter) {
        self.presenter = presenter
        (presenter.navigation.activeRoute?.contentMount ?? presenter.viewport).addSubview(input)
        frameMethod = class_getInstanceMethod(UILayoutGuide.self, #selector(getter: UILayoutGuide.layoutFrame))!
        actionMethod = class_getInstanceMethod(UIApplication.self, #selector(UIApplication.sendAction(_:to:from:for:)))!
        frameOriginal = method_getImplementation(frameMethod)
        actionOriginal = method_getImplementation(actionMethod)
        let frameGetter = unsafeBitCast(frameOriginal, to: (@convention(c) (UILayoutGuide, Selector) -> CGRect).self)
        let action = unsafeBitCast(actionOriginal, to: (@convention(c) (UIApplication, Selector, Selector, AnyObject?, AnyObject?, UIEvent?) -> Bool).self)
        let frameBlock: @convention(block) (UILayoutGuide) -> CGRect = { [weak self] guide in
            if guide is UIKeyboardLayoutGuide, let self, let container = guide.owningView, let presenter = self.presenter,
               container === presenter.session?.view || container === presenter.modals.coordinateView {
                let edge = self.top.map { container.convert(CGPoint(x: 0, y: $0), from: container.window).y } ?? container.bounds.maxY
                return CGRect(x: 0, y: edge, width: container.bounds.width, height: max(0, container.bounds.maxY - edge))
            }
            return frameGetter(guide, #selector(getter: UILayoutGuide.layoutFrame))
        }
        let actionBlock: @convention(block) (UIApplication, Selector, AnyObject?, AnyObject?, UIEvent?) -> Bool = { [weak self] app, selector, target, sender, event in
            if target == nil, NSStringFromSelector(selector) == "exactReportFirstResponder", let self {
                let responder: UIView? = self.top == nil ? nil : self.responder ?? self.presenter?.editing?.selectionField ?? self.presenter?.editing?.textArea ?? self.input
                responder?.perform(selector)
                return responder != nil
            }
            return action(app, #selector(UIApplication.sendAction(_:to:from:for:)), selector, target, sender, event)
        }
        frameReplacement = imp_implementationWithBlock(frameBlock)
        actionReplacement = imp_implementationWithBlock(actionBlock)
        method_setImplementation(frameMethod, frameReplacement)
        method_setImplementation(actionMethod, actionReplacement)
    }

    func restore() {
        method_setImplementation(frameMethod, frameOriginal)
        method_setImplementation(actionMethod, actionOriginal)
        imp_removeBlock(frameReplacement)
        imp_removeBlock(actionReplacement)
        input.removeFromSuperview()
    }
}

private final class KeyboardDragScroll: ScrollView {
    var simulatedTracking = false
    var simulatedDragging = false
    override var isTracking: Bool { simulatedTracking }
    override var isDragging: Bool { simulatedDragging }
}

#endif
