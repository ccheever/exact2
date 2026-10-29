#if os(macOS)
import AppKit
import XCTest
@testable import ExactKit

/// `tree --ax` on AppKit (LLP 1080.002 §3 stage 3): the unignored tree in
/// navigation order, AppKit's own roles and names, each element joined to
/// its view. Run by `bun host/apple/build.mjs --test`.
private extension NSView {
    nonisolated(unsafe) static var hiddenGetterCalls = 0
    /// Exchanged with `isAccessibilityHidden()`; the call inside is the original.
    @objc func countingIsAccessibilityHidden() -> Bool {
        NSView.hiddenGetterCalls += 1
        return countingIsAccessibilityHidden()
    }
}

final class AccessibilityTreeMacTests: XCTestCase {
    private var window: NSWindow!

    func testToggleButtonsReportFalseTrueAndMixedFromThePlatform() throws {
        let p = try fixture()
        for value in ["false", "true", "mixed"] {
            p.apply(wireBatch([["op": "props", "id": 2, "set": ["accessibilityPressed": value]]]))
            p.syncAccessibility()
            let ax = p.axElements(roots: [p.viewport])
            let button = try XCTUnwrap(elements(ax).first { $0["testId"] as? String == "named" })
            XCTAssertEqual(button["role"] as? String, "button")
            let states = try XCTUnwrap(button["states"] as? [String: Any])
            if value == "mixed" { XCTAssertEqual(states["pressed"] as? String, "mixed") }
            else { XCTAssertEqual(states["pressed"] as? Bool, value == "true") }
            XCTAssertNil(states["checked"])
            XCTAssertTrue((ax["observes"] as? [String] ?? []).contains("pressed"))
        }
        p.apply(wireBatch([["op": "props", "id": 2, "clear": ["accessibilityPressed"]]]))
        let ax = p.axElements(roots: [p.viewport])
        let button = try XCTUnwrap(elements(ax).first { $0["testId"] as? String == "named" })
        XCTAssertNil((button["states"] as? [String: Any])?["pressed"])
    }

    /// `painted`: a dialog and a popover in the page's top layer, the
    /// driver's own chrome, which is what the page's own projection of a
    /// modal is about; the last case here holds a dialog in AppKit's sheet.
    private func fixture(painted: Bool = true) throws -> Presenter {
        _ = NSApplication.shared
        let p = Presenter()
        p.dialogs.painted = painted
        p.menus.painted = painted
        window = NSWindow(contentRect: NSRect(x: 0, y: 0, width: 400, height: 300), styleMask: [.titled], backing: .buffered, defer: false)
        window.contentView = p.viewport
        p.apply(wireBatch([
            ["op": "create", "id": 1, "kind": "view"],
            ["op": "create", "id": 2, "kind": "button", "handlers": ["press"], "props": ["testId": "named", "accessibilityLabel": "Play"]],
            ["op": "create", "id": 3, "kind": "button", "handlers": ["press"], "props": ["testId": "bare"]],
            ["op": "children", "id": 1, "ids": [2, 3]],
            ["op": "roots", "ids": [1]],
            ["op": "frame", "id": 1, "x": 0.0, "y": 0.0, "w": 400.0, "h": 300.0],
            ["op": "frame", "id": 2, "x": 0.0, "y": 0.0, "w": 100.0, "h": 40.0],
            ["op": "frame", "id": 3, "x": 0.0, "y": 50.0, "w": 40.0, "h": 40.0],
        ]))
        p.syncAccessibility()
        return p
    }
    private func elements(_ ax: [String: Any]) -> [[String: Any]] { ax["elements"] as? [[String: Any]] ?? [] }

    func testViewportWalkOmitsHiddenAndInertSubtreesAndNamesRestoredCells() throws {
        let p = try fixture()
        p.apply(wireBatch([
            ["op": "create", "id": 20, "kind": "view", "props": ["accessibilityElementsHidden": "true"]],
            ["op": "create", "id": 21, "kind": "control", "props": ["type": "checkbox", "accessibilityLabel": "Hidden checkbox", "testId": "hidden-check"], "handlers": ["change"]],
            ["op": "create", "id": 22, "kind": "view", "props": ["inert": "true"]],
            ["op": "create", "id": 23, "kind": "input", "props": ["type": "password", "accessibilityLabel": "Password", "testId": "inert-password"]],
            ["op": "children", "id": 20, "ids": [21]],
            ["op": "children", "id": 22, "ids": [23]],
            ["op": "children", "id": 1, "ids": [2, 3, 20, 22]],
            ["op": "frame", "id": 20, "x": 0, "y": 100, "w": 200, "h": 40],
            ["op": "frame", "id": 21, "x": 0, "y": 0, "w": 100, "h": 30],
            ["op": "frame", "id": 22, "x": 0, "y": 150, "w": 200, "h": 40],
            ["op": "frame", "id": 23, "x": 0, "y": 0, "w": 100, "h": 30],
        ]))
        func named(_ name: String) -> [String: Any]? {
            elements(p.axElements(roots: [p.viewport])).first { $0["testId"] as? String == name }
        }
        XCTAssertNil(named("hidden-check"))
        XCTAssertNil(named("inert-password"))
        p.apply(wireBatch([
            ["op": "props", "id": 20, "clear": ["accessibilityElementsHidden"]],
            ["op": "props", "id": 22, "clear": ["inert"]],
        ]))
        XCTAssertEqual(named("hidden-check")?["name"] as? String, "Hidden checkbox")
        XCTAssertEqual(named("inert-password")?["name"] as? String, "Password")
    }

    /// From `start` (a view, or an element such as a field's cell) up to the viewport, each parent serves its child.
    private func assertModalParentChain(_ start: Any, presenter p: Presenter, file: StaticString = #filePath, line: UInt = #line) throws {
        var child = start as AnyObject
        var visited: Set<ObjectIdentifier> = []
        while child !== p.viewport {
            guard visited.insert(ObjectIdentifier(child)).inserted else { return XCTFail("parent cycle", file: file, line: line) }
            guard visited.count < 20 else { return XCTFail("parent chain never reaches viewport", file: file, line: line) }
            let element = try XCTUnwrap(child as? NSAccessibilityProtocol, file: file, line: line)
            let parent = try XCTUnwrap(element.accessibilityParent() as? NSView, file: file, line: line)
            XCTAssertTrue((parent.accessibilityChildren() ?? []).contains { ($0 as AnyObject) === child }, "AXParent points back through AXChildren", file: file, line: line)
            XCTAssertTrue(((child as? NSObject)?.accessibilityAttributeValue(.parent) as? NSView) === parent, "legacy AXParent agrees", file: file, line: line)
            XCTAssertFalse(elements(p.axElements(roots: [parent])).contains { $0["testId"] as? String == "named" }, "moving to a container cannot reach Play", file: file, line: line)
            child = parent
        }
    }

    func testElementsAreAppKitsUnignoredTreeJoinedToTheirViews() throws {
        let p = try fixture()
        let ax = p.axElements(roots: [p.viewport])
        XCTAssertEqual(ax["source"] as? String, "appkit")
        XCTAssertEqual(ax["order"] as? String, "navigation")
        let named = try XCTUnwrap(elements(ax).first { $0["testId"] as? String == "named" })
        XCTAssertEqual(named["role"] as? String, "button")
        XCTAssertEqual((named["native"] as? [String: Any])?["role"] as? String, "AXButton")
        XCTAssertEqual(named["name"] as? String, "Play")
        XCTAssertEqual(named["via"] as? String, "self")
        XCTAssertNotNil(named["frame"])
        let order = elements(ax).compactMap { $0["testId"] as? String }
        XCTAssertEqual(order.firstIndex(of: "named").map { $0 < (order.firstIndex(of: "bare") ?? 0) }, true)
    }

