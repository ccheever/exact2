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
