#if os(macOS)
import AppKit
import XCTest
@testable import ExactKit

final class AccessibilityTests: XCTestCase {
    private func fixture() -> (Presenter, NSWindow, NodeView, NodeView) {
        _ = NSApplication.shared
        let p = Presenter()
        let w = NSWindow(contentRect: NSRect(x: 0, y: 0, width: 400, height: 300), styleMask: [.titled], backing: .buffered, defer: false)
        w.contentView = p.viewport
        let first = NodeView(id: 1, kind: "button", presenter: p)
        let other = NodeView(id: 2, kind: "button", presenter: p)
        for n in [first, other] {
            n.frame = NSRect(x: 0, y: 0, width: 100, height: 40)
            p.root.addSubview(n); p.views[n.id] = n
        }
        first.props["autofocus"] = "true"
        return (p, w, first, other)
    }
    func testAutofocusRespectsExistingFocusAndStartsAgainAfterReset() {
        let (p, w, first, other) = fixture()
        XCTAssertTrue(w.makeFirstResponder(other))
        p.syncAccessibility()
        XCTAssertTrue(w.firstResponder === other)
        w.makeFirstResponder(nil)
        p.syncAccessibility()
        XCTAssertFalse(w.firstResponder === first)
        p.reset()
        let replacement = NodeView(id: 3, kind: "button", presenter: p)
        replacement.props["autofocus"] = "true"
        replacement.frame = first.frame
        p.root.addSubview(replacement); p.views[3] = replacement
        p.syncAccessibility()
        XCTAssertTrue(w.firstResponder === replacement)
    }
    func testAutofocusOnceAndVisibilityThroughNativeContainers() {
        let (p, w, first, _) = fixture()
        w.makeFirstResponder(nil)
        let wrapper = NSView(frame: first.frame)
        p.root.addSubview(wrapper); wrapper.addSubview(first)
        wrapper.isHidden = true
        XCTAssertFalse(first.accessibilityVisible)
        p.syncAccessibility()
        XCTAssertFalse(w.firstResponder === first)
        wrapper.isHidden = false
        p.syncAccessibility()
        XCTAssertTrue(w.firstResponder === first)
    }
    func testProjectionPrecedesAutofocus() {
        let (p, w, first, other) = fixture()
        w.makeFirstResponder(nil)
        let nav = NodeView(id: 3, kind: "view", presenter: p)
        let hidden = NodeView(id: 4, kind: "view", presenter: p)
        let active = NodeView(id: 5, kind: "view", presenter: p)
        nav.props = ["navigationBack": "back", "navigationKey": "active"]
        hidden.props["navigationKey"] = "hidden"
        active.props["navigationKey"] = "active"
        p.root.addSubview(nav); nav.addSubview(hidden); nav.addSubview(active)
        hidden.addSubview(first); active.addSubview(other)
        other.props["autofocus"] = "true"
        for n in [nav, hidden, active] { p.views[n.id] = n }
        p.apply(Batch(ops: [], timers: false, motion: false, clock: nil, error: nil))
        XCTAssertTrue(hidden.isHidden)
        XCTAssertTrue(w.firstResponder === other)
    }
    func testCanvasKeyAdmissionHonorsEditableAndActivationKeys() {
        let (p, w, button, _) = fixture()
        XCTAssertTrue(button.forwardsCanvasKey("KeyW"))
        XCTAssertFalse(button.forwardsCanvasKey("Space"))
        XCTAssertFalse(button.forwardsCanvasKey("Enter"))
        XCTAssertFalse(button.forwardsCanvasKey("Tab"))
        XCTAssertFalse(button.forwardsCanvasKey("KeyW", command: true))
        button.field = NSTextField()
        XCTAssertFalse(button.forwardsCanvasKey("KeyW"))
        withExtendedLifetime((p, w)) {}
    }
    /// VoiceOver's view of the tree, through the NSAccessibility attributes
    /// it reads: pressables are buttons or links, each a leaf named by its
    /// content; a labelled image is an image; a heading level is a heading;
    /// and every one of them is reachable from the window's content.
    func testPressablesLinksImagesAndHeadingsAreAccessibilityElements() {
        let (p, w, button, other) = fixture()
        button.handlers = ["press"]
        button.applyProps(set: ["accessibilityRole": "button", "accessibilitySelected": "true"], clear: [])
        let caption = NodeView(id: 10, kind: "text", presenter: p)
        caption.applyProps(set: ["text": "Dark"], clear: [])
        button.addSubview(caption); p.views[caption.id] = caption
        other.applyProps(set: ["accessibilityRole": "link", "accessibilityLabel": "Schedules"], clear: [])
        let logo = NodeView(id: 11, kind: "image", presenter: p)
        logo.applyProps(set: ["accessibilityLabel": "Caltrain"], clear: [])
        let spacer = NodeView(id: 12, kind: "image", presenter: p)
        spacer.applyProps(set: [:], clear: [])
        let heading = NodeView(id: 13, kind: "text", presenter: p)
        heading.applyProps(set: ["text": "Palo Alto", "accessibilityHeadingLevel": "1"], clear: [])
        let body = NodeView(id: 14, kind: "text", presenter: p)
        body.applyProps(set: ["text": "Northbound"], clear: [])
        for n in [logo, spacer, heading, body] {
            n.frame = NSRect(x: 0, y: 60, width: 100, height: 20)
            p.root.addSubview(n); p.views[n.id] = n
        }
        p.syncAccessibility()

        XCTAssertTrue(button.isAccessibilityElement())
        XCTAssertEqual(button.accessibilityRole(), .button)
        XCTAssertEqual(button.accessibilityLabel(), "Dark")
        XCTAssertTrue(button.isAccessibilitySelected())
        XCTAssertNil(button.accessibilityChildren(), "a control is a leaf")
        XCTAssertTrue(button.accessibilityPerformPress())
        XCTAssertTrue(other.isAccessibilityElement())
        XCTAssertEqual(other.accessibilityRole(), .link)
        XCTAssertEqual(other.accessibilityLabel(), "Schedules")
        XCTAssertTrue(logo.isAccessibilityElement())
        XCTAssertEqual(logo.accessibilityRole(), .image)
        XCTAssertEqual(logo.accessibilityLabel(), "Caltrain")
        XCTAssertFalse(spacer.isAccessibilityElement(), "an unlabelled image is decoration")
        XCTAssertEqual(heading.accessibilityRole()?.rawValue, "AXHeading")
        XCTAssertEqual(heading.accessibilityValue() as? Int, 1)
        XCTAssertEqual(heading.accessibilityLabel(), "Palo Alto")
        XCTAssertEqual(body.accessibilityRole(), .staticText)

        // What VoiceOver walks: element children from the window's content.
        var reached: [NSView] = []
        func walk(_ view: NSView) {
            for case let child as NSView in view.accessibilityChildren() ?? [] {
                if child.isAccessibilityElement() { reached.append(child) }
                walk(child)
            }
        }
        walk(w.contentView!)
        for node in [button, other, logo, heading, body] { XCTAssertTrue(reached.contains(node), "\(node.kind) \(node.id) is unreachable") }
        XCTAssertFalse(reached.contains(caption), "a button's text is its name, not a second stop")
        XCTAssertFalse(reached.contains(spacer))
    }
    /// Plain text takes the focus for selection but is never a Tab stop, as
    /// on the web: Tab walks the controls, and from a selected paragraph it
    /// goes on to the next stop after it. A key handler makes text focusable.
    func testParagraphsAreSelectableButNotTabStops() {
        let (p, w, first, other) = fixture()
        let text = NodeView(id: 20, kind: "text", presenter: p)
        text.applyProps(set: ["text": "Northbound"], clear: [])
        text.frame = NSRect(x: 0, y: 50, width: 100, height: 20)
        p.root.addSubview(text, positioned: .below, relativeTo: other); p.views[text.id] = text
        p.syncKeyViewLoop()
        XCTAssertTrue(text.acceptsFirstResponder, "a click still focuses it for selection")
        XCTAssertFalse(text.canBecomeKeyView)
        XCTAssertTrue(first.nextKeyView === other)
        XCTAssertTrue(other.nextKeyView === first)
        XCTAssertTrue(w.makeFirstResponder(first))
        w.selectNextKeyView(nil)
        XCTAssertTrue(w.firstResponder === other, "Tab passes over the paragraph")
        XCTAssertTrue(w.makeFirstResponder(text))
        w.selectNextKeyView(nil)
        XCTAssertTrue(w.firstResponder === other, "Tab from a selected paragraph goes on after it")
        text.handlers = ["key"]
        p.syncKeyViewLoop()
        XCTAssertTrue(text.canBecomeKeyView, "a key handler makes text focusable, as tabindex=0 does")
        XCTAssertTrue(first.nextKeyView === text)
    }
    /// HTML's `tabindex` (LLP 1088 D7.3): an explicit value makes a plain box
    /// focusable and, ≥ 0, a Tab stop, positive values first; a negative one
    /// takes a click but leaves Tab, a key handler's node included; absent is
    /// never `0`; a disabled button, inert and hidden boxes stay out, while a
    /// disabled box is a stop (`disabled` means nothing on a div, as in
    /// Chrome); a change while mounted moves it in or out.
    func testTabindexMakesABoxFocusableAndOrdersTab() {
        let (p, w, first, other) = fixture()
        func box(_ id: UInt32, _ props: [String: String], kind: String = "view") -> NodeView {
            let n = NodeView(id: id, kind: kind, presenter: p)
            n.applyProps(set: props, clear: [])
            n.frame = NSRect(x: 0, y: CGFloat(id) * 10, width: 100, height: 10)
            p.root.addSubview(n); p.views[id] = n
            return n
        }
        let stop = box(10, ["tabIndex": "0"]), plain = box(11, [:]), early = box(12, ["tabIndex": "2"])
        let skipped = box(13, ["tabIndex": "-1"]), keyed = box(14, ["tabIndex": "-1"])
        keyed.handlers = ["key"]
        let disabled = box(15, ["tabIndex": "0", "disabled": "true"]), inert = box(16, ["tabIndex": "0", "inert": "true"])
        let hidden = box(17, ["tabIndex": "0"]); hidden.isHidden = true
        let off = box(18, ["tabIndex": "0", "disabled": "true"], kind: "button")
        p.syncKeyViewLoop()
        XCTAssertFalse(plain.acceptsFirstResponder, "absent is not tabindex=0")
        XCTAssertTrue(stop.canBecomeKeyView, "tabindex=0 makes a box with no handler a stop")
        XCTAssertTrue(skipped.acceptsFirstResponder && !skipped.canBecomeKeyView, "-1: focusable, not a stop")
        XCTAssertTrue(keyed.acceptsFirstResponder && !keyed.canBecomeKeyView, "-1 takes a key handler's node out of Tab")
        XCTAssertTrue(disabled.canBecomeKeyView, "a disabled box is a stop, as Chrome's <div disabled>")
        for n in [off, inert, hidden] { XCTAssertFalse(n.acceptsFirstResponder || Presenter.tabbable(n) && !n.inert && !n.isHidden, "\(n.id) stays out") }
        XCTAssertTrue(early.nextKeyView === first, "a positive tabindex goes first, then tree order")
        XCTAssertTrue(first.nextKeyView === other && other.nextKeyView === stop)
        XCTAssertTrue(stop.nextKeyView === disabled && disabled.nextKeyView === early, "the loop wraps to the positive one")
        XCTAssertTrue(w.makeFirstResponder(other))
        w.selectNextKeyView(nil)
        XCTAssertTrue(w.firstResponder === stop, "Tab lands on the box itself")
        // A click makes the node first responder when it accepts (`mouseDown`;
        // the window's `makeFirstResponder` itself does not ask).
        XCTAssertTrue(skipped.acceptsFirstResponder && w.makeFirstResponder(skipped) && w.firstResponder === skipped, "a click focuses tabindex=-1")
        skipped.applyProps(set: ["tabIndex": "0"], clear: [])
        stop.applyProps(set: [:], clear: ["tabIndex"])
        p.syncKeyViewLoop()
        XCTAssertTrue(skipped.canBecomeKeyView && !stop.acceptsFirstResponder, "a change while mounted moves eligibility")
    }
    /// A batch marks the loop stale; the next key event rebuilds it before
    /// AppKit reads `nextKeyView`, so scrolling a list walks no nodes for it.
    func testTheKeyViewLoopIsRebuiltWhenAKeyNeedsIt() {
        let (p, w, first, other) = fixture()
        first.nextKeyView = nil; other.nextKeyView = nil
        p.keyViewLoopStale = true
        p.flushKeyViewLoop()
        XCTAssertFalse(p.keyViewLoopStale)
        XCTAssertTrue(first.nextKeyView === other)
        XCTAssertTrue(w.makeFirstResponder(first))
        w.selectNextKeyView(nil)
        XCTAssertTrue(w.firstResponder === other)
        first.nextKeyView = nil
        p.flushKeyViewLoop()
        XCTAssertNil(first.nextKeyView, "a fresh loop is not rebuilt")
    }
    /// `aria-pressed` as Core-AAM maps it: a toggle button is AXCheckBox,
    /// subrole AXToggle, value 0, 1 or 2 (mixed); without it, a button again.
    func testAriaPressedMakesAToggleButton() {
        let (_, w, button, _) = fixture()
        button.applyProps(set: ["accessibilityRole": "button", "accessibilityPressed": "true"], clear: [])
        XCTAssertEqual(button.accessibilityRole(), .checkBox)
        XCTAssertEqual(button.accessibilitySubrole(), .toggle)
        XCTAssertEqual(button.accessibilityValue() as? Int, 1)
        button.applyProps(set: ["accessibilityPressed": "mixed"], clear: [])
        XCTAssertEqual(button.accessibilityValue() as? Int, 2)
        button.applyProps(set: ["accessibilityPressed": "false"], clear: [])
        XCTAssertEqual(button.accessibilityValue() as? Int, 0)
        button.applyProps(set: [:], clear: ["accessibilityPressed"])
        XCTAssertEqual(button.accessibilityRole(), .button)
        XCTAssertNil(button.accessibilitySubrole())
        XCTAssertNil(button.accessibilityValue())
        withExtendedLifetime(w) {}
    }
    /// The agent's name follows the web's accname: a non-empty label names
    /// any node (a status text); an empty one names nothing, so a button
    /// falls back to its content.
    func testALabelNamesATextAndAnEmptyLabelNamesNothing() {
        let (p, w, button, _) = fixture()
        let purse = NodeView(id: 30, kind: "text", presenter: p)
        purse.applyProps(set: ["text": "20¢", "accessibilityRole": "status", "accessibilityLabel": "20 sheckles"], clear: [])
        XCTAssertEqual(purse.authoredLabel, "20 sheckles")
        XCTAssertEqual(purse.accessibleName, "20 sheckles")
        button.applyProps(set: ["text": "Play", "accessibilityLabel": ""], clear: [])
        XCTAssertNil(button.authoredLabel)
        XCTAssertEqual(button.accessibleName, "Play")
        withExtendedLifetime(w) {}
    }
    func testOffDoesNotTrackALiveRegion() {
        let (p, w, first, _) = fixture()
        first.props["accessibilityLive"] = "off"
        first.props["text"] = "Quiet"
        p.syncAccessibility()
        XCTAssertNil(first.liveText)
        first.props["accessibilityLive"] = "polite"
        p.syncAccessibility()
        XCTAssertEqual(first.liveText, "Quiet")
        withExtendedLifetime(w) {}
    }
    /// Spreadsheet F4, F14: ⌘V, ⌘C and ⌘X at a focused node are its
    /// `paste`, `copy` and `cut` — the nearest handler's — and with none the
    /// Edit menu's action is not this node's, so the chain goes on.
    func testEditActionsAreClipboardEventsAtTheNearestHandler() {
        let (p, w, grid, other) = fixture()
        grid.handlers = ["paste", "copy"]
        let cell = NodeView(id: 3, kind: "view", presenter: p)
        cell.handlers = ["focus"]
        grid.addSubview(cell); p.views[cell.id] = cell
        var heard: [String] = []
        p.onClipboard = { id, kind, text in heard.append("\(id):\(kind):\(text)") }
        XCTAssertTrue(cell.acceptsFirstResponder)
        XCTAssertTrue(cell.responds(to: #selector(NodeView.paste(_:))))
        XCTAssertFalse(cell.responds(to: #selector(NodeView.cut(_:))), "no cut handler: the chain goes on")
        XCTAssertFalse(other.responds(to: #selector(NodeView.paste(_:))))
        XCTAssertTrue(cell.clipboard(#selector(NodeView.paste(_:)), text: "a\tb"))
        XCTAssertTrue(cell.clipboard(#selector(NodeView.copy(_:))))
        XCTAssertEqual(heard, ["1:34:a\tb", "1:32:"])
        withExtendedLifetime(w) {}
    }
    /// Gallery F20: a click's modifiers ride with its press, as the
    /// `MouseEvent` a press action may take; any other press holds none.
    func testAPressCarriesTheModifiersHeld() {
        let (p, w, button, _) = fixture()
        button.handlers = ["press"]
        var held: [String] = []
        p.onPress = { _ in held.append(p.pressHeld) }
        p.press(button.id, held: KeyCodes.held([.shift, .command]))
        p.press(button.id)
        XCTAssertEqual(held, ["Shift+Meta+", ""])
        XCTAssertEqual(p.pressHeld, "")
        withExtendedLifetime(w) {}
    }
    /// A booted session's autofocus waits for the turn after its first
    /// activation. A field mounted after that focuses in its own batch.
    func testLaunchAutofocusWaitsForTheTurnAfterActivation() throws {
        let (session, w) = try launchFixture()
        defer { session.destroy() }
        let p = session.presenter
        let edit = try XCTUnwrap(p.views.values.first { $0.props["testId"] == "edit" })
        XCTAssertNil(p.focusedNode, "the first batch takes no focus")
        session.drawReceipt()()
        XCTAssertNil(p.focusedNode, "activation and release are later turns")
        var afterActivation: NodeView?? = nil
        DispatchQueue.main.async { afterActivation = .some(p.focusedNode) } // after activation, before the release it queues
        spin { afterActivation != nil && p.focusedNode != nil }
        XCTAssertEqual(afterActivation.map { $0 == nil }, true, "the release is the turn after activation")
        XCTAssertTrue(p.focusedNode === edit)
        w.makeFirstResponder(nil)
        let late = NodeView(id: 900, kind: "button", presenter: p)
        late.frame = NSRect(x: 0, y: 100, width: 100, height: 40); late.props["autofocus"] = "true"
        p.root.addSubview(late); p.views[late.id] = late
        p.syncAccessibility()
        XCTAssertTrue(w.firstResponder === late, "a later mount focuses at once")
        withExtendedLifetime(w) {}
    }
    /// A carried restart before the release leaves the launch autofocus
    /// pending. After the release, a restart restores the focus it found.
    func testARestartBeforeTheReleaseKeepsTheLaunchAutofocus() throws {
        let (session, w) = try launchFixture()
        defer { session.destroy() }
        let p = session.presenter
        XCTAssertTrue(session.apply(plan))
        XCTAssertNil(p.focusedNode)
        session.drawReceipt()()
        spin { p.focusedNode != nil }
        XCTAssertEqual(p.focusedNode?.props["testId"], "edit")
        XCTAssertTrue(session.apply(plan))
        XCTAssertEqual(p.focusedNode?.props["testId"], "edit", "restoreFocus puts it back")
        withExtendedLifetime(w) {}
    }
    /// A restart before the release that cannot restore the focus it found
    /// consumes the launch autofocus, as any restart with a focus does.
    func testARestartThatLosesAChosenFocusConsumesTheLaunchAutofocus() throws {
        let (session, w) = try launchFixture(extra: "      input testId=\"other\" value=\"\"\n")
        defer { session.destroy() }
        let p = session.presenter
        let other = try XCTUnwrap(p.views.values.first { $0.props["testId"] == "other" })
        XCTAssertTrue(w.makeFirstResponder(other.field ?? other))
        let replacement = try compile(Self.launchSource)
        XCTAssertTrue(session.apply(replacement))
        XCTAssertTrue(session.apply(replacement), "a second restart, with nothing focused, keeps it consumed")
        session.drawReceipt()()
        for _ in 0..<2 { // activation's turn, then the turn of the release it queues
            var ran = false
            DispatchQueue.main.async { ran = true }
            spin { ran }
        }
        XCTAssertNil(p.focusedNode)
        withExtendedLifetime(w) {}
    }
    /// Runs the main run loop until `done` holds, for at most `timeout` seconds.
    private func spin(timeout: TimeInterval = 10, until done: () -> Bool) {
        let end = Date().addingTimeInterval(timeout)
        while !done(), Date() < end { RunLoop.main.run(until: Date().addingTimeInterval(0.005)) }
        XCTAssertTrue(done(), "timed out")
    }
    private static let launchSource = "component App\n  view\n    column width=\"100%\" height=\"100%\"\n      input testId=\"edit\" value=\"draft\" autofocus=true\n"
    private var plan = Data()
    private func compile(_ source: String) throws -> Data {
        let dir = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
        try FileManager.default.createDirectory(at: dir, withIntermediateDirectories: true)
        defer { try? FileManager.default.removeItem(at: dir) }
        try source.write(to: dir.appendingPathComponent("app.contract"), atomically: true, encoding: .utf8)
        let compiler = Process()
        compiler.executableURL = URL(fileURLWithPath: try XCTUnwrap(ProcessInfo.processInfo.environment["EXACT_CONTRACT"]))
        compiler.arguments = ["build", dir.appendingPathComponent("app.contract").path, "-o", dir.appendingPathComponent("app.plan").path]
        try compiler.run(); compiler.waitUntilExit()
        XCTAssertEqual(compiler.terminationStatus, 0)
        return try Data(contentsOf: dir.appendingPathComponent("app.plan"))
    }
    private func launchFixture(extra: String = "") throws -> (ExactSession, NSWindow) {
        _ = NSApplication.shared
        plan = try compile(Self.launchSource + extra)
        let session = ExactApp.shared.makeSession()
        let view = ExactView(session: session)
        let w = NSWindow(contentRect: NSRect(x: 0, y: 0, width: 400, height: 300), styleMask: [.titled], backing: .buffered, defer: false)
        w.isReleasedWhenClosed = false
        let mount = NSView(frame: NSRect(x: 0, y: 0, width: 400, height: 300))
        w.contentView = mount
        view.frame = mount.bounds; mount.addSubview(view)
        XCTAssertNil(session.boot(plan: plan, size: CGSize(width: 400, height: 300)).error)
        return (session, w)
    }
}
#endif
