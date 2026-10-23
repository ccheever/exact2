#if os(iOS)
import UIKit
import XCTest
@testable import ExactKit
@testable import ExactGroupedLists

/// Keyboard avoidance and Next-field navigation in native grouped lists
/// (LLP 1116.003 D2), including pinned controls (LLP 1116.008 D8), and the
/// caret of a text area that grows as it is typed into.
final class KeyboardFormIOSTests: XCTestCase {
    private var window: UIWindow?
    private var session: ExactSession?
    /// The keys' top: an iPhone 18 Pro's number pad, 302 points over the
    /// screen's bottom.
    private let keys: CGFloat = 572

    override func tearDown() {
        session?.destroy()
        session = nil
        window?.isHidden = true
        window = nil
        super.tearDown()
    }

    private func waitFor(_ what: String, file: StaticString = #filePath, line: UInt = #line, _ ready: () -> Bool) {
        func done() -> Bool {
            guard let p = session?.presenter, !p.navigation.inTransition, !p.modals.inTransition else { return false }
            return ready()
        }
        let deadline = Date().addingTimeInterval(10)
        while !done(), Date() < deadline { RunLoop.main.run(mode: .default, before: Date().addingTimeInterval(0.05)) }
        XCTAssertTrue(done(), what, file: file, line: line)
    }

    private func named(_ id: String) -> NodeView? { session?.presenter.views.values.first { $0.props["testId"] == id } }

    private func fieldsShown(_ fields: [NodeView], below top: CGFloat) -> Bool {
        fields.allSatisfy { v in
            v.window != nil && box(v).maxY <= keys + 0.5 && box(v).minY >= top - 0.5
                && hit(atCenterOf: v)?.isDescendant(of: v) == true
        }
    }

    private func pinnedLayoutReady(_ p: Presenter) -> Bool {
        guard let bar = named("pinned-bar"), let memo = named("pinned-memo"), let node = named("pinned-scroll"),
              let scroll = p.navigation.shownScroll(node), let tabs = p.navigation.tabController, let window else { return false }
        return bar.window != nil && memo.window != nil && abs(box(bar).maxY - box(tabs.tabBar).minY) < 0.5
            && abs(box(scroll).maxY - window.bounds.maxY) < 0.5
            && abs(scroll.adjustedContentInset.bottom - window.bounds.maxY + box(tabs.tabBar).minY) < 0.5
    }

    private func fixture(_ plan: String = "EXACT_KEYBOARD_PLAN") throws -> ExactSession {
        let path = try XCTUnwrap(ProcessInfo.processInfo.environment[plan], "build.mjs --test --ios compiles \(plan)")
        ExactGroupedLists.install()
        let session = ExactApp.shared.makeSession(label: "keyboard-form")
        self.session = session
        let view = ExactView(session: session)
        let host = UIViewController()
        let scene = UIApplication.shared.connectedScenes.first as? UIWindowScene
        let window = scene.map { UIWindow(windowScene: $0) } ?? UIWindow(frame: CGRect(x: 0, y: 0, width: 402, height: 874))
        window.frame = CGRect(x: 0, y: 0, width: 402, height: 874)
        window.rootViewController = host
        window.makeKeyAndVisible()
        host.view.addSubview(view)
        self.window = window
        XCTAssertNil(session.boot(plan: try Data(contentsOf: URL(fileURLWithPath: path)), size: CGSize(width: 402, height: 874)).error)
        view.frame = host.view.bounds
        view.layoutIfNeeded()
        waitFor("the initial field is laid out") {
            let field = self.named(plan == "EXACT_KEYBOARD_PLAN" ? "reps" : "note")
            return field?.window != nil && (field?.bounds.width ?? 0) > 0
        }
        return session
    }

    private func node(_ testId: String) throws -> NodeView {
        try XCTUnwrap(session?.presenter.views.values.first { $0.props["testId"] == testId }, "no \(testId)")
    }

    private func box(_ v: UIView) -> CGRect { v.convert(v.bounds, to: nil) }

    private func hit(atCenterOf view: UIView) -> UIView? {
        let center = CGPoint(x: view.bounds.midX, y: view.bounds.midY)
        return window?.hitTest(view.convert(center, to: window), with: nil)
    }