    /// A control's accessibility is its cell's: the walk reads cells, joins
    /// them to the control's view, and omits a secure field's value.
    func testCellsAreWalkedAndASecureValueIsOmitted() throws {
        let p = try fixture()
        let field = NSSecureTextField(frame: NSRect(x: 0, y: 100, width: 100, height: 24))
        field.stringValue = "hunter2"
        let check = NSButton(checkboxWithTitle: "Checked", target: nil, action: nil)
        check.frame = NSRect(x: 0, y: 140, width: 100, height: 20)
        check.state = .on
        p.views[1]?.addSubview(field)
        p.views[1]?.addSubview(check)
        let all = elements(p.axElements(roots: [p.viewport]))
        let secure = try XCTUnwrap(all.first { ($0["states"] as? [String: Any])?["protected"] as? Bool == true }, "\(all.map { $0["native"] ?? "" })")
        XCTAssertNil(secure["value"])
        XCTAssertEqual(secure["id"] as? UInt32, 1)
        let box = try XCTUnwrap(all.first { $0["role"] as? String == "checkbox" }, "\(all.map { $0["native"] ?? "" })")
        XCTAssertEqual((box["states"] as? [String: Any])?["checked"] as? Bool, true)
    }

    /// A box whose ARIA role is `button` is one, as the web's tree has it
    /// (chat F14); an empty date reads empty and shows its author's
    /// `placeholder` or no words (LLP 1115.000 D13.1), never the picker's
    /// 1/1/2001 (kanban F23) and never a browser's `mm/dd/yyyy`.
    func testARoleButtonBoxIsAButtonAndAnEmptyDateReadsEmpty() throws {
        let p = try fixture()
        p.apply(wireBatch([
            ["op": "create", "id": 4, "kind": "view", "handlers": ["press"],
             "props": ["testId": "bubble", "accessibilityRole": "button", "accessibilityLabel": "Message"]],
            ["op": "children", "id": 1, "ids": [2, 3, 4]],
            ["op": "frame", "id": 4, "x": 0.0, "y": 100.0, "w": 100.0, "h": 40.0],
        ]))
        p.syncAccessibility()
        let bubble = try XCTUnwrap(elements(p.axElements(roots: [p.viewport])).first { $0["testId"] as? String == "bubble" })
        XCTAssertEqual(bubble["role"] as? String, "button")
        XCTAssertEqual(bubble["name"] as? String, "Message")
        let date = DateField()
        date.empty = true
        XCTAssertEqual(date.accessibilityValue() as? String, "")
        p.apply(wireBatch([
            ["op": "create", "id": 5, "kind": "control", "props": ["type": "date", "value": ""], "handlers": ["change"], "style": [:]],
            ["op": "create", "id": 6, "kind": "control", "props": ["type": "time", "value": "", "placeholder": "Any time"], "handlers": ["change"], "style": [:]],
            ["op": "children", "id": 1, "ids": [2, 3, 4, 5, 6]],
            ["op": "frame", "id": 5, "x": 0.0, "y": 150.0, "w": 140.0, "h": 24.0],
            ["op": "frame", "id": 6, "x": 0.0, "y": 180.0, "w": 140.0, "h": 24.0],
        ]))
        let bare = try XCTUnwrap(p.controls.controls[5] as? DateField), said = try XCTUnwrap(p.controls.controls[6] as? DateField)
        XCTAssertTrue(bare.empty && said.empty)
        XCTAssertEqual(bare.placeholder, "", "with no placeholder, no words: not a date format")
        XCTAssertEqual(said.placeholder, "Any time", "the author's placeholder")
    }

    /// A date shows the person's choice until its bound value changes, as the
    /// web build's input does: an action that only sent a mutation left the
    /// picker cleared, and the agent's `type` refused (x2apps kanban2 #5).
    func testADateKeepsTheChoiceUntilItsBoundValueChanges() throws {
        let p = try fixture()
        p.apply(wireBatch([
            ["op": "create", "id": 10, "kind": "control", "props": ["type": "date", "value": "", "testId": "due"], "handlers": ["change"], "style": [:]],
            ["op": "children", "id": 1, "ids": [2, 3, 10]],
            ["op": "frame", "id": 10, "x": 0.0, "y": 100.0, "w": 140.0, "h": 24.0],
        ]))
        var sent: [String] = []
        p.onControlValue = { _, value, _, change in if change { sent.append(value) } }
        let field = try XCTUnwrap(p.controls.controls[10] as? DateField)
        XCTAssertTrue(field.empty)
        let reply = try XCTUnwrap(p.controls.type(try XCTUnwrap(p.views[10]), "2026-06-01"))
        XCTAssertNil(reply["error"], "\(reply)")
        XCTAssertEqual(reply["value"] as? String, "2026-06-01")
        XCTAssertEqual(sent, ["2026-06-01"])
        XCTAssertFalse(field.empty, "the bound value is still empty: the choice stays")
        p.apply(wireBatch([["op": "props", "id": 10, "set": ["min": "2026-01-01"]]]))
        XCTAssertEqual(p.controls.shownDate(field, "date"), "2026-06-01", "another prop leaves it")
        p.apply(wireBatch([["op": "props", "id": 10, "set": ["value": "2026-07-04"]]]))
        XCTAssertEqual(p.controls.shownDate(field, "date"), "2026-07-04", "the bound value, once it changes")
        p.apply(wireBatch([["op": "props", "id": 10, "set": ["value": ""]]]))
        XCTAssertTrue(field.empty)
    }

    /// A range keeps the person's value until its bound value changes, as the
    /// web build's does (LLP 1069.001 D4, amended 2026-10-04): it snapped back
    /// to 10 after an action that wrote something else.
    func testARangeKeepsThePersonsValueUntilItsBoundValueChanges() throws {
        let p = try fixture()
        p.apply(wireBatch([
            ["op": "create", "id": 11, "kind": "control", "props": ["type": "range", "value": "10", "testId": "level"], "handlers": ["change"], "style": [:]],
            ["op": "children", "id": 1, "ids": [2, 3, 11]],
            ["op": "frame", "id": 11, "x": 0.0, "y": 140.0, "w": 129.0, "h": 24.0],
        ]))
        p.onControlValue = { _, _, _, _ in }
        let slider = try XCTUnwrap(p.controls.controls[11] as? NSSlider)
        XCTAssertEqual(slider.doubleValue, 10)
        let reply = try XCTUnwrap(p.controls.type(try XCTUnwrap(p.views[11]), "70"))
        XCTAssertEqual(reply["value"] as? String, "70")
        XCTAssertEqual(slider.doubleValue, 70, "the bound value is still 10: the knob stays")
        p.apply(wireBatch([["op": "props", "id": 11, "set": ["max": "90"]]]))
        XCTAssertEqual(slider.doubleValue, 70, "another prop leaves it")
        p.apply(wireBatch([["op": "props", "id": 11, "set": ["value": "40"]]]))
        XCTAssertEqual(slider.doubleValue, 40, "the bound value, once it changes")
    }

    /// An attached sheet is the modal: the driver judges everything else against it.
    func testASheetRootIsReportedAsTheModal() throws {
        let p = try fixture()
        let sheet = NSView(frame: NSRect(x: 0, y: 0, width: 200, height: 100))
        let done = NSButton(title: "Done", target: nil, action: nil)
        sheet.addSubview(done)
        let ax = p.axElements(roots: [p.viewport, sheet], modalRoot: sheet)
        let modal = try XCTUnwrap(ax["modal"] as? [String: Any])
        XCTAssertEqual([modal["present"] as? Bool, modal["by"] as? String == "sheet"], [true, true])
        let index = try XCTUnwrap(modal["element"] as? Int)
        XCTAssertTrue(elements(ax).contains { $0["parent"] as? Int == index && $0["name"] as? String == "Done" })
        XCTAssertTrue(elements(ax).contains { $0["testId"] as? String == "named" }, "the window behind stays exposed, for the driver's outside-modal")
    }

