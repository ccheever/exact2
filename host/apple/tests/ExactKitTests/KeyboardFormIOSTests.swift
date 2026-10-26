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
    /// Model a number pad occupying the bottom 302 points of this window.
    private var keys: CGFloat { (window?.bounds.height ?? 0) - 302 }

    override func tearDown() {
        session?.destroy()
        session = nil
        window?.isHidden = true
        window = nil
        super.tearDown()
    }

    private func waitFor(_ what: String, file: StaticString = #filePath, line: UInt = #line, _ ready: () -> Bool) {
        func done() -> Bool {
            guard let session, !Agent(session: session).nativeInFlight() else { return false }
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

    private func safeAreaBottom(_ p: Presenter) -> CGFloat? {
        guard let view = p.navigation.activeRoute?.view else { return nil }
        return view.convert(view.safeAreaLayoutGuide.layoutFrame, to: nil).maxY
    }

    private func pinnedLayoutReady(_ p: Presenter) -> Bool {
        guard let bar = named("pinned-bar"), let memo = named("pinned-memo"), let node = named("pinned-scroll"),
              let scroll = p.navigation.shownScroll(node), let bottom = safeAreaBottom(p), let window else { return false }
        return bar.window != nil && memo.window != nil && abs(box(bar).maxY - bottom) < 0.5
            && abs(box(scroll).maxY - window.bounds.maxY) < 0.5
            && abs(scroll.adjustedContentInset.bottom - window.bounds.maxY + bottom) < 0.5
    }

    private func fixture(_ plan: String = "EXACT_KEYBOARD_PLAN") throws -> ExactSession {
        let path = try XCTUnwrap(ProcessInfo.processInfo.environment[plan], "build.mjs --test --ios compiles \(plan)")
        ExactGroupedLists.install()
        let session = ExactApp.shared.makeSession(label: "keyboard-form")
        self.session = session
        let view = ExactView(session: session)
        let host = UIViewController()
        let scene = hostedWindowScene()
        let window = scene.map { UIWindow(windowScene: $0) } ?? UIWindow(frame: .zero)
        window.frame = window.screen.bounds
        window.rootViewController = host
        window.makeKeyAndVisible()
        view.frame = host.view.bounds
        view.autoresizingMask = [.flexibleWidth, .flexibleHeight]
        host.view.addSubview(view)
        self.window = window
        XCTAssertNil(session.boot(plan: try Data(contentsOf: URL(fileURLWithPath: path)), size: view.bounds.size).error)
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
        guard hostedWindowScene() != nil else {
            throw missing("scene", "keyboard ownership requires the hosted app responder chain")
        }
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
        XCTAssertTrue(try XCTUnwrap(reps.field).becomeFirstResponder())
        setKeyboard(p, top: keys)
        let bar = box(nav.navigationBar).maxY
        let route = try XCTUnwrap(nav.topViewController as? RouteController)
        waitFor("the field and its action clear the accessory") {
            abs(route.node.bounds.height - self.keys) < 0.5
                && self.fieldsShown([reps, add], below: bar)
        }
        try assertReachable("reps", below: bar)
        try assertReachable("add-set", below: bar)
        _ = try XCTUnwrap(reps.field?.inputAccessoryView)
        XCTAssertEqual(box(p.viewport), try XCTUnwrap(window).bounds, "the native container remains full-size under the keyboard and accessory")
        XCTAssertEqual(box(tabs.tabBar).minY, resting.minY, accuracy: 0.5, "the native tab bar keeps its position: \(box(tabs.tabBar))")

        let field = try XCTUnwrap(reps.field)
        XCTAssertTrue(field.isFirstResponder, "the field keeps focus while the keyboard changes its viewport")
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
        setKeyboard(p, top: nil)
        waitFor("the tab bar returns after the keyboard") {
            p.keyboardTop == nil && abs(self.box(tabs.tabBar).minY - resting.minY) < 0.5
        }
        XCTAssertEqual(box(tabs.tabBar).minY, resting.minY, accuracy: 0.5, "the tab bar where it was before the keys")
    }

    /// A hatch focus request reveals a field whose row has no cell and moves
    /// editing to it. Refused targets leave focus and the list's offset unchanged.
    func testProgrammaticFocusRevealsAnOffscreenGroupedFieldWithTheKeyboardUp() throws {
        guard hostedWindowScene() != nil else {
            throw missing("scene", "programmatic keyboard focus requires the hosted app responder chain")
        }
        let session = try fixture()
        let p = session.presenter
        let reps = try node("reps"), add = try node("add-set")
        let bench = try node("bench-reps"), benchAdd = try node("bench-add")
        let route = try XCTUnwrap(p.navigation.activeRoute)
        let nav = try XCTUnwrap(route.navigationController)
        let list = try XCTUnwrap(p.navigation.shownScroll(try node("form-list")))
        XCTAssertTrue(try XCTUnwrap(reps.field).becomeFirstResponder())
        setKeyboard(p, top: keys)
        waitFor("the initial field and its action clear the keyboard") {
            self.fieldsShown([reps, add], below: self.box(nav.navigationBar).maxY)
        }
        XCTAssertFalse(sequence(first: bench as UIView, next: \.superview).contains { $0 is UICollectionViewCell },
                       "the target starts in a virtualized row with no native cell")
        let before = list.contentOffset.y

        func assertRefused(_ target: NodeView, reason: String, file: StaticString = #filePath, line: UInt = #line) throws {
            p.focusNode(target)
            let reply = try JSONSerialization.jsonObject(with: Data(session.agent("{\"op\":\"logs\"}").utf8)) as? [String: Any]
            let lines = try XCTUnwrap(reply?["lines"] as? [String], "the session exposes its focus journal", file: file, line: line)
            XCTAssertTrue(lines.last?.hasSuffix("refused: \(reason)") == true,
                          "focus refusal reports \(reason): \(lines)", file: file, line: line)
            XCTAssertTrue(reps.field?.isFirstResponder == true, "a refused focus leaves the current editor focused", file: file, line: line)
            XCTAssertEqual(list.contentOffset.y, before, accuracy: 0.5, "a refused focus does not scroll the list", file: file, line: line)
        }

        let button = try node("add-exercise")
        XCTAssertTrue(p.groupedLists?.draws(button.id) == true, "UIKit draws this row instead of carrying its authored button")
        try assertRefused(button, reason: "hidden ancestor")
        for (property, reason) in [("disabled", "disabled"), ("inert", "inert ancestor")] {
            bench.props[property] = "true"
            try assertRefused(bench, reason: reason)
            bench.props[property] = nil
        }
        let section = try XCTUnwrap(sequence(first: bench as UIView, next: \.superview)
            .compactMap { $0 as? NodeView }.first { p.groupedLists?.hidSection($0) == true })
        let style = section.style
        section.style["display"] = .string("none")
        try assertRefused(bench, reason: "hidden ancestor")
        section.style = style

        p.focusNode(bench)

        waitFor("programmatic focus mounts and reveals the target row and its action") {
            bench.field?.isFirstResponder == true
                && self.fieldsShown([bench, benchAdd], below: self.box(nav.navigationBar).maxY)
        }
        XCTAssertGreaterThan(list.contentOffset.y, before, "the native list scrolls to the requested field")
        try assertReachable("bench-reps", below: box(nav.navigationBar).maxY)
        try assertReachable("bench-add", below: box(nav.navigationBar).maxY)
        XCTAssertEqual(p.viewport.contentOffset.y, 0, accuracy: 0.5, "the document viewport does not scroll; the list does")
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
        guard hostedWindowScene() != nil else {
            throw missing("scene", "keyboard ownership requires the hosted app responder chain")
        }
        let session = try fixture()
        let p = session.presenter
        _ = Agent(session: session).tap(["id": Int(try node("tab-pinned").id)])
        waitFor("the pinned tab reaches its safe area") { self.pinnedLayoutReady(p) }
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
        setKeyboard(p, top: keys)
        waitFor("Memo clears the pinned bar") {
            self.fieldsShown([memo], below: top) && self.box(memo).maxY <= self.box(bar).minY + 0.5
                && self.box(bar).maxY <= self.keys + 0.5 && scroll.contentInset.bottom > base
        }
        assertShown(memo, "memo")
        XCTAssertGreaterThan(scroll.contentInset.bottom, base, "room below the content for the field at its end")

        let room = scroll.contentInset.bottom
        author.textFieldDidBeginEditing(try XCTUnwrap(author.field))
        setKeyboard(p, top: keys)
        waitFor("Author clears the pinned bar") {
            self.fieldsShown([author], below: top) && self.box(author).maxY <= self.box(bar).minY + 0.5
                && self.box(bar).maxY <= self.keys + 0.5
        }
        assertShown(author, "author")
        XCTAssertLessThanOrEqual(scroll.contentInset.bottom, room + 0.5, "no more room than it had")

        memo.field?.resignFirstResponder()
        author.field?.resignFirstResponder()
        setKeyboard(p, top: nil)
        waitFor("the keyboard's extra room is removed") {
            let high = max(-scroll.adjustedContentInset.top, scroll.contentSize.height + scroll.adjustedContentInset.bottom - scroll.bounds.height)
            return p.keyboardTop == nil && abs(scroll.contentInset.bottom - base) < 0.5 && scroll.contentOffset.y <= high + 0.5
        }
        XCTAssertEqual(scroll.contentInset.bottom, base, accuracy: 0.5, "the room goes back with the keyboard")
        let high = max(-scroll.adjustedContentInset.top, scroll.contentSize.height + scroll.adjustedContentInset.bottom - scroll.bounds.height)
        XCTAssertLessThanOrEqual(scroll.contentOffset.y, high + 0.5, "and the offset is within the content again")
    }

    /// A pinned bar rests at the route's native safe-area edge while its
    /// scroller reaches the window's bottom. The keyboard leaves while Memo
    /// remains the editing field; its extra room must be released.
    func testAPinnedBarAndItsScrollerRespectTheRoutesSafeArea() throws {
        guard hostedWindowScene() != nil else {
            throw missing("scene", "keyboard ownership requires the hosted app responder chain")
        }
        let session = try fixture()
        let p = session.presenter
        _ = Agent(session: session).tap(["id": Int(try node("tab-pinned").id)])
        waitFor("the pinned tab reaches its safe area") { self.pinnedLayoutReady(p) }
        let bottom = try XCTUnwrap(safeAreaBottom(p))
        let bar = try node("pinned-bar"), memo = try node("pinned-memo")
        let scroll = try XCTUnwrap(p.navigation.shownScroll(try node("pinned-scroll")))
        let window = try XCTUnwrap(self.window)
        let base = scroll.contentInset.bottom
        func atRest(_ when: String, file: StaticString = #filePath, line: UInt = #line) {
            XCTAssertEqual(box(bar).maxY, bottom, accuracy: 0.5, "the pinned bar rests at the safe-area edge \(when)", file: file, line: line)
            XCTAssertEqual(box(scroll).maxY, window.bounds.maxY, accuracy: 0.5, "the scroller reaches the window bottom \(when)", file: file, line: line)
            XCTAssertEqual(scroll.adjustedContentInset.bottom, window.bounds.maxY - bottom, accuracy: 0.5, "UIKit preserves the native bottom inset \(when)", file: file, line: line)
        }
        atRest("at first")
        memo.textFieldDidBeginEditing(try XCTUnwrap(memo.field))
        setKeyboard(p, top: keys)
        waitFor("the pinned scroller gives the field room above the keys") {
            scroll.contentInset.bottom > base && self.box(bar).maxY <= self.keys + 0.5
        }
        XCTAssertGreaterThan(scroll.contentInset.bottom, base, "room for the field at the end, above the pinned bar")
        setKeyboard(p, top: nil)
        XCTAssertTrue(p.editing === memo, "hiding the keyboard leaves the field's edit active")
        waitFor("the pinned bar and scroller return to the safe area") {
            p.keyboardTop == nil && abs(self.box(bar).maxY - bottom) < 0.5
                && abs(self.box(scroll).maxY - window.bounds.maxY) < 0.5 && abs(scroll.contentInset.bottom - base) < 0.5
        }
        atRest("after the keyboard")
        XCTAssertEqual(scroll.contentInset.bottom, base, accuracy: 0.5, "the room goes with the keyboard, the field still focused")
    }

    func testTheCaretOfAGrowingTextAreaStaysAboveTheKeyboard() throws {
        guard hostedWindowScene() != nil else {
            throw missing("scene", "keyboard ownership requires the hosted app responder chain")
        }
        let session = try fixture("EXACT_CARET_REVEAL_PLAN")
        let p = session.presenter
        let note = try node("note"), text = try XCTUnwrap(note.textArea)
        // Model editing without opening a keyboard; setKeyboard supplies
        // the guide and ownership for this caret-reveal test
        note.textViewDidBeginEditing(text)
        setKeyboard(p, top: keys)
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
        setKeyboard(p, top: nil)
    }

    /// As in a browser, which shows the selection once an edit's `input`
    /// handlers have run: a handler that moves the focus elsewhere, or hides
    /// the text area, keeps the scroll it leaves.
    func testAnInputHandlerThatMovesTheFocusOrHidesTheTextAreaKeepsTheScroll() throws {
        guard hostedWindowScene() != nil else {
            throw missing("scene", "keyboard ownership requires the hosted app responder chain")
        }
        let session = try fixture("EXACT_CARET_REVEAL_PLAN")
        let p = session.presenter
        let list = try XCTUnwrap(p.navigation.shownScroll(try node("list")))
        let jumper = try node("jumper"), far = try node("far"), jumping = try XCTUnwrap(jumper.textArea)
        jumper.textViewDidBeginEditing(jumping)
        setKeyboard(p, top: keys)
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
        setKeyboard(p, top: nil)
    }

    func testTheNativeRoutesEditorIsRevealedAfterItsPendingFill() throws {
        guard hostedWindowScene() != nil else {
            throw missing("scene", "keyboard ownership requires the hosted app responder chain")
        }
        let session = try fixture(), p = session.presenter
        let route = try node("route-log"), field = try node("bench-reps")
        let scroll = try XCTUnwrap(p.navigation.shownScroll(try node("form-list")))
        XCTAssertTrue(try XCTUnwrap(field.field).becomeFirstResponder())
        p.collections.onFill?(route.id, Data())
        XCTAssertTrue(session.fillInFlight)
        let rest = -scroll.adjustedContentInset.top
        scroll.contentOffset = CGPoint(x: 0, y: rest)
        XCTAssertGreaterThan(box(field).maxY, keys, "the editor starts beneath the keyboard")
        setKeyboard(p, top: keys)
        NotificationCenter.default.post(name: UIResponder.keyboardDidChangeFrameNotification, object: nil)
        waitFor("the fill lands and the editor clears the keyboard") {
            !session.fillInFlight && self.box(field).maxY <= self.keys + 0.5 && scroll.contentOffset.y > rest
        }
    }

    func testNativeKeyboardAvoidanceKeepsTheWindowsFoldAndRootSize() throws {
        guard hostedWindowScene() != nil else {
            throw missing("scene", "keyboard ownership requires the hosted app responder chain")
        }
        let session = try fixture()
        let p = session.presenter, agent = Agent(session: session)
        let view = try XCTUnwrap(session.view)
        // Keep the safe-area assertion meaningful on windows without a home indicator
        let host = try XCTUnwrap(self.window?.rootViewController)
        host.additionalSafeAreaInsets.bottom += 20
        host.view.layoutIfNeeded()
        let root = try XCTUnwrap(p.root.subviews.first as? NodeView)
        p.apply(wireBatch([["op": "props", "id": Int(root.id), "set": ["viewportFit": "cover"]]]))
        view.setNeedsLayout()
        view.layoutIfNeeded()
        let safe = p.insets
        XCTAssertGreaterThan(safe.bottom, 0)
        let window = session.screenSize ?? p.viewportSize
        if #available(iOS 27.1, *) {
            waitFor("UIKit reports the device's fold") { p.hingeReported }
        }
        if p.hasFold {
            let fold = p.fold
            for policy in ["resizes-content", "resizes-visual", "overlays-content"] {
                p.apply(wireBatch([["op": "props", "id": Int(root.id), "set": ["interactiveWidget": policy]]]))
                setKeyboard(p, top: keys)
                XCTAssertGreaterThan(p.keyboardInset, 0)
                XCTAssertEqual(p.viewportSize, window)
                XCTAssertEqual(p.insets, safe)
                XCTAssertEqual(p.fold, fold, "keyboard avoidance preserves the device's real division regions")
                setKeyboard(p, top: nil)
                XCTAssertEqual(p.fold, fold)
            }
            return
        }
        for policy in ["resizes-content", "resizes-visual", "overlays-content"] {
            p.apply(wireBatch([["op": "props", "id": Int(root.id), "set": ["interactiveWidget": policy]]]))
            XCTAssertEqual(p.keyboardPolicy, policy)
            var fold = ViewportFold(posture: "folded", cols: 1, rows: 2,
                                    rects: try Segments.even(viewport: window, cols: 1, rows: 2, gap: 40))
            XCTAssertNil(agent.prefer(["fold": ["posture": "folded", "cols": 1, "rows": 2, "gap": 40]])["error"])
            XCTAssertEqual(p.fold, fold, "\(policy): the full grid before the keyboard")
            setKeyboard(p, top: keys)
            XCTAssertGreaterThan(p.keyboardInset, 0, "\(policy): the keyboard overlaps the viewport")
            XCTAssertEqual(p.insets, safe, "the owning route's keyboard preserves the other routes' safe area")
            XCTAssertEqual(p.viewportSize, window, "\(policy): the native application root keeps its size")
            XCTAssertEqual(p.fold, fold, "\(policy): keyboard avoidance does not change the window's division regions")
            let route = try XCTUnwrap(p.navigation.activeRoute)
            XCTAssertEqual(route.node.bounds.height, policy == "resizes-content" ? keys : window.height, accuracy: 0.5)

            XCTAssertNil(agent.prefer(["fold": ["posture": "continuous"]])["error"])
            fold.posture = "continuous"
            XCTAssertEqual(p.fold, fold, "posture changes use the window's full regions while editing")
            setKeyboard(p, top: nil)
            XCTAssertEqual(p.fold, fold)

            setKeyboard(p, top: keys)
            XCTAssertNil(agent.prefer(["fold": ["gap": 80]])["error"])
            fold.rects = try Segments.even(viewport: window, cols: 1, rows: 2, gap: 80)
            XCTAssertEqual(p.fold, fold, "a new grid uses the full window while editing")
            setKeyboard(p, top: nil)
            XCTAssertEqual(p.fold, fold)

            let upperKeys = fold.rects[0].midY
            setKeyboard(p, top: upperKeys)
            XCTAssertEqual(p.viewportSize, window)
            XCTAssertEqual(p.fold, fold)
            XCTAssertEqual(route.node.bounds.height, policy == "resizes-content" ? upperKeys : window.height, accuracy: 0.5,
                           "only the editing route's content area changes")
            setKeyboard(p, top: nil)
            XCTAssertEqual(route.node.bounds.size, window)
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