    private func assertReachable(_ testId: String, below bar: CGFloat, file: StaticString = #filePath, line: UInt = #line) throws {
        let v = try node(testId), r = box(v)
        XCTAssertLessThanOrEqual(r.maxY, keys + 0.5, "\(testId) above the keys: \(r)", file: file, line: line)
        XCTAssertGreaterThanOrEqual(r.minY, bar - 0.5, "\(testId) below the bar: \(r)", file: file, line: line)
        let hit = hit(atCenterOf: v)
        XCTAssertTrue(hit?.isDescendant(of: v) == true, "\(testId) takes a touch at its middle, not \(String(describing: hit))", file: file, line: line)
    }

    /// The page's own scroller takes the keyboard down with a drag that
    /// reaches it, as a scroller's unsaid dismissal does (LLP 1115.000 D9.1),
    /// and its drag counts as the keyboard's interactive drag.
    func testThePagesOwnScrollerPutsTheKeyboardAwayOnADrag() throws {
        let session = try fixture()
        XCTAssertEqual(session.presenter.viewport.keyboardDismissMode, .interactive)
        XCTAssertFalse(session.presenter.interactiveKeyboardDrag, "no drag yet")
    }

    func testAFocusedFieldAndItsAddButtonStayAboveTheKeyboardInATabsGroupedList() throws {
        let session = try fixture()
        let p = session.presenter
        let tabs = try XCTUnwrap(p.navigation.tabController, "the tabs are a UITabBarController")
        let nav = try XCTUnwrap(tabs.selectedViewController as? UINavigationController)
        let reps = try node("reps"), add = try node("add-set")
        XCTAssertFalse(p.views.values.contains { $0.field?.isFirstResponder == true }, "mounting the workout form does not summon a keyboard")
        XCTAssertEqual(reps.field?.keyboardType, .numberPad)
        XCTAssertEqual(try node("weight").field?.keyboardType, .decimalPad)
        XCTAssertGreaterThan(box(add).minY, keys, "the fixture's Add button starts where the keys will be")
        let resting = box(tabs.tabBar)
        reps.textFieldDidBeginEditing(try XCTUnwrap(reps.field))
        p.applyKeyboard(top: keys, duration: 0, curve: 0)
        let bar = box(nav.navigationBar).maxY
        waitFor("the field and its action clear the accessory") {
            abs(self.box(p.viewport).maxY - self.keys - (reps.field?.inputAccessoryView?.bounds.height ?? 0)) < 0.5
                && self.fieldsShown([reps, add], below: bar)
        }
        try assertReachable("reps", below: bar)
        try assertReachable("add-set", below: bar)
        let accessory = try XCTUnwrap(reps.field?.inputAccessoryView)
        XCTAssertEqual(box(p.viewport).maxY, keys + accessory.bounds.height, accuracy: 0.5, "content draws under the accessory")
        XCTAssertEqual(box(tabs.tabBar).minY, resting.minY, accuracy: 0.5, "the tab bar stays at the bottom, under the keys: \(box(tabs.tabBar))")

        let field = try XCTUnwrap(reps.field)
        XCTAssertTrue(field.becomeFirstResponder(), "the test host's window takes a first responder")
        _ = reps.textFieldShouldReturn(field)
        let weight = try node("weight")
        waitFor("Next focuses and reveals Weight") {
            weight.field?.isFirstResponder == true && self.fieldsShown([weight, add], below: bar)
        }
        XCTAssertEqual(weight.field?.isFirstResponder, true, "Next moved to Weight")
        try assertReachable("weight", below: bar)
        try assertReachable("add-set", below: bar)
        let bench = try node("bench-reps")
        // `layout form-list native 8`, retained if the test fails.
        let hierarchy = Agent(session: session).nativeSubviews(try node("form-list").id, depth: 8, limit: 200, plan: false)
        let data = try JSONSerialization.data(withJSONObject: hierarchy, options: [.prettyPrinted, .sortedKeys])
        let attachment = XCTAttachment(data: data, uniformTypeIdentifier: "public.json")
        attachment.name = "Native form-list hierarchy before Next"
        self.add(attachment)
        XCTAssertNotNil(bench.window, "the clipped field is still mounted")
        XCTAssertFalse(bench.bounds.isEmpty, "the offscreen field has laid-out bounds")
        XCTAssertGreaterThanOrEqual(box(bench).minY, keys,
                                    "the next field starts below the accessory, so Next must reveal it")
        XCTAssertFalse(hit(atCenterOf: bench)?.isDescendant(of: bench) == true,
                       "the offscreen field must not receive a touch before Next")
        _ = weight.textFieldShouldReturn(try XCTUnwrap(weight.field))
        let benchAdd = try node("bench-add")
        waitFor("Next reveals the next section") {
            bench.field?.isFirstResponder == true && self.fieldsShown([bench, benchAdd], below: bar)
        }
        XCTAssertEqual(bench.field?.isFirstResponder, true, "Next moved on to the next section's field")
        try assertReachable("bench-reps", below: bar)
        try assertReachable("bench-add", below: bar)
        bench.field?.resignFirstResponder()
        p.applyKeyboard(top: nil, duration: 0, curve: 0)
        waitFor("the tab bar returns after the keyboard") {
            p.keyboardTop == nil && abs(self.box(tabs.tabBar).minY - resting.minY) < 0.5
        }
        XCTAssertEqual(box(tabs.tabBar).minY, resting.minY, accuracy: 0.5, "the tab bar where it was before the keys")
    }

