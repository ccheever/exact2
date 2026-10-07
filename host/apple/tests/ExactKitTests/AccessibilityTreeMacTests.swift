#if os(macOS)
import AppKit
import XCTest
@testable import ExactKit

/// `tree --ax` on AppKit (LLP 1080.002 §3 stage 3): the unignored tree in
/// navigation order, AppKit's own roles and names, each element joined to
/// its view. Run by `bun host/apple/build.mjs --test`.
final class AccessibilityTreeMacTests: XCTestCase {
    private var window: NSWindow!

    private func fixture() throws -> Presenter {
        _ = NSApplication.shared
        let p = Presenter()
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
    /// (chat F14); an empty date reads empty and shows its format, never
    /// the picker's 1/1/2001 (kanban F23).
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
        XCTAssertEqual(DateField.placeholder("date", Locale(identifier: "en_US")), "mm/dd/yyyy")
        XCTAssertEqual(DateField.placeholder("date", Locale(identifier: "en_GB")), "dd/mm/yyyy")
        XCTAssertEqual(DateField.placeholder("time", Locale(identifier: "en_US")), "--:-- --")
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
}
#endif