    func testAnAuthoredModalProjectsOnlyItsSubtree() throws {
        let p = try fixture()
        p.apply(wireBatch([
            ["op": "props", "id": 1, "set": ["accessibilityRole": "group", "accessibilityLabel": "Page"]],
            ["op": "create", "id": 10, "kind": "view", "props": ["accessibilityRole": "dialog", "accessibilityModal": "true", "accessibilityLabel": "Setup", "testId": "dlg"]],
            ["op": "create", "id": 11, "kind": "button", "handlers": ["press"], "props": ["accessibilityLabel": "OK", "testId": "ok"]],
            ["op": "children", "id": 10, "ids": [11]],
            ["op": "children", "id": 1, "ids": [2, 3, 10]],
            ["op": "frame", "id": 10, "x": 0.0, "y": 100.0, "w": 200.0, "h": 100.0],
            ["op": "frame", "id": 11, "x": 0.0, "y": 0.0, "w": 80.0, "h": 30.0],
        ]))
        p.syncAccessibility()
        let ax = p.axElements(roots: [p.viewport])
        let modal = try XCTUnwrap(ax["modal"] as? [String: Any])
        XCTAssertEqual(modal["present"] as? Bool, true)
        XCTAssertEqual(modal["by"] as? String, "aria-modal")
        XCTAssertEqual(modal["id"] as? UInt32, 10)
        let all = elements(ax)
        XCTAssertTrue(all.contains { $0["testId"] as? String == "dlg" && $0["role"] as? String == "dialog" })
        XCTAssertTrue(all.contains { $0["testId"] as? String == "ok" })
        XCTAssertFalse(all.contains { $0["testId"] as? String == "named" }, "the page behind the dialog is hidden")
        try assertModalParentChain(XCTUnwrap(p.views[11]), presenter: p)

        p.apply(wireBatch([["op": "props", "id": 10, "set": ["accessibilityModal": "false"]]]))
        p.syncAccessibility(changed: [10])
        let unmodal = p.axElements(roots: [p.viewport])
        XCTAssertEqual((unmodal["modal"] as? [String: Any])?["present"] as? Bool, false)
        XCTAssertTrue(elements(unmodal).contains { $0["testId"] as? String == "named" })
        XCTAssertTrue((p.views[10]?.accessibilityParent() as? NSView) === p.views[1], "clearing modality restores the authored parent")

        p.dialogs.show(try XCTUnwrap(p.views[10]))
        let shown = p.axElements(roots: [p.viewport])
        XCTAssertEqual((shown["modal"] as? [String: Any])?["by"] as? String, "showModal")
        XCTAssertFalse(elements(shown).contains { $0["testId"] as? String == "named" })
        p.dialogs.close(try XCTUnwrap(p.views[10]))
        XCTAssertTrue(elements(p.axElements(roots: [p.viewport])).contains { $0["testId"] as? String == "named" })
    }