    func testTheBarFollowsAGroupedListThatReplacesTheRoutesScrollInPlace() throws {
        let session = try fixture()
        let tabs = try XCTUnwrap(session.presenter.navigation.tabController)
        _ = Agent(session: session).tap(["id": Int(try node("tab-other").id)])
        waitFor("the Other tab mounts its route") {
            (tabs.selectedViewController as? UINavigationController)?.topViewController?.navigationItem.title == "Other"
                && self.named("fill")?.window != nil
        }
        let nav = try XCTUnwrap(tabs.selectedViewController as? UINavigationController)
        let route = try XCTUnwrap(nav.topViewController)
        XCTAssertEqual(route.navigationItem.title, "Other")
        XCTAssertEqual(route.navigationItem.largeTitleDisplayMode, .always)
        XCTAssertTrue(route.contentScrollView(for: .top) is ScrollView, "the empty state's scroll")
        _ = Agent(session: session).tap(["id": Int(try node("fill").id)])
        waitFor("the grouped list replaces the empty scroll") {
            guard let list = self.named("other-list"), list.window != nil else { return false }
            return session.presenter.navigation.shownScroll(list) is GroupedScroller
        }
        XCTAssertNotNil(try? node("other-list"), "the list took the scroll's place")
        for edge: NSDirectionalRectEdge in [.top, .bottom] {
            let shown = route.contentScrollView(for: edge)
            XCTAssertTrue(shown is GroupedScroller, "edge \(edge.rawValue) follows the list's collection view, not \(String(describing: shown))")
        }
    }

    func testTheFocusedFieldIsShownAboveABarPinnedOverItsScroller() throws {
        let session = try fixture()
        let p = session.presenter
        _ = Agent(session: session).tap(["id": Int(try node("tab-pinned").id)])
        waitFor("the pinned tab reaches its tab bar") { self.pinnedLayoutReady(p) }
        let tabs = try XCTUnwrap(p.navigation.tabController)
        let nav = try XCTUnwrap(tabs.selectedViewController as? UINavigationController)
        let author = try node("pinned-author"), memo = try node("pinned-memo"), bar = try node("pinned-bar")
        let scroll = try XCTUnwrap(p.navigation.shownScroll(try node("pinned-scroll")))
        let base = scroll.contentInset.bottom
        XCTAssertGreaterThan(box(memo).minY, keys, "the fixture's last field starts where the keys will be")
        let top = box(nav.navigationBar).maxY

        func assertShown(_ field: NodeView, _ name: String, file: StaticString = #filePath, line: UInt = #line) {
            let r = box(field), cover = box(bar)
            XCTAssertLessThanOrEqual(r.maxY, cover.minY + 0.5, "\(name) above the pinned bar: \(r) over \(cover)", file: file, line: line)
            XCTAssertGreaterThanOrEqual(r.minY, top - 0.5, "\(name) below the navigation bar: \(r)", file: file, line: line)
            XCTAssertLessThanOrEqual(cover.maxY, keys + 0.5, "the bar rides the keys' top", file: file, line: line)
            let hit = hit(atCenterOf: field)
            XCTAssertTrue(hit?.isDescendant(of: field) == true, "\(name) takes a touch at its middle, not \(String(describing: hit))", file: file, line: line)
        }

        memo.textFieldDidBeginEditing(try XCTUnwrap(memo.field))
        p.applyKeyboard(top: keys, duration: 0, curve: 0)
        waitFor("Memo clears the pinned bar") {
            self.fieldsShown([memo], below: top) && self.box(memo).maxY <= self.box(bar).minY + 0.5
                && self.box(bar).maxY <= self.keys + 0.5 && scroll.contentInset.bottom > base
        }
        assertShown(memo, "memo")
        XCTAssertGreaterThan(scroll.contentInset.bottom, base, "room below the content for the field at its end")

        let room = scroll.contentInset.bottom
        author.textFieldDidBeginEditing(try XCTUnwrap(author.field))
        p.applyKeyboard(top: keys, duration: 0, curve: 0)
        waitFor("Author clears the pinned bar") {
            self.fieldsShown([author], below: top) && self.box(author).maxY <= self.box(bar).minY + 0.5
                && self.box(bar).maxY <= self.keys + 0.5
        }
        assertShown(author, "author")
        XCTAssertLessThanOrEqual(scroll.contentInset.bottom, room + 0.5, "no more room than it had")

        memo.field?.resignFirstResponder()
        author.field?.resignFirstResponder()
        p.applyKeyboard(top: nil, duration: 0, curve: 0)
        waitFor("the keyboard's extra room is removed") {
            let high = max(-scroll.adjustedContentInset.top, scroll.contentSize.height + scroll.adjustedContentInset.bottom - scroll.bounds.height)
            return p.keyboardTop == nil && abs(scroll.contentInset.bottom - base) < 0.5 && scroll.contentOffset.y <= high + 0.5
        }
        XCTAssertEqual(scroll.contentInset.bottom, base, accuracy: 0.5, "the room goes back with the keyboard")
        let high = max(-scroll.adjustedContentInset.top, scroll.contentSize.height + scroll.adjustedContentInset.bottom - scroll.bounds.height)
        XCTAssertLessThanOrEqual(scroll.contentOffset.y, high + 0.5, "and the offset is within the content again")
    }

