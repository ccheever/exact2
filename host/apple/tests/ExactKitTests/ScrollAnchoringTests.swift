#if os(iOS) || os(tvOS)
import UIKit
#else
import AppKit
#endif
import XCTest
@testable import ExactKit

/// CSS Scroll Anchoring's selection and adjustment (`ScrollAnchoring.swift`,
/// #138 X23d), on a tree of rects.
final class ScrollAnchoringTests: XCTestCase {
    private final class Box {
        let name: String, rect: CGRect, children: [Box], excluded: Bool
        init(_ name: String, _ y: CGFloat, _ h: CGFloat, excluded: Bool = false, _ children: [Box] = []) {
            self.name = name; rect = CGRect(x: 0, y: y, width: 100, height: h); self.children = children; self.excluded = excluded
        }
    }
    private func select(_ boxes: [Box], port: CGRect) -> String? {
        ScrollAnchoring.select(boxes, port: port, children: { $0.children }, excluded: { $0.excluded }, rect: { $0.rect })?.name
    }
    private let port = CGRect(x: 0, y: 200, width: 100, height: 200)

    func testExamination() {
        XCTAssertEqual(ScrollAnchoring.examine(CGRect(x: 0, y: 0, width: 100, height: 48), port: port), .skip)
        XCTAssertEqual(ScrollAnchoring.examine(CGRect(x: 0, y: 250, width: 100, height: 0), port: port), .skip, "no area")
        XCTAssertEqual(ScrollAnchoring.examine(CGRect(x: 0, y: 250, width: 100, height: 30), port: port), .select)
        XCTAssertEqual(ScrollAnchoring.examine(CGRect(x: 0, y: 180, width: 100, height: 30), port: port), .descend)
    }

    func testTheFirstVisibleBoxInTreeOrderIsTheAnchor() {
        let rows = [Box("header", 0, 48), Box("a", 48, 140), Box("b", 188, 30), Box("c", 218, 30)]
        XCTAssertEqual(select(rows, port: port), "b", "a partly visible leaf is the anchor")
        XCTAssertEqual(select([Box("header", 0, 48), Box("c", 218, 30)], port: port), "c")
    }

    func testAPartlyVisibleBoxIsDescendedInto() {
        let section = Box("section", 100, 400, [Box("x", 100, 90), Box("y", 190, 30), Box("z", 220, 30)])
        XCTAssertEqual(select([section], port: port), "y")
        // Nothing inside fully or partly visible: the section itself.
        XCTAssertEqual(select([Box("section", 100, 400, [Box("x", 100, 20)])], port: port), "section")
    }

    func testExcludedSubtreesAreNotCandidates() {
        let rows = [Box("sticky", 200, 30, excluded: true), Box("row", 230, 30)]
        XCTAssertEqual(select(rows, port: port), "row")
        XCTAssertNil(select([Box("fixed", 200, 30, excluded: true)], port: port))
    }

    func testAFocusedEditableHasPriorityOverTheFirstVisibleBox() throws {
        #if os(macOS)
        _ = NSApplication.shared
        #endif
        let p = Presenter()
        let bounds = CGRect(x: 0, y: 0, width: 320, height: 400)
        #if os(iOS) || os(tvOS)
        let window = UIWindow(frame: bounds)
        window.addSubview(p.viewport)
        window.makeKeyAndVisible()
        defer { window.isHidden = true }
        #else
        let window = NSWindow(contentRect: bounds, styleMask: [.titled], backing: .buffered, defer: false)
        window.contentView = p.viewport
        window.makeKeyAndOrderFront(nil)
        defer { window.orderOut(nil) }
        #endif
        p.viewport.frame = bounds
        p.apply(wireBatch([
            ["op": "create", "id": 1, "kind": "view", "style": ["overflow_y": "scroll"]],
            ["op": "create", "id": 2, "kind": "view", "props": ["id": "first"]],
            ["op": "create", "id": 3, "kind": "input", "props": ["type": "text", "value": "Editing"]],
            ["op": "children", "id": 1, "ids": [2, 3]], ["op": "roots", "ids": [1]],
            ["op": "frame", "id": 1, "x": 0, "y": 0, "w": 300, "h": 200],
            ["op": "frame", "id": 2, "x": 0, "y": 0, "w": 300, "h": 100],
            ["op": "frame", "id": 3, "x": 0, "y": 120, "w": 200, "h": 40],
            ["op": "content", "id": 1, "w": 300, "h": 600],
        ]))
        let scroller = try XCTUnwrap(p.views[1]), field = try XCTUnwrap(p.views[3])
        #if os(iOS) || os(tvOS)
        p.focusNode(field)
        #else
        window.makeFirstResponder(field.field)
        #endif
        XCTAssertEqual(p.focusedNode?.id, 3)
        #if os(iOS) || os(tvOS)
        let space = try XCTUnwrap(scroller.scrollView)
        #else
        let space = try XCTUnwrap(scroller.scroll?.documentView)
        #endif
        let port = CGRect(x: 0, y: 50, width: 300, height: 200)
        func anchor() -> UInt32? { ScrollAnchoring.anchor(in: scroller, space: space, port: port)?.node.id }
        XCTAssertEqual(anchor(), 3, "the focused editor wins over box 2")
        p.apply(wireBatch([["op": "props", "id": 3, "set": ["editable": "false"]]]))
        XCTAssertEqual(anchor(), 2, "a read-only editor is no priority candidate")
        p.apply(wireBatch([["op": "props", "id": 3, "clear": ["editable"]],
                           ["op": "frame", "id": 3, "x": 0, "y": 350, "w": 200, "h": 40]]))
        XCTAssertEqual(anchor(), 2, "an offscreen editor is no priority candidate")
    }

    func testTheAdjustmentFollowsTheAnchorAndClamps() {
        XCTAssertEqual(ScrollAnchoring.adjusted(top: 200, before: 198, after: 150, minimum: 0, maximum: 1000), 152)
        XCTAssertEqual(ScrollAnchoring.adjusted(top: 200, before: 198, after: 400, minimum: 0, maximum: 300), 300)
        XCTAssertEqual(ScrollAnchoring.adjusted(top: 20, before: 100, after: 0, minimum: 0, maximum: 300), 0)
    }

    func testOnlyLayoutPropertiesSuppress() {
        var changes = ScrollAnchoring.Changes()
        changes.note(1, from: ["background_color": "#fff"], to: ["background_color": "#000"], popover: false)
        XCTAssertTrue(changes.layout.isEmpty)
        changes.note(2, from: ["height": 48], to: ["height": 0], popover: false)
        changes.note(3, from: [:], to: ["position_type": "absolute"], popover: false)
        // A popover lays out absolute unless its author made it sticky (the
        // kernel's top layer): unsticking one takes it out of flow; making
        // one relative doesn't, nor dropping a sticky no author wrote.
        changes.note(4, from: ["position_type": "sticky", "written": ["position_type"]], to: [:], popover: true)
        changes.note(5, from: [:], to: ["position_type": "relative"], popover: true)
        changes.note(6, from: ["position_type": "sticky"], to: [:], popover: true)
        XCTAssertEqual(changes.layout, [2, 3, 4, 5, 6])
        XCTAssertEqual(changes.positioned, [3, 4])
        changes.reset()
        XCTAssertTrue(changes.layout.isEmpty && changes.positioned.isEmpty)
    }
}
