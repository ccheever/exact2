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
}
#endif