    /// The pinned tab at rest: its scroll is the route's bottom-edge
    /// scroller, which reaches under the tab bar for UIKit to inset, and its
    /// absolutely positioned bar (`bottom=0`) rests on the tab bar rather
    /// than under it. Both hold after a keyboard comes and goes with the
    /// field still focused, which once left the scroll padded above the bars
    /// for good, and the room taken for the field goes with the keyboard.
    func testABarPinnedOverTheEdgeScrollerRestsOnTheTabBar() throws {
        let session = try fixture()
        let p = session.presenter
        _ = Agent(session: session).tap(["id": Int(try node("tab-pinned").id)])
        waitFor("the pinned tab reaches its tab bar") { self.pinnedLayoutReady(p) }
        let tabs = try XCTUnwrap(p.navigation.tabController)
        let bar = try node("pinned-bar"), memo = try node("pinned-memo")
        let scroll = try XCTUnwrap(p.navigation.shownScroll(try node("pinned-scroll")))
        let window = try XCTUnwrap(self.window)
        let base = scroll.contentInset.bottom
        func atRest(_ when: String, file: StaticString = #filePath, line: UInt = #line) {
            XCTAssertEqual(box(bar).maxY, box(tabs.tabBar).minY, accuracy: 0.5, "the pinned bar rests on the tab bar \(when)", file: file, line: line)
            XCTAssertEqual(box(scroll).maxY, window.bounds.maxY, accuracy: 0.5, "the scroll reaches under the tab bar \(when)", file: file, line: line)
            XCTAssertEqual(scroll.adjustedContentInset.bottom, window.bounds.maxY - box(tabs.tabBar).minY, accuracy: 0.5, "UIKit insets it by the tab bar \(when)", file: file, line: line)
        }
        atRest("at first")
        memo.textFieldDidBeginEditing(try XCTUnwrap(memo.field))
        p.applyKeyboard(top: keys, duration: 0, curve: 0)
        waitFor("the pinned scroller gives the field room above the keys") {
            scroll.contentInset.bottom > base && self.box(bar).maxY <= self.keys + 0.5
        }
        XCTAssertGreaterThan(scroll.contentInset.bottom, base, "room for the field at the end, above the pinned bar")
        p.applyKeyboard(top: nil, duration: 0, curve: 0)
        waitFor("the pinned bar and scroller return to the tab bar") {
            p.keyboardTop == nil && abs(self.box(bar).maxY - self.box(tabs.tabBar).minY) < 0.5
                && abs(self.box(scroll).maxY - window.bounds.maxY) < 0.5 && abs(scroll.contentInset.bottom - base) < 0.5
        }
        atRest("after the keyboard")
        XCTAssertEqual(scroll.contentInset.bottom, base, accuracy: 0.5, "the room goes with the keyboard, the field still focused")
    }

