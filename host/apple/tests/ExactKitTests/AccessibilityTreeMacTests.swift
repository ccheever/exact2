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
