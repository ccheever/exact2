#if os(macOS)
import AppKit
import XCTest
@testable import ExactKit

final class AccessibilityTests: XCTestCase {
    /// These cases walk Tab through buttons: Keyboard Navigation on (LLP 1115.000 D7.2).
    override func setUp() { super.setUp(); keyboardNavigation(true) }
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
        heading.applyProps(set: ["text": "Palo Alto", "accessibilityRole": "heading", "accessibilityHeadingLevel": "1"], clear: [])
        let body = NodeView(id: 14, kind: "text", presenter: p)
        body.applyProps(set: ["text": "Northbound"], clear: [])
        // LLP 1115.000 D12.5: an `aria-level` with no `role="heading"` is not a heading.
        let levelled = NodeView(id: 15, kind: "text", presenter: p)
        levelled.applyProps(set: ["text": "Departures", "accessibilityHeadingLevel": "2"], clear: [])
        for n in [logo, spacer, heading, body, levelled] {
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
        XCTAssertEqual(levelled.accessibilityRole(), .staticText, "a level alone is text")
        XCTAssertEqual(levelled.accessibilityValue() as? String, "Departures")
        levelled.applyProps(set: ["accessibilityRole": "heading"], clear: [])
        p.syncAccessibility()
        XCTAssertEqual(levelled.accessibilityRole()?.rawValue, "AXHeading", "its role makes it one")
        XCTAssertEqual(levelled.accessibilityValue() as? Int, 2)

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
    /// X74: a heading inside a button is the button's child, as Chrome keeps
    /// `<button><h2>` (button › heading). The button keeps its name, state
    /// and press; its text and images stay its name, not children, so a
    /// button holding only those is still a leaf, and `aria-hidden` hides.
    func testAHeadingInsideAButtonIsItsChild() {
        let (p, w, button, other) = fixture()
        button.handlers = ["press"]
        button.applyProps(set: ["accessibilityExpanded": "false"], clear: [])
        let row = NodeView(id: 30, kind: "view", presenter: p)
        let heading = NodeView(id: 31, kind: "text", presenter: p)
        heading.applyProps(set: ["text": "Legacy features", "accessibilityRole": "heading", "accessibilityHeadingLevel": "2"], clear: [])
        let count = NodeView(id: 32, kind: "text", presenter: p)
        count.applyProps(set: ["text": "3 rows"], clear: [])
        let chevron = NodeView(id: 33, kind: "image", presenter: p)
        chevron.applyProps(set: [:], clear: [])
        button.addSubview(row); row.addSubview(heading); button.addSubview(count); button.addSubview(chevron)
        let caption = NodeView(id: 34, kind: "text", presenter: p)
        caption.applyProps(set: ["text": "Dark"], clear: [])
        let icon = NodeView(id: 35, kind: "image", presenter: p)
        icon.applyProps(set: ["accessibilityLabel": "Moon"], clear: [])
        other.addSubview(caption); other.addSubview(icon)
        for n in [row, heading, count, chevron, caption, icon] { p.views[n.id] = n }
        p.syncAccessibility()

        XCTAssertEqual(button.accessibilityRole(), .button)
        XCTAssertEqual(button.accessibilityLabel(), "Legacy features 3 rows")
        XCTAssertFalse(button.isAccessibilityExpanded())
        XCTAssertTrue(button.accessibilityPerformPress())
        let children = button.accessibilityChildren() ?? []
        XCTAssertEqual(children.count, 1, "the heading, not the button's text or images")
        XCTAssertTrue(children.first as? NodeView === heading)
        XCTAssertTrue(heading.accessibilityParent() as? NodeView === button)
        XCTAssertEqual(heading.accessibilityRole()?.rawValue, "AXHeading")
        XCTAssertEqual(heading.accessibilityValue() as? Int, 2)
        XCTAssertFalse(heading.accessibilityPerformPress(), "the press is the button's")
        XCTAssertNil(other.accessibilityChildren(), "a button of text and an image is a leaf")
        heading.applyProps(set: ["accessibilityElementsHidden": "true"], clear: [])
        XCTAssertNil(button.accessibilityChildren(), "an aria-hidden heading is no child")
        withExtendedLifetime(w) {}
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
    /// LLP 1116 D8: a drawn `role="progressbar"` is AppKit's progress
    /// indicator (#278), its value `aria-valuenow` between `aria-valuemin`
    /// and `aria-valuemax` (ARIA's 0 and 100 where unsaid), `aria-valuetext`
    /// its description, and its children unread.
    func testARangeRoleIsOneElementWithItsValue() {
        let (p, w, _, _) = fixture()
        let bar = NodeView(id: 20, kind: "view", presenter: p)
        bar.frame = NSRect(x: 0, y: 100, width: 200, height: 8)
        p.root.addSubview(bar); p.views[bar.id] = bar
        let fill = NodeView(id: 21, kind: "view", presenter: p)
        fill.frame = NSRect(x: 0, y: 0, width: 60, height: 8)
        bar.addSubview(fill); p.views[fill.id] = fill
        bar.applyProps(set: ["accessibilityRole": "progressbar", "accessibilityValueNow": "30", "accessibilityLabel": "Download"], clear: [])
        XCTAssertTrue(bar.isAccessibilityElement())
        XCTAssertEqual(bar.accessibilityRole(), .progressIndicator)
        XCTAssertEqual(bar.accessibilityLabel(), "Download")
        XCTAssertEqual(bar.accessibilityValue() as? NSNumber, 30)
        XCTAssertEqual(bar.accessibilityMinValue() as? NSNumber, 0)
        XCTAssertEqual(bar.accessibilityMaxValue() as? NSNumber, 100)
        XCTAssertNil(bar.accessibilityChildren(), "its children are presentational")
        bar.applyProps(set: ["accessibilityValueNow": "2", "accessibilityValueMax": "8", "accessibilityValueText": "2 of 8 glasses"], clear: [])
        XCTAssertEqual(bar.accessibilityValue() as? NSNumber, 2)
        XCTAssertEqual(bar.accessibilityMaxValue() as? NSNumber, 8)
        XCTAssertEqual(bar.accessibilityValueDescription(), "2 of 8 glasses")
        bar.applyProps(set: [:], clear: ["accessibilityValueText"])
        XCTAssertNil(bar.accessibilityValueDescription())
        XCTAssertEqual(bar.rangeValueText, "25%", "what UIKit speaks")
        bar.applyProps(set: ["accessibilityRole": "slider"], clear: [])
        XCTAssertEqual(bar.accessibilityRole(), .slider)
        withExtendedLifetime(w) {}
    }
    private final class Opened: ExactSessionDelegate {
        var urls: [String] = []
        func exactSession(_ session: ExactSession, command name: String, args: [Any]) {
            if name == "openURL" { urls += args.compactMap { $0 as? String } }
        }
    }
    /// X78: a link carries its URL, as an `NSTextView` link run and a SwiftUI
    /// `Link` do. A `link` node's `AXURL` is its `href`. A paragraph's `href`
    /// run is one element from one ask to the next, and a hit test returns
    /// it (one made per ask is invalid to another process, VoiceOver's);
    /// new runs make a new one. Pressing either opens its URL.
    func testALinkCarriesItsURLAndAnInlineLinkIsOneElementWhileItsRunsLast() throws {
        _ = NSApplication.shared
        let session = ExactApp.shared.makeSession(label: "link-url")
        defer { session.destroy() }
        let opened = Opened()
        session.delegate = opened
        let p = session.presenter
        p.viewport.frame = NSRect(x: 0, y: 0, width: 400, height: 200)
        let w = NSWindow(contentRect: p.viewport.frame, styleMask: [.titled], backing: .buffered, defer: false)
        w.isReleasedWhenClosed = false
        w.contentView = p.viewport
        defer { w.close() }
        func runs(_ href: String) -> [String: Any] {
            func run(_ id: Int, _ props: [String: String]) -> [String: Any] { ["id": id, "parent": 3, "paint": true, "props": props, "style": ["font_size": 18.0]] }
            return ["op": "paragraph", "id": 3, "runs": [run(4, ["text": "Visit the "]), run(5, ["text": "site", "href": href]), run(6, ["text": " today"])]]
        }
        p.apply(wireBatch([
            ["op": "create", "id": 1, "kind": "view"],
            ["op": "create", "id": 2, "kind": "button", "props": ["accessibilityRole": "link", "href": "https://example.com/link-node", "text": "Example link"]],
            ["op": "create", "id": 3, "kind": "text", "style": ["font_size": 18.0]],
            runs("https://example.com/inline-run"),
            ["op": "children", "id": 1, "ids": [2, 3]], ["op": "roots", "ids": [1]],
            ["op": "frame", "id": 1, "x": 0, "y": 0, "w": 400, "h": 200],
            ["op": "frame", "id": 2, "x": 16, "y": 16, "w": 140, "h": 28],
            ["op": "frame", "id": 3, "x": 16, "y": 56, "w": 300, "h": 28],
        ]))
        p.viewport.layoutSubtreeIfNeeded()
        let link = try XCTUnwrap(p.views[2]), paragraph = try XCTUnwrap(p.views[3])
        XCTAssertEqual(link.accessibilityRole(), .link)
        XCTAssertEqual(link.accessibilityURL(), URL(string: "https://example.com/link-node"))
        let run = try XCTUnwrap(paragraph.accessibilityChildren()?.first as? NSAccessibilityElement)
        XCTAssertEqual(paragraph.accessibilityChildren()?.count, 1)
        XCTAssertTrue(paragraph.accessibilityChildren()?.first as AnyObject === run, "the same element at the next ask")
        XCTAssertEqual(run.accessibilityRole(), .link)
        XCTAssertEqual(run.accessibilityLabel(), "site")
        XCTAssertEqual(run.accessibilityURL(), URL(string: "https://example.com/inline-run"))
        let frame = run.accessibilityFrame()
        XCTAssertFalse(frame.isEmpty)
        XCTAssertTrue(paragraph.accessibilityHitTest(NSPoint(x: frame.midX, y: frame.midY)) as AnyObject === run, "a hit test over the run returns it")
        XCTAssertTrue(link.accessibilityPerformPress())
        XCTAssertTrue(run.accessibilityPerformPress())
        XCTAssertEqual(opened.urls, ["https://example.com/link-node", "https://example.com/inline-run"])
        p.apply(wireBatch([runs("https://example.com/moved")]))
        let moved = try XCTUnwrap(paragraph.accessibilityChildren()?.first as? NSAccessibilityElement)
        XCTAssertFalse(moved === run, "new runs, a new element")
        XCTAssertEqual(moved.accessibilityURL(), URL(string: "https://example.com/moved"))
        XCTAssertTrue(paragraph.accessibilityChildren()?.first as AnyObject === moved)
    }
    /// X78 with #278: a status paragraph's words and link are kept from one
    /// ask to the next like any paragraph's runs, and made again when its
    /// runs change, when it stops being a group, or (with no runs) when its
    /// text changes.
    func testAStatusParagraphKeepsItsWordsWhileItsRunsLast() throws {
        let (p, w, _, _) = fixture()
        let status = NodeView(id: 70, kind: "text", presenter: p)
        status.frame = NSRect(x: 0, y: 60, width: 300, height: 20)
        p.root.addSubview(status); p.views[status.id] = status
        func runs(_ href: String) -> [String: Any] {
            func run(_ id: Int, _ props: [String: String]) -> [String: Any] { ["id": id, "parent": 70, "paint": true, "props": props, "style": ["font_size": 16.0]] }
            return ["op": "paragraph", "id": 70, "runs": [run(71, ["text": "Archived. "]), run(72, ["text": "Undo", "href": href])]]
        }
        status.applyProps(set: ["accessibilityRole": "status"], clear: [])
        p.apply(wireBatch([runs("https://example.com/undo")]))
        func children() -> [NSAccessibilityElement] { status.accessibilityChildren()?.compactMap { $0 as? NSAccessibilityElement } ?? [] }
        let first = children()
        XCTAssertEqual(first.map { $0.accessibilityRole() }, [.staticText, .link])
        XCTAssertTrue(zip(first, children()).allSatisfy { $0 === $1 }, "the same elements at the next ask")
        p.apply(wireBatch([runs("https://example.com/moved")]))
        let moved = children()
        XCTAssertFalse(moved.first === first.first, "new runs, new elements")
        XCTAssertEqual(moved.last?.accessibilityURL(), URL(string: "https://example.com/moved"))
        status.applyProps(set: [:], clear: ["accessibilityRole"])
        XCTAssertEqual(children().map { $0.accessibilityRole() }, [.link], "no role: only its link, as any paragraph")
        p.apply(wireBatch([["op": "paragraph", "id": 70, "runs": []]]))
        status.applyProps(set: ["text": "Saved", "accessibilityRole": "status"], clear: [])
        let saved = try XCTUnwrap(children().first)
        XCTAssertTrue(children().first === saved)
        XCTAssertEqual(saved.accessibilityValue() as? String, "Saved")
        status.applyProps(set: ["text": "Synced"], clear: [])
        XCTAssertEqual(children().first?.accessibilityValue() as? String, "Synced", "its text changed: a new element")
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
    /// #278: `status` and `alert` are live regions by their roles, as WebKit
    /// serves them: a group whose subrole is the role, named only by its
    /// author, a paragraph's words a static text inside it, a container's
    /// its children; live with no `aria-live`, and `off` silences one.
    func testStatusAndAlertAreLiveRegionGroups() {
        let (p, w, _, _) = fixture()
        let stage = NodeView(id: 40, kind: "text", presenter: p)
        let error = NodeView(id: 41, kind: "view", presenter: p)
        let words = NodeView(id: 42, kind: "text", presenter: p)
        stage.frame = NSRect(x: 0, y: 60, width: 200, height: 20)
        error.frame = NSRect(x: 0, y: 90, width: 200, height: 20)
        words.frame = NSRect(x: 0, y: 0, width: 200, height: 20)
        for n in [stage, error] { p.root.addSubview(n); p.views[n.id] = n }
        error.addSubview(words); p.views[words.id] = words
        stage.applyProps(set: ["text": "Downloading", "accessibilityRole": "status"], clear: [])
        error.applyProps(set: ["accessibilityRole": "alert"], clear: [])
        words.applyProps(set: ["text": "Setup failed"], clear: [])
        p.syncAccessibility()
        XCTAssertTrue(stage.isAccessibilityElement())
        XCTAssertEqual(stage.accessibilityRole(), .group)
        XCTAssertEqual(stage.accessibilitySubrole()?.rawValue, "AXApplicationStatus")
        XCTAssertNil(stage.accessibilityLabel(), "named only by its author")
        let text = stage.accessibilityChildren()?.compactMap { $0 as? NSAccessibilityElement } ?? []
        XCTAssertEqual(text.map { $0.accessibilityRole() }, [.staticText])
        XCTAssertEqual(text.first?.accessibilityValue() as? String, "Downloading")
        XCTAssertEqual(stage.liveText, "Downloading", "live by its role")
        XCTAssertTrue(error.isAccessibilityElement())
        XCTAssertEqual(error.accessibilityRole(), .group)
        XCTAssertEqual(error.accessibilitySubrole()?.rawValue, "AXApplicationAlert")
        XCTAssertEqual(error.accessibilityChildren()?.first as? NodeView, words)
        XCTAssertEqual(error.liveText, "Setup failed")
        stage.applyProps(set: ["accessibilityLive": "off"], clear: [])
        p.syncAccessibility()
        XCTAssertNil(stage.liveText, "its aria-live over its role's")
        stage.applyProps(set: [:], clear: ["accessibilityRole", "accessibilityLive"])
        XCTAssertEqual(stage.accessibilityRole(), .staticText, "no role: a paragraph again")
        XCTAssertNil(stage.accessibilitySubrole())
        withExtendedLifetime(w) {}
    }
    /// A `dialog` element is a dialog by its implicit role, and an explicit
    /// role wins, as HTML-AAM has them.
    func testADialogElementIsADialogGroup() {
        let (p, w, _, _) = fixture()
        let dialog = NodeView(id: 44, kind: "view", presenter: p)
        dialog.frame = NSRect(x: 0, y: 60, width: 200, height: 80)
        p.root.addSubview(dialog); p.views[dialog.id] = dialog
        dialog.applyProps(set: ["semanticTag": "dialog", "accessibilityLabel": "Rename"], clear: [])
        XCTAssertTrue(dialog.isAccessibilityElement())
        XCTAssertEqual(dialog.accessibilityRole(), .group)
        XCTAssertEqual(dialog.accessibilitySubrole()?.rawValue, "AXApplicationDialog")
        XCTAssertEqual(dialog.accessibilityLabel(), "Rename")
        dialog.applyProps(set: ["accessibilityRole": "alertdialog"], clear: [])
        XCTAssertEqual(dialog.accessibilitySubrole()?.rawValue, "AXApplicationAlertDialog")
        withExtendedLifetime(w) {}
    }
    /// A role removed or changed at runtime leaves no trace of it: a box
    /// that was a status, a dialog, a group or a range is a plain view again,
    /// as a new one is, and a status that becomes a group has no subrole.
    func testARemovedRoleLeavesAPlainView() {
        let (p, w, _, _) = fixture()
        let fresh = NodeView(id: 49, kind: "view", presenter: p)
        let roles: [[String: String]] = [["accessibilityRole": "status"], ["accessibilityRole": "dialog", "accessibilityLabel": "Setup"],
                                         ["accessibilityRole": "group"], ["accessibilityRole": "progressbar", "accessibilityValueNow": "30", "accessibilityValueText": "30 files"]]
        for (i, set) in roles.enumerated() {
            let box = NodeView(id: UInt32(50 + i), kind: "view", presenter: p)
            box.frame = NSRect(x: 0, y: 30 * CGFloat(i), width: 200, height: 20)
            p.root.addSubview(box); p.views[box.id] = box
            box.applyProps(set: set, clear: [])
            p.syncAccessibility()
            XCTAssertTrue(box.isAccessibilityElement(), "\(set)")
            box.applyProps(set: [:], clear: ["accessibilityRole"])
            p.syncAccessibility()
            XCTAssertFalse(box.isAccessibilityElement(), "\(set) removed")
            XCTAssertTrue([nil, fresh.accessibilityRole()].contains(box.accessibilityRole()), "\(set) removed: AppKit's default role, or none")
            XCTAssertNil(box.accessibilitySubrole(), "\(set) removed")
            XCTAssertNil(box.accessibilityValue(), "\(set) removed")
            XCTAssertNil(box.accessibilityValueDescription(), "\(set) removed")
            XCTAssertNil(box.liveText, "\(set) removed")
        }
        let box = NodeView(id: 60, kind: "view", presenter: p)
        p.root.addSubview(box); p.views[box.id] = box
        box.applyProps(set: ["accessibilityRole": "status"], clear: [])
        box.applyProps(set: ["accessibilityRole": "group"], clear: [])
        XCTAssertEqual(box.accessibilityRole(), .group)
        XCTAssertNil(box.accessibilitySubrole(), "a status become a group")
        box.applyProps(set: ["accessibilityRole": "none"], clear: [])
        XCTAssertFalse(box.isAccessibilityElement(), "role=\"none\"")
        withExtendedLifetime(w) {}
    }
    /// An `alert` speaks as it appears, as a browser's alert event has it,
    /// except at launch; a live region otherwise only as its text changes.
    func testAnAlertSpeaksAsItAppears() {
        let (p, w, _, _) = fixture()
        let error = NodeView(id: 43, kind: "text", presenter: p)
        error.frame = NSRect(x: 0, y: 60, width: 200, height: 20)
        p.root.addSubview(error); p.views[error.id] = error
        error.applyProps(set: ["text": "Setup failed", "accessibilityRole": "alert"], clear: [])
        XCTAssertEqual(error.livePoliteness, "assertive")
        XCTAssertTrue(error.liveSpeaks("Setup failed", launching: false))
        XCTAssertFalse(error.liveSpeaks("Setup failed", launching: true), "there at launch: read with the screen")
        error.props["accessibilityRole"] = "status"
        XCTAssertEqual(error.livePoliteness, "polite")
        XCTAssertFalse(error.liveSpeaks("Setup failed", launching: false), "a status appearing is not spoken")
        p.syncAccessibility()
        XCTAssertFalse(error.liveSpeaks("Setup failed", launching: false), "unchanged")
        XCTAssertTrue(error.liveSpeaks("Unpacking", launching: false))
        XCTAssertFalse(error.liveSpeaks("", launching: false))
        error.props["accessibilityLive"] = "assertive"
        XCTAssertEqual(error.livePoliteness, "assertive", "its aria-live over its role's")
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
        p.syncAccessibility() // a later batch's sync: the restarted tree's autofocus stays consumed
        XCTAssertNil(p.focusedNode)
        withExtendedLifetime(w) {}
    }
    /// Runs the main run loop until `done` holds, for at most `timeout` seconds.
    private func spin(timeout: TimeInterval = 10, until done: () -> Bool) {
        let end = Date().addingTimeInterval(timeout)
        while !done(), Date() < end { RunLoop.main.run(mode: .default, before: Date().addingTimeInterval(0.005)) }
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