    func testTheCaretOfAGrowingTextAreaStaysAboveTheKeyboard() throws {
        let session = try fixture("EXACT_CARET_REVEAL_PLAN")
        let p = session.presenter
        let note = try node("note"), text = try XCTUnwrap(note.textArea)
        // Told it is edited, not made first responder: the viewport would
        // follow the test host's own keyboard guide, which has no keys
        note.textViewDidBeginEditing(text)
        p.applyKeyboard(top: keys, duration: 0, curve: 0)
        waitFor("the text area's viewport follows the keys") { abs(self.box(p.viewport).maxY - self.keys) < 0.5 }
        XCTAssertEqual(box(p.viewport).maxY, keys, accuracy: 0.5, "the viewport ends at the keys")
        let start = box(note).height
        // Typed as keys type: UIKit tells the delegate of each insertion
        for i in 1...14 {
            text.insertText(i == 1 ? "Line 1" : "\nLine \(i)")
            let end = try XCTUnwrap(text.selectedTextRange?.end)
            let caret = text.convert(text.caretRect(for: end), to: nil)
            XCTAssertLessThanOrEqual(caret.maxY, keys + 0.5, "line \(i)'s caret above the keys: \(caret)")
            XCTAssertGreaterThanOrEqual(caret.minY, box(p.viewport).minY - 0.5, "and inside the viewport: \(caret)")
            let hit = window?.hitTest(CGPoint(x: caret.midX, y: caret.midY), with: nil)
            XCTAssertTrue(hit?.isDescendant(of: note) == true, "nothing covers line \(i)'s caret: \(String(describing: hit))")
        }
        XCTAssertGreaterThan(box(note).height, start + 200, "the text area grew with its text: \(box(note))")
        p.applyKeyboard(top: nil, duration: 0, curve: 0)
    }

    /// As in a browser, which shows the selection once an edit's `input`
    /// handlers have run: a handler that moves the focus elsewhere, or hides
    /// the text area, keeps the scroll it leaves.
    func testAnInputHandlerThatMovesTheFocusOrHidesTheTextAreaKeepsTheScroll() throws {
        let session = try fixture("EXACT_CARET_REVEAL_PLAN")
        let p = session.presenter
        let list = try XCTUnwrap(p.navigation.shownScroll(try node("list")))
        let jumper = try node("jumper"), far = try node("far"), jumping = try XCTUnwrap(jumper.textArea)
        jumper.textViewDidBeginEditing(jumping)
        p.applyKeyboard(top: keys, duration: 0, curve: 0)
        waitFor("the input-handler fixture follows the keys") { abs(self.box(p.viewport).maxY - self.keys) < 0.5 }
        // The text area scrolled above the port, the far field still under the keys
        list.contentOffset.y += box(jumper).maxY - box(p.viewport).minY + 20
        XCTAssertLessThan(box(jumper).maxY, box(p.viewport).minY, "the text area starts above the port")
        XCTAssertGreaterThan(box(far).minY, keys, "the far field starts under the keys")
        jumping.insertText("x")
        XCTAssertTrue(p.editor === far, "the handler moved the focus to the far field")
        XCTAssertLessThan(box(jumper).maxY, box(p.viewport).minY, "the list is not scrolled back to the text area's caret: \(box(jumper))")
        far.field?.resignFirstResponder()
        let vanisher = try node("vanisher"), vanishing = try XCTUnwrap(vanisher.textArea)
        vanisher.textViewDidBeginEditing(vanishing)
        list.contentOffset.y += box(vanisher).maxY - box(p.viewport).minY + 20
        let offset = list.contentOffset.y
        XCTAssertLessThan(box(vanisher).maxY, box(p.viewport).minY, "the other text area starts above the port")
        vanishing.insertText("x")
        XCTAssertTrue(vanishing.isHidden, "the handler hid the text area (`visibility: hidden`)")
        XCTAssertEqual(list.contentOffset.y, offset, accuracy: 0.5, "the list is not scrolled to a hidden caret")
        p.applyKeyboard(top: nil, duration: 0, curve: 0)
    }