    /// AppKit drops its reply to another process (VoiceOver) when the code
    /// serving the request calls `isAccessibilityHidden()`: the window reads
    /// as an empty scroll area. The projection reads the host's own flag.
    func testTheModalProjectionCallsNoAccessibilityHiddenGetter() throws {
        let p = try fixture()
        p.apply(wireBatch([
            ["op": "create", "id": 9, "kind": "view"],
            ["op": "create", "id": 10, "kind": "view", "props": ["accessibilityRole": "alertdialog", "accessibilityModal": "true", "accessibilityLabel": "Confirm"]],
            ["op": "create", "id": 11, "kind": "button", "handlers": ["press"], "props": ["accessibilityLabel": "OK"]],
            ["op": "children", "id": 10, "ids": [11]],
            ["op": "children", "id": 9, "ids": [10]],
            ["op": "children", "id": 1, "ids": [2, 3, 9]],
            ["op": "frame", "id": 9, "x": 0.0, "y": 100.0, "w": 200.0, "h": 100.0],
            ["op": "frame", "id": 10, "x": 0.0, "y": 0.0, "w": 200.0, "h": 100.0],
            ["op": "frame", "id": 11, "x": 0.0, "y": 0.0, "w": 80.0, "h": 30.0],
        ]))
        p.syncAccessibility()
        let dialog = try XCTUnwrap(p.views[10]), ok = try XCTUnwrap(p.views[11]), wrapper = try XCTUnwrap(p.views[9])
        let getter = try XCTUnwrap(class_getInstanceMethod(NSView.self, #selector(NSView.isAccessibilityHidden)))
        let counting = try XCTUnwrap(class_getInstanceMethod(NSView.self, #selector(NSView.countingIsAccessibilityHidden)))
        method_exchangeImplementations(getter, counting)
        NSView.hiddenGetterCalls = 0
        let children = p.viewport.accessibilityChildren() as? [NSView]
        let contents = p.viewport.accessibilityAttributeValue(.contents) as? [NSView]
        let parent = dialog.accessibilityAttributeValue(.parent) as? NSView
        _ = ok.accessibilityParent()
        method_exchangeImplementations(getter, counting)
        XCTAssertEqual(NSView.hiddenGetterCalls, 0, "serving AXChildren, AXContents and AXParent calls no isAccessibilityHidden()")
        XCTAssertTrue(children?.count == 1 && children?.first === dialog, "the viewport's child is exactly the modal")
        XCTAssertTrue(contents?.count == 1 && contents?.first === dialog)
        XCTAssertTrue(parent === p.viewport)
        wrapper.setAccessibilityHidden(true)
        XCTAssertNil(p.macAccessibilityModal, "an ancestor the host hid from accessibility still hides the modal")
        wrapper.setAccessibilityHidden(false)
        XCTAssertTrue(p.macAccessibilityModal === dialog)
    }

    func testModalProjectsAXContentsAsWellAsNavigationChildren() throws {
        let p = try fixture()
        p.apply(wireBatch([
            ["op": "create", "id": 10, "kind": "view", "props": ["accessibilityRole": "dialog", "accessibilityModal": "true", "accessibilityLabel": "Setup"]],
            ["op": "children", "id": 1, "ids": [2, 3, 10]],
            ["op": "frame", "id": 10, "x": 0.0, "y": 100.0, "w": 200.0, "h": 100.0],
        ]))
        p.syncAccessibility()
        let dialog = try XCTUnwrap(p.views[10])
        let contents = try XCTUnwrap(p.viewport.accessibilityContents())
        XCTAssertTrue(contents.contains { ($0 as? NSView) === dialog }, "AXContents includes the modal")
        XCTAssertFalse(contents.contains { ($0 as? NSView) === p.views[2] }, "AXContents excludes the page button")
        for attribute in [NSAccessibility.Attribute.children, .contents] {
            let served = try XCTUnwrap(p.viewport.accessibilityAttributeValue(attribute) as? [Any])
            XCTAssertTrue(served.contains { ($0 as? NSView) === dialog }, "\(attribute.rawValue) includes the modal")
            XCTAssertFalse(served.contains { ($0 as? NSView) === p.views[2] }, "\(attribute.rawValue) excludes the page button")
        }
    }

    func testModalKeepsItsPresentedPopoverFieldInTheAccessibilityTree() throws {
        let p = try fixture()
        p.apply(wireBatch([
            ["op": "create", "id": 10, "kind": "view", "props": ["accessibilityRole": "dialog", "accessibilityModal": "true", "accessibilityLabel": "Setup", "testId": "dlg"]],
            ["op": "create", "id": 11, "kind": "button", "handlers": ["press"], "props": ["accessibilityLabel": "Options", "testId": "open", "popovertarget": "options"]],
            ["op": "create", "id": 12, "kind": "view", "props": ["popover": "manual", "id": "options", "testId": "pop"]],
            ["op": "create", "id": 13, "kind": "input", "props": ["accessibilityLabel": "Filter", "testId": "filter"]],
            ["op": "children", "id": 12, "ids": [13]],
            ["op": "children", "id": 10, "ids": [11, 12]],
            ["op": "children", "id": 1, "ids": [2, 3, 10]],
            ["op": "frame", "id": 10, "x": 0.0, "y": 100.0, "w": 200.0, "h": 100.0],
            ["op": "frame", "id": 11, "x": 0.0, "y": 0.0, "w": 80.0, "h": 30.0],
            ["op": "frame", "id": 12, "x": 0.0, "y": 30.0, "w": 180.0, "h": 60.0],
            ["op": "frame", "id": 13, "x": 0.0, "y": 0.0, "w": 140.0, "h": 24.0],
        ]))
        let dialog = try XCTUnwrap(p.views[10]), pop = try XCTUnwrap(p.views[12]), button = try XCTUnwrap(p.views[11])
        p.menus.show(pop, from: button)
        XCTAssertTrue(p.menus.isOpen(pop))
        XCTAssertTrue(p.menus.contains(dialog, pop))
        p.syncAccessibility()
        let all = elements(p.axElements(roots: [p.viewport]))
        XCTAssertTrue(all.contains { $0["testId"] as? String == "filter" }, "the presented field stays reachable")
        XCTAssertFalse(all.contains { $0["testId"] as? String == "named" })
        let ax = p.axElements(roots: [p.viewport])
        let modal = try XCTUnwrap(ax["modal"] as? [String: Any])
        let presented = try XCTUnwrap(modal["presented"] as? [Int])
        let field = try XCTUnwrap(elements(ax).first { $0["testId"] as? String == "filter" })
        XCTAssertTrue(presented.contains(try XCTUnwrap(field["i"] as? Int)), "the driver can include this field in the modal boundary")
        // The popover has no role: it is no element, and its field is served in its place.
        XCTAssertFalse(pop.isAccessibilityElement())
        let filter = try XCTUnwrap(NSAccessibility.unignoredChildren(from: [pop]).first as AnyObject?)
        XCTAssertTrue((filter as? NSCell)?.controlView?.superview === p.views[13], "the element is the Filter field's")
        let contents = try XCTUnwrap(p.viewport.accessibilityContents())
        XCTAssertTrue(contents.contains { ($0 as AnyObject) === filter })
        XCTAssertFalse(contents.contains { ($0 as? NSView) === pop })
        XCTAssertFalse(contents.contains { ($0 as? NSView) === p.views[2] })
        try assertModalParentChain(filter, presenter: p)
    }

    func testExitingAndTransparentSiblingsDoNotCoverAnAuthoredModal() throws {
        let p = try fixture()
        p.apply(wireBatch([
            ["op": "create", "id": 10, "kind": "view", "props": ["accessibilityRole": "dialog", "accessibilityModal": "true", "accessibilityLabel": "Behind", "testId": "behind"]],
            ["op": "create", "id": 11, "kind": "view", "props": ["accessibilityRole": "dialog", "accessibilityModal": "true", "accessibilityLabel": "Front", "testId": "front"]],
            ["op": "children", "id": 1, "ids": [2, 3, 10, 11]],
            ["op": "frame", "id": 10, "x": 0.0, "y": 100.0, "w": 200.0, "h": 100.0],
            ["op": "frame", "id": 11, "x": 0.0, "y": 100.0, "w": 200.0, "h": 100.0],
        ]))
        p.syncAccessibility()
        XCTAssertEqual(p.macAccessibilityModal?.id, 11)
        p.beginExit(11)
        p.syncAccessibility()
        XCTAssertEqual(p.macAccessibilityModal?.id, 10, "the exiting paint ghost covers nothing")
        XCTAssertFalse(elements(p.axElements(roots: [p.viewport])).contains { $0["testId"] as? String == "named" })
        XCTAssertTrue(p.endExit(11))
        p.apply(wireBatch([
            ["op": "create", "id": 12, "kind": "view"],
            ["op": "children", "id": 1, "ids": [2, 3, 10, 12]],
            ["op": "frame", "id": 12, "x": 0.0, "y": 100.0, "w": 200.0, "h": 100.0],
        ]))
        try XCTUnwrap(p.views[12]).alphaValue = 0
        XCTAssertEqual(p.macAccessibilityModal?.id, 10, "a transparent sibling covers nothing")
    }

    func testOnlyAnOpaqueFullCoverDisqualifiesAnAuthoredModal() throws {
        let p = try fixture()
        p.apply(wireBatch([
            ["op": "create", "id": 10, "kind": "view", "props": ["accessibilityRole": "dialog", "accessibilityModal": "true"]],
            ["op": "create", "id": 12, "kind": "view"],
            ["op": "children", "id": 1, "ids": [2, 3, 10, 12]],
            ["op": "frame", "id": 10, "x": 0.0, "y": 100.0, "w": 200.0, "h": 100.0],
            ["op": "frame", "id": 12, "x": 180.0, "y": 180.0, "w": 40.0, "h": 40.0],
        ]))
        XCTAssertEqual(p.macAccessibilityModal?.id, 10, "an empty overlapping corner covers nothing")
        p.apply(wireBatch([["op": "frame", "id": 12, "x": 0.0, "y": 100.0, "w": 200.0, "h": 100.0]]))
        XCTAssertEqual(p.macAccessibilityModal?.id, 10, "even a full-size transparent background covers nothing")
        p.apply(wireBatch([["op": "style", "id": 12, "style": ["background_color": [0, 0, 0, 255], "visibility": "hidden"]]]))
        XCTAssertTrue(try XCTUnwrap(p.views[12]).cssVisibilityHidden)
        XCTAssertEqual(p.macAccessibilityModal?.id, 10, "visibility:hidden is not a cover")
        XCTAssertFalse(elements(p.axElements(roots: [p.viewport])).contains { $0["testId"] as? String == "named" })
        p.apply(wireBatch([["op": "style", "id": 12, "style": ["background_color": [0, 0, 0, 255], "visibility": "visible"]]]))
        XCTAssertNil(p.macAccessibilityModal, "a fully opaque painted cover hides the dialog")
        p.apply(wireBatch([["op": "frame", "id": 12, "x": 180.0, "y": 180.0, "w": 40.0, "h": 40.0]]))
        XCTAssertEqual(p.macAccessibilityModal?.id, 10, "a painted corner leaves the dialog exposed")
    }

    func testModalIncludesSiblingPopoversThroughTheirInvokerChain() throws {
        let p = try fixture()
        p.apply(wireBatch([
            ["op": "create", "id": 10, "kind": "view", "props": ["accessibilityRole": "dialog", "accessibilityModal": "true"]],
            ["op": "create", "id": 11, "kind": "button", "props": ["accessibilityLabel": "Options", "popovertarget": "options"]],
            ["op": "create", "id": 12, "kind": "view", "props": ["popover": "manual", "id": "options"]],
            ["op": "create", "id": 13, "kind": "input", "props": ["accessibilityLabel": "Filter", "testId": "filter"]],
            ["op": "create", "id": 14, "kind": "button", "props": ["accessibilityLabel": "More", "popovertarget": "more"]],
            ["op": "create", "id": 15, "kind": "view", "props": ["popover": "manual", "id": "more"]],
            ["op": "create", "id": 16, "kind": "input", "props": ["accessibilityLabel": "Detail", "testId": "detail"]],
            ["op": "create", "id": 17, "kind": "view", "props": ["popover": "manual", "id": "other"]],
            ["op": "create", "id": 18, "kind": "input", "props": ["accessibilityLabel": "Unrelated", "testId": "other"]],
            ["op": "children", "id": 10, "ids": [11]],
            ["op": "children", "id": 12, "ids": [13, 14]],
            ["op": "children", "id": 15, "ids": [16]],
            ["op": "children", "id": 17, "ids": [18]],
            ["op": "children", "id": 1, "ids": [2, 3, 10, 12, 15, 17]],
            ["op": "frame", "id": 10, "x": 0.0, "y": 100.0, "w": 200.0, "h": 100.0],
        ] + (11...18).map { ["op": "frame", "id": $0, "x": 0.0, "y": 0.0, "w": 140.0, "h": 30.0] }))
        let dialog = try XCTUnwrap(p.views[10]), pop = try XCTUnwrap(p.views[12]), more = try XCTUnwrap(p.views[15])
        p.menus.show(try XCTUnwrap(p.views[17]), from: try XCTUnwrap(p.views[2]))
        p.menus.show(pop, from: try XCTUnwrap(p.views[11]))
        p.menus.show(more, from: try XCTUnwrap(p.views[14]))
        XCTAssertTrue(p.menus.isOpen(pop) && p.menus.isOpen(more))
        XCTAssertFalse(p.menus.contains(dialog, pop), "the popover is authored beside the dialog")
        let ax = p.axElements(roots: [p.viewport]), all = elements(ax)
        let presented = try XCTUnwrap((ax["modal"] as? [String: Any])?["presented"] as? [Int])
        for id in ["filter", "detail"] {
            let field = try XCTUnwrap(all.first { $0["testId"] as? String == id })
            XCTAssertTrue(presented.contains(try XCTUnwrap(field["i"] as? Int)))
        }
        XCTAssertFalse(all.contains { ["named", "other"].contains($0["testId"] as? String ?? "") })
        // Neither popover has a role: each is served by its elements, whose parent is the page's scroll view.
        for popover in [pop, more] {
            for element in NSAccessibility.unignoredChildren(from: [popover]) {
                try assertModalParentChain(element, presenter: p)
            }
        }
        p.menus.close(more)
        p.menus.close(pop)
        XCTAssertFalse(elements(p.axElements(roots: [p.viewport])).contains { $0["testId"] as? String == "filter" })
    }

    func testHiddenShowModalDialogFallsBackToThePage() throws {
        let p = try fixture()
        p.apply(wireBatch([
            ["op": "create", "id": 10, "kind": "view", "props": ["semanticTag": "dialog", "accessibilityLabel": "Setup", "testId": "dlg"]],
            ["op": "children", "id": 1, "ids": [2, 3, 10]],
            ["op": "frame", "id": 10, "x": 0.0, "y": 100.0, "w": 200.0, "h": 100.0],
        ]))
        let dialog = try XCTUnwrap(p.views[10])
        p.dialogs.show(dialog)
        p.apply(wireBatch([["op": "props", "id": 10, "set": ["accessibilityElementsHidden": "true"]]]))
        p.syncAccessibility()
        XCTAssertNil(p.macAccessibilityModal)
        XCTAssertTrue(try XCTUnwrap(p.views[2]).inert, "the open dialog still contains input")
        XCTAssertFalse(try XCTUnwrap(p.views[2]).isAccessibilityHidden(), "the page's control is restored for VoiceOver")
        XCTAssertTrue(elements(p.axElements(roots: [p.viewport])).contains { $0["testId"] as? String == "named" })
        XCTAssertTrue((p.viewport.accessibilityContents() ?? []).contains { ($0 as? NSView) === p.views[2] })
        let row = try XCTUnwrap(p.views[2])
        row.placementHidden = true
        row.placementHidden = false
        XCTAssertTrue(row.inert, "returning on screen does not change input containment")
        XCTAssertFalse(row.isAccessibilityHidden(), "a recycled row uses the restored accessibility gate")
        XCTAssertTrue(elements(p.axElements(roots: [p.viewport])).contains { $0["testId"] as? String == "named" })
    }

    func testAScrollWrappedModalKeepsItsNativeControl() throws {
        let p = try fixture()
        p.apply(wireBatch([
            ["op": "create", "id": 10, "kind": "view", "props": ["accessibilityRole": "dialog", "accessibilityModal": "true", "accessibilityLabel": "Settings", "testId": "dlg"]],
            ["op": "children", "id": 1, "ids": [2, 3, 10]],
            ["op": "frame", "id": 10, "x": 0.0, "y": 100.0, "w": 200.0, "h": 100.0],
        ]))
        let dialog = try XCTUnwrap(p.views[10])
        let scroll = NSScrollView(frame: NSRect(x: 0, y: 100, width: 200, height: 100))
        let document = NSView(frame: scroll.bounds)
        scroll.documentView = document
        try XCTUnwrap(p.views[1]).addSubview(scroll)
        document.addSubview(dialog)
        let native = NSButton(title: "Continue", target: nil, action: nil)
        native.frame = NSRect(x: 10, y: 10, width: 100, height: 30)
        dialog.addSubview(native)
        p.syncAccessibility()

        let all = elements(p.axElements(roots: [p.viewport]))
        XCTAssertTrue(all.contains { $0["testId"] as? String == "dlg" && $0["role"] as? String == "dialog" })
        XCTAssertTrue(all.contains { $0["name"] as? String == "Continue" && $0["role"] as? String == "button" })
        XCTAssertFalse(all.contains { $0["testId"] as? String == "named" })
        try assertModalParentChain(dialog, presenter: p)
    }

    /// Onboarding and habits F16: what the web's tree has, AppKit's has. A
    /// native checkbox's `aria-label` names it (its cell is served the
    /// control's label); `role="radio"`/`"checkbox"`/`"switch"` with
    /// `aria-checked` are checkable; a `radiogroup` groups them; `aria-hidden`
    /// text leaves the tree and a name; `role="img"` is a labelled image.
    func testARIARolesStatesAndHiddenTextReachAppKit() throws {
        let p = try fixture()
        p.apply(wireBatch([
            ["op": "create", "id": 10, "kind": "control", "props": ["type": "checkbox", "accessibilityLabel": "Design", "testId": "design"], "handlers": ["change"], "style": [:]],
            ["op": "create", "id": 11, "kind": "text", "props": ["text": "Design", "accessibilityElementsHidden": "true", "testId": "design-text"]],
            ["op": "create", "id": 12, "kind": "view", "props": ["accessibilityRole": "radiogroup", "accessibilityLabel": "Theme", "testId": "theme"]],
            ["op": "create", "id": 13, "kind": "button", "handlers": ["press"], "props": ["accessibilityRole": "radio", "accessibilityChecked": "true", "testId": "light"]],
            ["op": "create", "id": 14, "kind": "text", "props": ["text": "Light"]],
            ["op": "create", "id": 15, "kind": "view", "handlers": ["press"], "props": ["accessibilityRole": "checkbox", "accessibilityChecked": "false", "testId": "remind"]],
            ["op": "create", "id": 16, "kind": "text", "props": ["text": "⏰", "accessibilityElementsHidden": "true"]],
            ["op": "create", "id": 17, "kind": "text", "props": ["text": "Remind me"]],
            ["op": "create", "id": 18, "kind": "svg", "props": ["accessibilityRole": "img", "accessibilityLabel": "33% done", "testId": "ring"]],
            ["op": "create", "id": 19, "kind": "button", "handlers": ["press"], "props": ["accessibilityRole": "switch", "accessibilityChecked": "true", "accessibilityLabel": "Sound", "testId": "sound"]],
            ["op": "children", "id": 12, "ids": [13]],
            ["op": "children", "id": 13, "ids": [14]],
            ["op": "children", "id": 15, "ids": [16, 17]],
            ["op": "children", "id": 1, "ids": [2, 3, 10, 11, 12, 15, 18, 19]],
            ["op": "frame", "id": 10, "x": 0.0, "y": 100.0, "w": 20.0, "h": 20.0],
            ["op": "frame", "id": 11, "x": 30.0, "y": 100.0, "w": 60.0, "h": 20.0],
            ["op": "frame", "id": 12, "x": 0.0, "y": 130.0, "w": 200.0, "h": 30.0],
            ["op": "frame", "id": 13, "x": 0.0, "y": 0.0, "w": 80.0, "h": 30.0],
            ["op": "frame", "id": 14, "x": 0.0, "y": 0.0, "w": 80.0, "h": 30.0],
            ["op": "frame", "id": 15, "x": 0.0, "y": 170.0, "w": 200.0, "h": 30.0],
            ["op": "frame", "id": 16, "x": 0.0, "y": 0.0, "w": 20.0, "h": 30.0],
            ["op": "frame", "id": 17, "x": 30.0, "y": 0.0, "w": 100.0, "h": 30.0],
            ["op": "frame", "id": 18, "x": 0.0, "y": 210.0, "w": 32.0, "h": 32.0],
            ["op": "frame", "id": 19, "x": 0.0, "y": 250.0, "w": 80.0, "h": 30.0],
        ]))
        p.syncAccessibility()
        let all = elements(p.axElements(roots: [p.viewport]))
        func one(_ testId: String) throws -> [String: Any] {
            try XCTUnwrap(all.first { $0["testId"] as? String == testId }, "\(testId) in \(all.map { "\($0["role"] ?? "") \($0["testId"] ?? "")" })")
        }
        let design = try one("design")
        XCTAssertEqual([design["role"] as? String, design["name"] as? String], ["checkbox", "Design"])
        XCTAssertFalse(all.contains { $0["testId"] as? String == "design-text" }, "aria-hidden text is off the tree")
        let theme = try one("theme"), light = try one("light")
        XCTAssertEqual([theme["role"] as? String, theme["name"] as? String], ["radiogroup", "Theme"])
        XCTAssertEqual([light["role"] as? String, light["name"] as? String], ["radio", "Light"])
        XCTAssertEqual(light["parent"] as? Int, theme["i"] as? Int)
        XCTAssertEqual((light["states"] as? [String: Any])?["checked"] as? Bool, true)
        let remind = try one("remind")
        XCTAssertEqual([remind["role"] as? String, remind["name"] as? String], ["checkbox", "Remind me"], "a hidden glyph names nothing")
        XCTAssertEqual((remind["states"] as? [String: Any])?["checked"] as? Bool, false)
        let ring = try one("ring")
        XCTAssertEqual([ring["role"] as? String, ring["name"] as? String], ["image", "33% done"])
        let sound = try one("sound")
        XCTAssertEqual([sound["role"] as? String, (sound["states"] as? [String: Any])?["checked"] as? Bool], ["switch", true] as [AnyHashable?])
    }

    func testFormStateWritesFollowPropsAndInheritedAvailability() throws {
        let p = try fixture()
        p.apply(wireBatch([
            ["op": "create", "id": 20, "kind": "view", "props": ["accessibilityHint": "Help", "accessibilityRequired": "true"]],
            ["op": "create", "id": 21, "kind": "view", "props": ["accessibilityDisabled": "true"]],
            ["op": "create", "id": 22, "kind": "view", "props": ["title": "Tip"]],
            ["op": "children", "id": 1, "ids": [20, 21, 22]],
        ]))
        let titled = try XCTUnwrap(p.views[22])
        XCTAssertTrue(titled.isAccessibilityEnabled(), "a new NSView's default must be explicitly enabled")
        XCTAssertEqual(titled.toolTip, "Tip")
        XCTAssertNil(titled.accessibilityHelp(), "a tooltip must not replace the authored accessibility description")
        p.apply(wireBatch([["op": "props", "id": 22, "set": ["title": "New tip"]]]))
        XCTAssertNil(titled.accessibilityHelp())
        let node = try XCTUnwrap(p.views[20])
        XCTAssertEqual(node.accessibilityHelp(), "Help")
        XCTAssertTrue(node.isAccessibilityRequired())
        XCTAssertTrue(node.isAccessibilityEnabled())
        p.apply(wireBatch([["op": "props", "id": 20, "set": ["testId": "described"]]]))
        XCTAssertEqual(node.accessibilityHelp(), "Help", "unrelated props keep the form states")
        XCTAssertTrue(node.isAccessibilityRequired())
        p.apply(wireBatch([
            ["op": "children", "id": 1, "ids": [21]],
            ["op": "children", "id": 21, "ids": [20]],
        ]))
        XCTAssertFalse(node.isAccessibilityEnabled(), "aria-disabled is inherited after reparenting")
        p.apply(wireBatch([
            ["op": "props", "id": 21, "clear": ["accessibilityDisabled"]],
            ["op": "props", "id": 20, "clear": ["accessibilityHint", "accessibilityRequired"]],
        ]))
        XCTAssertTrue(node.isAccessibilityEnabled())
        XCTAssertNil(node.accessibilityHelp())
        XCTAssertFalse(node.isAccessibilityRequired())
        p.apply(wireBatch([["op": "props", "id": 20, "set": ["accessibilityHint": "Again", "accessibilityRequired": "true"]]]))
        XCTAssertEqual(node.accessibilityHelp(), "Again")
        XCTAssertTrue(node.isAccessibilityRequired())
    }

    /// Onboarding F22, spreadsheet F20: a field's `aria-describedby` text is
    /// its AXHelp (and follows that text), `aria-required` its AXRequired,
    /// `aria-invalid` WebKit's AXInvalid; a menu button's `aria-haspopup` is
    /// Chromium's AXHasPopup and AXPopupValue.
    func testFormStatesAndHasPopupAreServed() throws {
        let p = try fixture()
        p.apply(wireBatch([
            ["op": "create", "id": 20, "kind": "input", "props": ["accessibilityLabel": "Email", "testId": "email", "accessibilityRequired": "true",
                                                              "accessibilityInvalid": "true", "accessibilityDescribedBy": "email-error", "accessibilityHint": "Work address"]],
            ["op": "create", "id": 21, "kind": "text", "props": ["text": "Enter an email", "id": "email-error"]],
            ["op": "create", "id": 22, "kind": "button", "handlers": ["press"], "props": ["accessibilityHasPopup": "menu", "testId": "file", "accessibilityHint": "Opens the menu"]],
            ["op": "create", "id": 23, "kind": "text", "props": ["text": "File"]],
            ["op": "create", "id": 24, "kind": "input", "props": ["accessibilityLabel": "Name", "testId": "name", "accessibilityInvalid": "false"]],
            ["op": "children", "id": 22, "ids": [23]],
            ["op": "children", "id": 1, "ids": [2, 3, 20, 21, 22, 24]],
            ["op": "frame", "id": 20, "x": 0.0, "y": 100.0, "w": 200.0, "h": 24.0],
            ["op": "frame", "id": 21, "x": 0.0, "y": 130.0, "w": 200.0, "h": 20.0],
            ["op": "frame", "id": 22, "x": 0.0, "y": 160.0, "w": 80.0, "h": 30.0],
            ["op": "frame", "id": 23, "x": 0.0, "y": 0.0, "w": 80.0, "h": 30.0],
            ["op": "frame", "id": 24, "x": 0.0, "y": 200.0, "w": 200.0, "h": 24.0],
        ]))
        p.syncAccessibility()
        func one(_ testId: String) throws -> [String: Any] {
            let all = elements(p.axElements(roots: [p.viewport]))
            return try XCTUnwrap(all.first { $0["testId"] as? String == testId && $0["role"] as? String != "text" }, "\(testId) in \(all.map { "\($0["role"] ?? "") \($0["testId"] ?? "")" })")
        }
        var email = try one("email")
        XCTAssertEqual([email["role"] as? String, email["name"] as? String, email["description"] as? String], ["textbox", "Email", "Enter an email"], "aria-describedby wins over aria-description")
        var states = try XCTUnwrap(email["states"] as? [String: Any])
        XCTAssertEqual(states["required"] as? Bool, true)
        XCTAssertEqual(states["invalid"] as? String, "true")
        XCTAssertNil((try one("name"))["states"].flatMap { ($0 as? [String: Any])?["invalid"] }, "aria-invalid=false serves nothing")
        let file = try one("file")
        XCTAssertEqual([file["role"] as? String, (file["states"] as? [String: Any])?["haspopup"] as? String, file["description"] as? String], ["button", "menu", "Opens the menu"])
        // The description follows the text it names.
        p.apply(wireBatch([["op": "props", "id": 21, "set": ["text": "Use name@domain"], "clear": []]]))
        p.syncAccessibility(changed: [21])
        email = try one("email")
        XCTAssertEqual(email["description"] as? String, "Use name@domain")
        p.apply(wireBatch([["op": "props", "id": 20, "set": [:], "clear": ["accessibilityRequired", "accessibilityInvalid", "accessibilityDescribedBy"]]]))
        p.syncAccessibility(changed: [20])
        email = try one("email")
        states = try XCTUnwrap(email["states"] as? [String: Any])
        XCTAssertEqual([states["required"] as? Bool, states["invalid"] as? Bool, email["description"] as? String], [nil, nil, "Work address"] as [AnyHashable?])
    }

    /// Ledger2 Rough 3: `aria-labelledby` names a radiogroup by its visible
    /// heading, wins over `aria-label` (accname), and follows the heading's text.
    func testAriaLabelledByNamesByTheHeadingsText() throws {
        let p = try fixture()
        p.apply(wireBatch([
            ["op": "create", "id": 30, "kind": "text", "props": ["text": "Currency", "id": "currency-heading"]],
            ["op": "create", "id": 31, "kind": "view", "props": ["accessibilityRole": "radiogroup", "accessibilityLabelledBy": "currency-heading",
                                                             "accessibilityLabel": "Ignored", "testId": "group"]],
            ["op": "create", "id": 32, "kind": "button", "handlers": ["press"], "props": ["accessibilityRole": "radio", "accessibilityChecked": "true", "testId": "usd"]],
            ["op": "create", "id": 33, "kind": "text", "props": ["text": "USD"]],
            ["op": "children", "id": 32, "ids": [33]],
            ["op": "children", "id": 31, "ids": [32]],
            ["op": "children", "id": 1, "ids": [2, 3, 30, 31]],
            ["op": "frame", "id": 30, "x": 0.0, "y": 100.0, "w": 200.0, "h": 20.0],
            ["op": "frame", "id": 31, "x": 0.0, "y": 130.0, "w": 200.0, "h": 40.0],
            ["op": "frame", "id": 32, "x": 0.0, "y": 0.0, "w": 80.0, "h": 30.0],
            ["op": "frame", "id": 33, "x": 0.0, "y": 0.0, "w": 80.0, "h": 30.0],
        ]))
        p.syncAccessibility()
        func group() throws -> [String: Any] {
            let all = elements(p.axElements(roots: [p.viewport]))
            return try XCTUnwrap(all.first { $0["testId"] as? String == "group" }, "group in \(all.map { "\($0["role"] ?? "") \($0["testId"] ?? "")" })")
        }
        XCTAssertEqual(try group()["name"] as? String, "Currency")
        p.apply(wireBatch([["op": "props", "id": 30, "set": ["text": "Currency symbol"], "clear": []]]))
        p.syncAccessibility(changed: [30])
        XCTAssertEqual(try group()["name"] as? String, "Currency symbol", "the name follows the heading")
    }

    /// Gallery F22, onboarding F27: a shortcut behind a shown `aria-modal`
    /// view is not heard, and Enter or Space on a focused button is its own.
    func testShortcutsStayInsideTheModalAndLeaveTheFocusItsKeys() throws {
        let p = try fixture()
        p.apply(wireBatch([
            ["op": "create", "id": 5, "kind": "view", "props": ["accessibilityModal": "true", "accessibilityRole": "dialog"]],
            ["op": "create", "id": 6, "kind": "button", "handlers": ["press"], "props": ["accessibilityKeyShortcuts": "Escape"]],
            ["op": "children", "id": 5, "ids": [6]],
            ["op": "children", "id": 1, "ids": [2, 3, 5]],
            ["op": "frame", "id": 5, "x": 0.0, "y": 100.0, "w": 200.0, "h": 100.0],
            ["op": "frame", "id": 6, "x": 0.0, "y": 0.0, "w": 80.0, "h": 30.0],
        ]))
        let done = try XCTUnwrap(p.views[2]), back = try XCTUnwrap(p.views[3]), cancel = try XCTUnwrap(p.views[6])
        XCTAssertFalse(p.shortcutAdmits(done, key: "Escape", held: "", focus: nil), "behind the modal")
        XCTAssertTrue(p.shortcutAdmits(cancel, key: "Escape", held: "", focus: nil))
        p.apply(wireBatch([["op": "props", "id": 5, "set": ["accessibilityModal": "false"], "clear": []]]))
        XCTAssertTrue(p.shortcutAdmits(done, key: "Escape", held: "", focus: nil), "no modal shown")
        XCTAssertFalse(p.shortcutAdmits(done, key: "Enter", held: "", focus: back), "the focused button's own Enter")
        XCTAssertTrue(p.shortcutAdmits(done, key: "Enter", held: "Meta+", focus: back))
        XCTAssertTrue(p.shortcutAdmits(done, key: "Enter", held: "", focus: done))
    }

    /// #278: a `dialog` or `alertdialog` role is a group with WebKit's
    /// subrole, named by its author, holding its children; a status
    /// paragraph with a link holds its words around the link, so each word
    /// is read once, as Chrome's tree has them.
    func testDialogRolesAndAStatusWithALinkReadAsTheWebsTree() throws {
        let p = try fixture()
        func run(_ id: Int, _ text: String, link: Bool = false) -> [String: Any] {
            ["id": id, "parent": 20, "paint": true, "style": ["font_size": 16.0],
             "props": link ? ["text": text, "href": "https://example.com/undo", "testId": "undo"] : ["text": text]]
        }
        p.apply(wireBatch([
            ["op": "create", "id": 10, "kind": "view", "props": ["accessibilityRole": "dialog", "accessibilityLabel": "Setup", "testId": "dlg"]],
            ["op": "create", "id": 11, "kind": "button", "handlers": ["press"], "props": ["accessibilityLabel": "OK"]],
            ["op": "create", "id": 12, "kind": "view", "props": ["accessibilityRole": "alertdialog", "accessibilityLabel": "Discard draft?", "testId": "confirm"]],
            ["op": "create", "id": 20, "kind": "text", "props": ["accessibilityRole": "status", "testId": "archived"], "style": ["font_size": 16.0]],
            ["op": "paragraph", "id": 20, "runs": [run(21, "Archived. "), run(22, "Undo", link: true), run(23, " or keep it.")]],
            ["op": "children", "id": 10, "ids": [11]],
            ["op": "children", "id": 1, "ids": [2, 3, 10, 12, 20]],
            ["op": "frame", "id": 10, "x": 0.0, "y": 100.0, "w": 200.0, "h": 60.0],
            ["op": "frame", "id": 11, "x": 10.0, "y": 10.0, "w": 80.0, "h": 30.0],
            ["op": "frame", "id": 12, "x": 0.0, "y": 170.0, "w": 200.0, "h": 40.0],
            ["op": "frame", "id": 20, "x": 0.0, "y": 220.0, "w": 400.0, "h": 24.0],
        ]))
        p.viewport.layoutSubtreeIfNeeded()
        p.syncAccessibility()
        let all = elements(p.axElements(roots: [p.viewport]))
        func element(_ id: UInt32) throws -> [String: Any] {
            try XCTUnwrap(all.first { $0["id"] as? UInt32 == id && $0["via"] as? String == "self" }, "\(id) in \(all.map { "\($0["role"] ?? "") \($0["id"] ?? "")" })")
        }
        func children(_ e: [String: Any]) -> [String] {
            all.filter { $0["parent"] as? Int == e["i"] as? Int }.map { "\($0["role"] ?? "") \($0["name"] ?? "")" }
        }
        let dialog = try element(10), confirm = try element(12), status = try element(20)
        XCTAssertEqual(dialog["role"] as? String, "dialog")
        XCTAssertEqual(dialog["name"] as? String, "Setup")
        XCTAssertEqual((dialog["native"] as? [String: Any])?["subrole"] as? String, "AXApplicationDialog")
        XCTAssertEqual(children(dialog), ["button OK"])
        XCTAssertEqual(confirm["role"] as? String, "alertdialog")
        XCTAssertEqual(confirm["name"] as? String, "Discard draft?")
        XCTAssertEqual((confirm["native"] as? [String: Any])?["subrole"] as? String, "AXApplicationAlertDialog")
        XCTAssertEqual(status["role"] as? String, "status")
        XCTAssertEqual(status["name"] as? String, "", "named only by its author")
        XCTAssertEqual(children(status), ["text Archived. ", "link Undo", "text  or keep it."], "each word once, in reading order")
    }

    /// VoiceOver moves by `AXChildrenInNavigationOrder`. It is `AXChildren`,
    /// the order the author wrote, wherever the frames are: AppKit's own sorts
    /// by frame and read a chip over a textarea's top-left corner first.
    /// Another process reads the method's answer (measured with
    /// `AXUIElementCopyAttributeValue`); AppKit's in-process
    /// `accessibilityAttributeValue` of the name does not consult it.
    func testNavigationOrderIsTheTreeOrder() throws {
        let p = try fixture()
        p.apply(wireBatch([
            ["op": "create", "id": 20, "kind": "view", "props": ["accessibilityRole": "group", "accessibilityLabel": "Composer"]],
            ["op": "create", "id": 21, "kind": "textarea", "props": ["value": "", "accessibilityLabel": "Message"], "handlers": ["input"]],
            ["op": "create", "id": 22, "kind": "button", "handlers": ["press"], "props": ["accessibilityLabel": "Chip"]],
            ["op": "create", "id": 23, "kind": "button", "handlers": ["press"], "props": ["accessibilityLabel": "Send"]],
            ["op": "create", "id": 24, "kind": "button", "handlers": ["press"], "props": ["accessibilityLabel": "Corner"]],
            ["op": "create", "id": 30, "kind": "view", "props": ["accessibilityRole": "list", "accessibilityLabel": "Chips"], "style": ["overflow_y": "scroll"]],
            ["op": "create", "id": 31, "kind": "button", "handlers": ["press"], "props": ["accessibilityLabel": "Below"]],
            ["op": "create", "id": 32, "kind": "button", "handlers": ["press"], "props": ["accessibilityLabel": "Above"]],
            ["op": "children", "id": 20, "ids": [21, 22, 23]],
            ["op": "children", "id": 30, "ids": [31, 32]],
            ["op": "children", "id": 1, "ids": [2, 3, 20, 30, 24]],
            ["op": "frame", "id": 20, "x": 0.0, "y": 100.0, "w": 400.0, "h": 100.0],
            ["op": "frame", "id": 21, "x": 0.0, "y": 0.0, "w": 300.0, "h": 70.0],
            ["op": "frame", "id": 22, "x": 4.0, "y": 4.0, "w": 60.0, "h": 20.0],
            ["op": "frame", "id": 23, "x": 0.0, "y": 76.0, "w": 60.0, "h": 20.0],
            ["op": "frame", "id": 30, "x": 0.0, "y": 210.0, "w": 200.0, "h": 80.0],
            ["op": "frame", "id": 31, "x": 0.0, "y": 40.0, "w": 60.0, "h": 20.0],
            ["op": "frame", "id": 32, "x": 0.0, "y": 0.0, "w": 60.0, "h": 20.0],
            ["op": "frame", "id": 24, "x": 0.0, "y": 0.0, "w": 20.0, "h": 20.0],
        ]))
        p.syncAccessibility()
        let scroll = try XCTUnwrap(p.views[30]?.subviews.first { $0 is ChainingScrollView })
        for parent in [try XCTUnwrap(p.views[20]), scroll, p.viewport] {
            let children = try XCTUnwrap(parent.accessibilityChildren() as? [AnyObject])
            XCTAssertGreaterThan(children.count, 1)
            let order = try XCTUnwrap(Presenter.navigationOrder(parent) as? [AnyObject])
            XCTAssertTrue(order.elementsEqual(children, by: ===), "\(parent): \(order) is not \(children)")
        }
        let order = elements(p.axElements(roots: [p.viewport])).compactMap { $0["name"] as? String }
        XCTAssertEqual(order.filter { ["Message", "Chip", "Send", "Below", "Above", "Corner"].contains($0) }, ["Message", "Chip", "Send", "Below", "Above", "Corner"])
    }

    func testATargetScopesTheReply() throws {
        let p = try fixture()
        let ax = p.axElements(roots: [p.viewport], scope: [2])
        XCTAssertEqual(elements(ax).compactMap { $0["testId"] as? String }, ["named"])
    }

    /// D3: a sheet is this session's only when it holds this session's views;
    /// another session's (or the host app's) sheet is foreign and not walked.
    func testOnlyASheetHoldingTheSessionsViewsIsItsOwn() throws {
        let p = try fixture(), other = Presenter()
        let foreign = NSWindow(contentRect: NSRect(x: 0, y: 0, width: 200, height: 100), styleMask: [.titled], backing: .buffered, defer: false)
        foreign.contentView = other.viewport
        guard case .foreign = p.axSheet(foreign) else { return XCTFail("another presenter's sheet is foreign") }
        guard case .owned = other.axSheet(foreign) else { return XCTFail("a sheet holding the presenter's viewport is its own") }
        let mine = NSWindow(contentRect: NSRect(x: 0, y: 0, width: 200, height: 100), styleMask: [.titled], backing: .buffered, defer: false)
        mine.contentView?.addSubview(try XCTUnwrap(p.views[3]))
        guard case .owned = p.axSheet(mine) else { return XCTFail("a sheet holding one of the presenter's views is its own") }
        guard case .none = p.axSheet(nil) else { return XCTFail("no sheet") }
    }

    /// LLP 1115.002 D7.2: a `dialog` in its sheet is that window's. The page
    /// behind it exposes nothing, the sheet's content is the session's own
    /// root, and the walk from it finds the dialog as the modal `showModal`
    /// made, with what is in it; a popover AppKit shows from it is a root too.
    func testADialogInItsSheetProjectsOnlyItsSubtree() throws {
        let p = try fixture(painted: false)
        p.apply(wireBatch([
            ["op": "create", "id": 10, "kind": "view", "props": ["semanticTag": "dialog", "id": "setup", "accessibilityRole": "dialog", "accessibilityLabel": "Setup", "testId": "dlg"]],
            ["op": "create", "id": 11, "kind": "button", "handlers": ["press"], "props": ["accessibilityLabel": "Options", "testId": "open", "popovertarget": "options"]],
            ["op": "create", "id": 12, "kind": "view", "props": ["popover": "auto", "id": "options", "testId": "pop"]],
            ["op": "create", "id": 13, "kind": "input", "props": ["accessibilityLabel": "Filter", "testId": "filter"]],
            ["op": "children", "id": 12, "ids": [13]],
            ["op": "children", "id": 10, "ids": [11, 12]],
            ["op": "children", "id": 1, "ids": [2, 3, 10]],
            ["op": "frame", "id": 10, "x": 0.0, "y": 100.0, "w": 200.0, "h": 100.0],
            ["op": "frame", "id": 11, "x": 0.0, "y": 0.0, "w": 80.0, "h": 30.0],
            ["op": "frame", "id": 12, "x": 0.0, "y": 30.0, "w": 180.0, "h": 60.0],
            ["op": "frame", "id": 13, "x": 0.0, "y": 0.0, "w": 140.0, "h": 24.0],
        ]))
        let dialog = try XCTUnwrap(p.views[10])
        p.dialogs.show(dialog)
        defer { p.reset() }
        let sheet = try XCTUnwrap(window.attachedSheet)
        XCTAssertTrue(dialog.window === sheet)
        guard case .owned(let content) = p.axSheet(sheet) else { return XCTFail("the dialog's sheet is the session's own") }
        XCTAssertEqual(p.sheets(on: window).count, 1)
        // The page: nothing of it, and nothing of the dialog either, which is another window's.
        XCTAssertEqual(p.viewport.accessibilityChildren()?.count, 0)
        let parent = dialog.accessibilityParent() as AnyObject?
        XCTAssertTrue(parent === sheet || parent === content, "the dialog's parent is its sheet, not the viewport")
        let ax = p.axElements(roots: [p.viewport, content], modalRoot: content)
        let modal = try XCTUnwrap(ax["modal"] as? [String: Any])
        XCTAssertEqual(modal["present"] as? Bool, true)
        XCTAssertEqual(modal["by"] as? String, "showModal")
        XCTAssertEqual(modal["id"] as? UInt32, 10)
        let all = elements(ax)
        XCTAssertTrue(all.contains { $0["testId"] as? String == "dlg" && $0["role"] as? String == "dialog" })
        XCTAssertTrue(all.contains { $0["testId"] as? String == "open" })
        XCTAssertFalse(all.contains { $0["testId"] as? String == "named" }, "the page behind the sheet is hidden")
        // A popover from the sheet: AppKit's, in a window of its own, and a root of its own.
        p.menus.show(try XCTUnwrap(p.views[12]), from: try XCTUnwrap(p.views[11]))
        let popover = try XCTUnwrap(p.menus.windowed.first)
        XCTAssertTrue(p.views[12]?.superview === popover.content)
        XCTAssertTrue(p.hosts(popover.window))
        let with = elements(p.axElements(roots: [p.viewport, content] + p.menus.windowed.map(\.content), modalRoot: content))
        XCTAssertTrue(with.contains { $0["testId"] as? String == "filter" }, "the popover's field is reachable")
        XCTAssertFalse(with.contains { $0["testId"] as? String == "named" })
        p.dialogs.close(dialog)
        XCTAssertNil(window.attachedSheet)
        XCTAssertTrue(p.menus.windowed.isEmpty, "a popover of the dialog's closes with it")
        XCTAssertTrue(elements(p.axElements(roots: [p.viewport])).contains { $0["testId"] as? String == "named" }, "the page is back")
    }
}
#endif