    func testTheAgentsFoldEndsWhereTheKeyboardEndsTheViewport() throws {
        let session = try fixture()
        let p = session.presenter, agent = Agent(session: session)
        let view = try XCTUnwrap(session.view)
        let root = try XCTUnwrap(p.root.subviews.first as? NodeView)
        let window = session.screenSize ?? p.viewportSize
        XCTAssertFalse(p.hasFold, "the agent supplies the fold on this phone")
        for policy in ["resizes-content", "resizes-visual", "overlays-content"] {
            p.apply(wireBatch([["op": "props", "id": Int(root.id), "set": ["interactiveWidget": policy]]]))
            XCTAssertEqual(p.keyboardPolicy, policy)
            var fold = ViewportFold(posture: "folded", cols: 1, rows: 2,
                                    rects: try Segments.even(viewport: window, cols: 1, rows: 2, gap: 40))
            XCTAssertNil(agent.prefer(["fold": ["posture": "folded", "cols": 1, "rows": 2, "gap": 40]])["error"])
            XCTAssertEqual(p.fold, fold, "\(policy): the full grid before the keyboard")
            p.applyKeyboard(top: keys, duration: 0, curve: 0)
            XCTAssertGreaterThan(p.keyboardInset, 0, "\(policy): the keyboard overlaps the viewport")
            let end = policy == "resizes-visual" ? p.viewportSize.height - p.keyboardInset : p.viewportSize.height
            if policy == "overlays-content" {
                XCTAssertEqual(p.fold, fold, "an overlay leaves the segments unchanged")
            } else {
                XCTAssertLessThan(end, fold.rects[1].maxY, "\(policy): the keyboard clips the lower segment")
                XCTAssertEqual(p.fold.rects.first, fold.rects.first, "\(policy): the upper segment stays in place")
                XCTAssertEqual(p.fold.rects.last?.minY, fold.rects[1].minY, "\(policy): the lower segment stays in place")
                XCTAssertEqual(p.fold.rects.last?.maxY ?? -1, end, accuracy: 0.01, "\(policy): the lower segment ends at the keyboard")
            }
            if policy != "resizes-content" {
                XCTAssertEqual(p.viewportSize, window, "\(policy): the layout viewport keeps its size")
            }
            // Change only posture while the grid is clipped; hiding must restore its full rectangles
            XCTAssertNil(agent.prefer(["fold": ["posture": "continuous"]])["error"])
            XCTAssertEqual(p.fold.posture, "continuous")
            XCTAssertEqual(p.fold.rects.last?.maxY ?? -1, end, accuracy: 0.01)
            p.applyKeyboard(top: nil, duration: 0, curve: 0)
            fold.posture = "continuous"
            XCTAssertEqual(p.fold, fold, "\(policy): dismissal restores the full grid and keeps the new posture")

            p.applyKeyboard(top: keys, duration: 0, curve: 0)
            XCTAssertNil(agent.prefer(["fold": ["gap": 80]])["error"])
            fold.rects = try Segments.even(viewport: window, cols: 1, rows: 2, gap: 80)
            XCTAssertEqual(p.fold.rects.last?.minY, fold.rects[1].minY, "\(policy): a new grid uses the window while the keyboard is up")
            XCTAssertEqual(p.fold.rects.last?.maxY ?? -1, end, accuracy: 0.01, "\(policy): a new grid is clipped immediately")
            p.applyKeyboard(top: nil, duration: 0, curve: 0)
            XCTAssertEqual(p.fold, fold, "\(policy): the new grid is restored on dismissal")

            p.applyKeyboard(top: fold.rects[0].midY, duration: 0, curve: 0)
            let upper = try XCTUnwrap(p.fold.rects.first), lower = try XCTUnwrap(p.fold.rects.last)
            if policy == "resizes-content" {
                XCTAssertLessThan(p.viewportSize.height, fold.rects[0].maxY)
                XCTAssertEqual(upper.maxY, p.viewportSize.height, accuracy: 0.01, "content resizing clips the upper row too")
                XCTAssertEqual(lower.height, 0)
            } else if policy == "resizes-visual" {
                XCTAssertEqual(upper, fold.rects[0], "visual resizing keeps the upper row even above the keys")
                XCTAssertEqual(lower.height, 0, "the last row ends at zero height when the keys reach above it")
            } else {
                XCTAssertEqual(p.fold, fold)
            }
            p.applyKeyboard(top: nil, duration: 0, curve: 0)
            XCTAssertEqual(p.fold, fold)
        }
        // Model a hardware fold reported after prefer's bounded discovery wait
        p.hasFold = true
        view.fit()
        XCTAssertNil(view.agentFold, "hardware detection discards the simulation")
        XCTAssertEqual(p.fold, .flat, "the view uses the device's geometry again")
        XCTAssertNotNil(agent.prefer(["fold": ["rows": 2]])["error"], "hardware keeps control of subsequent requests")
    }
}
#endif
