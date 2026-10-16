#if os(macOS)
import AppKit
import ApplicationServices
import XCTest
@testable import ExactKit

/// What an accessibility client (VoiceOver) is served and told, read as one
/// reads it: through the AX API, from a thread of its own while the main
/// thread serves, so AppKit's request path is the one VoiceOver's takes, and
/// through an observer of the notifications the host posts. Needs this
/// process to be trusted for accessibility (the terminal that runs it).
private final class AXClient: @unchecked Sendable {
    private let lock = NSLock()
    private var heard: [(name: String, info: [String: Any], element: AXUIElement)] = []
    private var observer: AXObserver?
    private let app = AXUIElementCreateApplication(getpid())
    private let queue = DispatchQueue(label: "ax-client")

    /// The main run loop turns until `body`, run on the client's thread, returns.
    private func onClient<T>(_ body: @escaping @Sendable () -> T) -> T {
        var result: T?
        let done = DispatchSemaphore(value: 0)
        queue.async { result = body(); done.signal() }
        while done.wait(timeout: .now()) == .timedOut { RunLoop.main.run(mode: .default, before: Date(timeIntervalSinceNow: 0.005)) }
        return result!
    }
    func listen(_ names: [String]) throws {
        let ok: Bool = onClient { [self] in
            let callback: AXObserverCallbackWithInfo = { _, element, name, info, context in
                let client = Unmanaged<AXClient>.fromOpaque(context!).takeUnretainedValue()
                client.lock.lock(); defer { client.lock.unlock() }
                client.heard.append((name as String, info as? [String: Any] ?? [:], element))
            }
            guard AXObserverCreateWithInfoCallback(getpid(), callback, &observer) == .success, let observer else { return false }
            for name in names where AXObserverAddNotification(observer, app, name as CFString, Unmanaged.passUnretained(self).toOpaque()) != .success { return false }
            return true
        }
        guard ok, let observer else { throw XCTSkip("no AX observer on this process") }
        CFRunLoopAddSource(CFRunLoopGetMain(), AXObserverGetRunLoopSource(observer), .defaultMode)
    }
    /// What was heard once the main run loop has turned for `seconds`.
    func notifications(after seconds: TimeInterval = 0.3) -> [(name: String, info: [String: Any], element: AXUIElement)] {
        RunLoop.main.run(until: Date(timeIntervalSinceNow: seconds))
        lock.lock(); defer { lock.unlock() }
        return heard
    }
    func clear() { lock.lock(); heard.removeAll(); lock.unlock() }
    deinit { if let observer { CFRunLoopRemoveSource(CFRunLoopGetMain(), AXObserverGetRunLoopSource(observer), .defaultMode) } }

    static func value(_ element: AXUIElement, _ attribute: String) -> Any? {
        var value: CFTypeRef?
        return AXUIElementCopyAttributeValue(element, attribute as CFString, &value) == .success ? value : nil
    }
    /// The element whose AXIdentifier is `identifier`, found from the application down.
    private static func find(_ identifier: String, in element: AXUIElement, depth: Int = 0) -> AXUIElement? {
        if value(element, "AXIdentifier") as? String == identifier { return element }
        guard depth < 40 else { return nil }
        for child in value(element, "AXChildren") as? [AXUIElement] ?? [] {
            if let hit = find(identifier, in: child, depth: depth + 1) { return hit }
        }
        return nil
    }
    /// Its attributes among `names`, each as text; nil when no element has the identifier.
    func attributes(of identifier: String, _ names: [String]) -> [String: String]? {
        let app = app
        return onClient {
            guard let element = Self.find(identifier, in: app) else { return nil }
            var out: [String: String] = [:]
            for name in names { if let v = Self.value(element, name) { out[name] = "\(v)" } }
            return out
        }
    }
    /// Its attribute, read on the client's thread.
    func value(of element: AXUIElement, _ attribute: String) -> String? {
        onClient { Self.value(element, attribute).map { "\($0)" } }
    }
    /// The identifiers of the elements an attribute holds (AXSelectedChildren).
    func identifiers(of identifier: String, _ attribute: String) -> [String]? {
        let app = app
        return onClient {
            guard let element = Self.find(identifier, in: app), let list = Self.value(element, attribute) as? [AXUIElement] else { return nil }
            return list.map { Self.value($0, "AXIdentifier") as? String ?? "?" }
        }
    }
    /// The roles of the text fields among the application's elements, by their name (AXDescription).
    func fieldRoles() -> [String: String] {
        let app = app
        return onClient {
            var out: [String: String] = [:]
            func walk(_ element: AXUIElement, _ depth: Int) {
                if let name = Self.value(element, "AXDescription") as? String, Self.value(element, "AXValue") is String,
                   let role = Self.value(element, "AXRole") as? String { out[name] = role }
                guard depth < 40 else { return }
                for child in Self.value(element, "AXChildren") as? [AXUIElement] ?? [] { walk(child, depth + 1) }
            }
            walk(app, 0)
            return out
        }
    }
    /// Its actions.
    func actions(of identifier: String) -> [String] {
        let app = app
        return onClient {
            var names: CFArray?
            guard let element = Self.find(identifier, in: app), AXUIElementCopyActionNames(element, &names) == .success else { return [] }
            return names as? [String] ?? []
        }
    }
}

final class AccessibilityClientMacTests: XCTestCase {
    nonisolated(unsafe) private static var launched = false
    private var window: NSWindow!

    override func tearDown() {
        window?.orderOut(nil)
        window = nil
        super.tearDown()
    }

    private func fixture(_ ops: [[String: Any]]) throws -> Presenter {
        guard AXIsProcessTrusted() else { throw XCTSkip("this process is not trusted for accessibility") }
        // AppKit serves accessibility clients once the application has finished launching.
        if !Self.launched {
            Self.launched = true
            NSApplication.shared.setActivationPolicy(.accessory)
            NSApp.finishLaunching()
        }
        let p = Presenter()
        p.dialogs.painted = true
        p.menus.painted = true
        window = NSWindow(contentRect: NSRect(x: 0, y: 0, width: 400, height: 300), styleMask: [.titled], backing: .buffered, defer: false)
        window.isReleasedWhenClosed = false
        window.contentView = p.viewport
        p.apply(wireBatch([
            ["op": "create", "id": 1, "kind": "view"],
            ["op": "create", "id": 2, "kind": "button", "handlers": ["press"], "props": ["testId": "page-button", "accessibilityLabel": "More actions"]],
            ["op": "children", "id": 1, "ids": [2]],
            ["op": "roots", "ids": [1]],
            ["op": "frame", "id": 1, "x": 0.0, "y": 0.0, "w": 400.0, "h": 300.0],
            ["op": "frame", "id": 2, "x": 0.0, "y": 0.0, "w": 100.0, "h": 30.0],
        ] + ops))
        p.syncAccessibility()
        window.orderFront(nil)
        return p
    }

    /// A `role="menu"` box is AppKit's menu and its rows (`menuitem`, a
    /// box's `menuitemcheckbox`) are menu items, as WebKit serves them
    /// (Core-AAM), so VoiceOver says "menu" and "menu item" where it said
    /// nothing and "button" (realinput-1016a rows 10 and 13). A checked row
    /// carries AppKit's check mark, and the menu says it opened as it shows.
    func testAMenuAndItsRowsAreAppKitsMenuAndMenuItems() throws {
        let p = try fixture([])
        let client = AXClient()
        try client.listen(["AXMenuOpened"])
        p.apply(wireBatch([
            ["op": "create", "id": 10, "kind": "view", "props": ["accessibilityRole": "menu", "accessibilityLabel": "Environment actions", "testId": "menu"]],
            ["op": "create", "id": 11, "kind": "button", "handlers": ["press"], "props": ["accessibilityRole": "menuitem", "testId": "remove", "accessibilityLabel": "Remove from this device…"]],
            ["op": "create", "id": 12, "kind": "view", "handlers": ["press"],
             "props": ["accessibilityRole": "menuitemcheckbox", "accessibilityChecked": "true", "testId": "pin", "accessibilityLabel": "Pinned"]],
            ["op": "children", "id": 10, "ids": [11, 12]],
            ["op": "children", "id": 1, "ids": [2, 10]],
            ["op": "frame", "id": 10, "x": 0.0, "y": 40.0, "w": 200.0, "h": 70.0],
            ["op": "frame", "id": 11, "x": 0.0, "y": 0.0, "w": 200.0, "h": 30.0],
            ["op": "frame", "id": 12, "x": 0.0, "y": 35.0, "w": 200.0, "h": 30.0],
        ]))
        p.syncAccessibility()
        // It says it opened, as AppKit's menu does, once while it shows.
        p.syncAccessibility()
        XCTAssertEqual(client.notifications().map(\.name), ["AXMenuOpened"])
        let keys = ["AXRole", "AXRoleDescription", "AXDescription", "AXMenuItemMarkChar"]
        let menu = try XCTUnwrap(client.attributes(of: "menu", keys))
        XCTAssertEqual(menu["AXRole"], "AXMenu")
        XCTAssertEqual(menu["AXRoleDescription"], "menu")
        XCTAssertEqual(menu["AXDescription"], "Environment actions")
        let remove = try XCTUnwrap(client.attributes(of: "remove", keys))
        XCTAssertEqual(remove["AXRole"], "AXMenuItem")
        XCTAssertEqual(remove["AXRoleDescription"], "menu item")
        XCTAssertEqual(remove["AXDescription"], "Remove from this device…")
        XCTAssertNil(remove["AXMenuItemMarkChar"])
        let pin = try XCTUnwrap(client.attributes(of: "pin", keys))
        XCTAssertEqual(pin["AXRole"], "AXMenuItem")
        XCTAssertEqual(pin["AXMenuItemMarkChar"], "✓")
        // The row still presses as a button does.
        XCTAssertTrue(client.actions(of: "remove").contains("AXPress"))
        XCTAssertTrue(client.actions(of: "pin").contains("AXPress"))
        let tree = p.axElements(roots: [p.viewport])["elements"] as? [[String: Any]] ?? []
        XCTAssertEqual(tree.first { $0["testId"] as? String == "menu" }?["role"] as? String, "menu")
        XCTAssertEqual(tree.first { $0["testId"] as? String == "remove" }?["role"] as? String, "menuitem")
        // Gone and back, it opens again.
        client.clear()
        p.apply(wireBatch([["op": "children", "id": 1, "ids": [2]]]))
        p.syncAccessibility()
        p.apply(wireBatch([["op": "children", "id": 1, "ids": [2, 10]]]))
        p.syncAccessibility()
        XCTAssertEqual(client.notifications().map(\.name), ["AXMenuOpened"])

        p.apply(wireBatch([["op": "props", "id": 12, "set": ["accessibilityChecked": "false"]]]))
        XCTAssertNil(try XCTUnwrap(client.attributes(of: "pin", keys))["AXMenuItemMarkChar"], "an unchecked row has no mark")
        p.apply(wireBatch([["op": "props", "id": 10, "clear": ["accessibilityRole"]], ["op": "props", "id": 11, "clear": ["accessibilityRole"]]]))
        XCTAssertEqual(try XCTUnwrap(client.attributes(of: "remove", keys))["AXRole"], "AXButton", "a plain button again")
    }

    /// A labelled `group` says its role as AppKit's group does (realinput-1016a
    /// row 5: the image preview's zoomable group read with no "group").
    func testALabelledGroupHasAppKitsRoleDescription() throws {
        _ = try fixture([
            ["op": "create", "id": 10, "kind": "view", "handlers": ["key"],
             "props": ["accessibilityRole": "group", "accessibilityLabel": "large-grid.png, zoomable image", "tabindex": "0",
                       "accessibilityHint": "Use Enter to toggle zoom.", "testId": "viewport"]],
            ["op": "children", "id": 1, "ids": [2, 10]],
            ["op": "frame", "id": 10, "x": 0.0, "y": 40.0, "w": 200.0, "h": 150.0],
        ])
        let group = try XCTUnwrap(AXClient().attributes(of: "viewport", ["AXRole", "AXRoleDescription", "AXDescription", "AXHelp"]))
        XCTAssertEqual(group["AXRole"], "AXGroup")
        XCTAssertEqual(group["AXRoleDescription"], "group")
        XCTAssertEqual(group["AXDescription"], "large-grid.png, zoomable image")
        XCTAssertEqual(group["AXHelp"], "Use Enter to toggle zoom.")
    }

    /// An `alertdialog` that becomes the modal is announced as `NSAlert`'s
    /// sheet is read, its title, "dialog" and its description, whatever the
    /// focus does next (realinput-1016a row 13: only "Cancel, button" was
    /// spoken); VoiceOver is moved into it.
    func testAnAlertDialogIsAnnouncedAsItOpens() throws {
        let p = try fixture([])
        let client = AXClient()
        try client.listen(["AXLayoutChanged", "AXAnnouncementRequested"])
        p.apply(wireBatch([
            ["op": "create", "id": 10, "kind": "view",
             "props": ["accessibilityRole": "alertdialog", "accessibilityModal": "true", "accessibilityLabel": "Remove Lane Third from this device?",
                       "accessibilityDescribedBy": "message", "testId": "confirm"]],
            ["op": "create", "id": 11, "kind": "text", "props": ["text": "This forgets its pairing.", "id": "message"]],
            ["op": "create", "id": 12, "kind": "button", "handlers": ["press"], "props": ["accessibilityLabel": "Cancel", "testId": "cancel"]],
            ["op": "children", "id": 10, "ids": [11, 12]],
            ["op": "children", "id": 1, "ids": [2, 10]],
            ["op": "frame", "id": 10, "x": 50.0, "y": 50.0, "w": 300.0, "h": 150.0],
            ["op": "frame", "id": 11, "x": 10.0, "y": 10.0, "w": 280.0, "h": 20.0],
            ["op": "frame", "id": 12, "x": 10.0, "y": 100.0, "w": 80.0, "h": 30.0],
        ]))
        p.syncAccessibility()
        let heard = client.notifications()
        let announcement = try XCTUnwrap(heard.first { $0.name == "AXAnnouncementRequested" }, "\(heard.map(\.name))")
        XCTAssertEqual(announcement.info["AXAnnouncementKey"] as? String, "Remove Lane Third from this device?, dialog, This forgets its pairing.")
        XCTAssertEqual((announcement.info["AXPriorityKey"] as? NSNumber)?.intValue, NSAccessibilityPriorityLevel.high.rawValue)
        let layout = try XCTUnwrap(heard.first { $0.name == "AXLayoutChanged" })
        let landed = try XCTUnwrap((layout.info["AXUIElementsKey"] as? [AXUIElement])?.first)
        XCTAssertEqual(AXClient.value(landed, "AXIdentifier") as? String, "confirm", "VoiceOver is moved into the dialog")
        let dialog = try XCTUnwrap(client.attributes(of: "confirm", ["AXRole", "AXSubrole", "AXRoleDescription", "AXDescription", "AXHelp"]))
        XCTAssertEqual(dialog, ["AXRole": "AXGroup", "AXSubrole": "AXApplicationAlertDialog", "AXRoleDescription": "dialog",
                                "AXDescription": "Remove Lane Third from this device?", "AXHelp": "This forgets its pairing."])

    }

    /// A modal `dialog` drawn in the page is announced as it opens, as
    /// VoiceOver reads a sheet's or a panel's title as it appears, though
    /// the focus goes straight into its first field, and only once while it
    /// stays up: a dialog closed over it does not have it read again, as an
    /// AppKit sheet is not when its child sheet ends. One in a popover is
    /// not announced, as an `NSPopover` is not: VoiceOver reads its focus.
    func testAModalDialogIsAnnouncedAsItOpensUnlessInAPopover() throws {
        let p = try fixture([])
        let client = AXClient()
        try client.listen(["AXAnnouncementRequested"])
        p.apply(wireBatch([
            ["op": "create", "id": 10, "kind": "view",
             "props": ["accessibilityRole": "dialog", "accessibilityModal": "true", "accessibilityLabel": "Add Environment", "testId": "add"]],
            ["op": "create", "id": 11, "kind": "input", "handlers": ["input"], "props": ["value": "", "accessibilityLabel": "Host", "autofocus": "true"]],
            ["op": "children", "id": 10, "ids": [11]],
            ["op": "children", "id": 1, "ids": [2, 10]],
            ["op": "frame", "id": 10, "x": 50.0, "y": 50.0, "w": 300.0, "h": 150.0],
            ["op": "frame", "id": 11, "x": 10.0, "y": 10.0, "w": 280.0, "h": 24.0],
        ]))
        p.syncAccessibility()
        XCTAssertTrue(p.focusedNode === p.views[11], "the field took the focus")
        let heard = client.notifications().filter { $0.name == "AXAnnouncementRequested" }
        XCTAssertEqual(heard.map { $0.info["AXAnnouncementKey"] as? String }, ["Add Environment, dialog"])
        XCTAssertEqual((heard.first?.info["AXPriorityKey"] as? NSNumber)?.intValue, NSAccessibilityPriorityLevel.high.rawValue)

        // An alert dialog over it is announced; closed, the dialog under it is not read again.
        client.clear()
        p.apply(wireBatch([
            ["op": "create", "id": 30, "kind": "view",
             "props": ["accessibilityRole": "alertdialog", "accessibilityModal": "true", "accessibilityLabel": "Discard this environment?"]],
            ["op": "children", "id": 1, "ids": [2, 10, 30]],
            ["op": "frame", "id": 30, "x": 80.0, "y": 80.0, "w": 200.0, "h": 80.0],
        ]))
        p.syncAccessibility()
        XCTAssertTrue(p.macAccessibilityModal === p.views[30], "the alert dialog is the modal")
        p.apply(wireBatch([["op": "children", "id": 1, "ids": [2, 10]]]))
        p.syncAccessibility()
        XCTAssertTrue(p.macAccessibilityModal === p.views[10], "the dialog is the modal again")
        XCTAssertEqual(client.notifications().filter { $0.name == "AXAnnouncementRequested" }.map { $0.info["AXAnnouncementKey"] as? String },
                       ["Discard this environment?, dialog"])

        // Closed and opened again, it is announced again.
        client.clear()
        p.apply(wireBatch([["op": "children", "id": 1, "ids": [2]]]))
        p.syncAccessibility()
        p.apply(wireBatch([["op": "children", "id": 1, "ids": [2, 10]]]))
        p.syncAccessibility()
        XCTAssertEqual(client.notifications().filter { $0.name == "AXAnnouncementRequested" }.map { $0.info["AXAnnouncementKey"] as? String },
                       ["Add Environment, dialog"])

        // The same dialog as a popover's content: not announced.
        client.clear()
        p.apply(wireBatch([["op": "children", "id": 1, "ids": [2]]]))
        p.syncAccessibility()
        p.apply(wireBatch([
            ["op": "create", "id": 20, "kind": "view", "props": ["popover": "auto", "testId": "layer"]],
            ["op": "children", "id": 20, "ids": [10]],
            ["op": "children", "id": 1, "ids": [2, 20]],
            ["op": "frame", "id": 20, "x": 0.0, "y": 0.0, "w": 300.0, "h": 150.0],
            ["op": "frame", "id": 10, "x": 0.0, "y": 0.0, "w": 300.0, "h": 150.0],
        ]))
        p.menus.show(try XCTUnwrap(p.views[20]), from: try XCTUnwrap(p.views[2]))
        p.syncAccessibility()
        XCTAssertNotNil(p.macAccessibilityModal, "the dialog is the modal")
        XCTAssertFalse(client.notifications().contains { $0.name == "AXAnnouncementRequested" })
    }

    /// A modal whose field took the focus as it arrived (`autofocus`) sends
    /// VoiceOver to the field, as AppKit's popover posts only the focus change
    /// for its first responder (realinput-1016a row 15: the font picker read
    /// "Interface font family, dialog", not "Search fonts").
    func testAModalThatTookTheFocusSendsVoiceOverToTheFocus() throws {
        let p = try fixture([])
        let client = AXClient()
        try client.listen(["AXLayoutChanged"])
        p.apply(wireBatch([
            ["op": "create", "id": 10, "kind": "view",
             "props": ["accessibilityRole": "dialog", "accessibilityModal": "true", "accessibilityLabel": "Interface font family", "testId": "picker"]],
            ["op": "create", "id": 11, "kind": "input", "handlers": ["input"], "props": ["value": "", "accessibilityLabel": "Search fonts", "testId": "search", "autofocus": "true"]],
            ["op": "children", "id": 10, "ids": [11]],
            ["op": "children", "id": 1, "ids": [2, 10]],
            ["op": "frame", "id": 10, "x": 50.0, "y": 50.0, "w": 300.0, "h": 150.0],
            ["op": "frame", "id": 11, "x": 10.0, "y": 10.0, "w": 280.0, "h": 24.0],
        ]))
        p.syncAccessibility()
        XCTAssertTrue(p.focusedNode === p.views[11], "the field took the focus")
        let layout = try XCTUnwrap(client.notifications().first { $0.name == "AXLayoutChanged" })
        let landed = try XCTUnwrap((layout.info["AXUIElementsKey"] as? [AXUIElement])?.first)
        XCTAssertEqual(AXClient.value(landed, "AXDescription") as? String, "Search fonts")
        XCTAssertEqual(AXClient.value(landed, "AXRole") as? String, "AXTextField")
    }

    /// An `input` is AppKit's text field to another process, whatever it
    /// carries: a `keycapture` handler, a placeholder. A number's is still
    /// AppKit's stepper.
    func testAnInputIsATextField() throws {
        _ = try fixture([
            ["op": "create", "id": 11, "kind": "input", "handlers": ["input"], "props": ["value": "", "accessibilityLabel": "Project title"]],
            ["op": "create", "id": 12, "kind": "input", "handlers": ["input"], "props": ["value": "", "accessibilityLabel": "Search fonts", "placeholder": "Search"]],
            ["op": "create", "id": 13, "kind": "input", "handlers": ["keycapture", "blur"],
             "props": ["value": "", "accessibilityLabel": "Keybinding for New thread", "placeholder": "Press shortcut"]],
            ["op": "create", "id": 14, "kind": "input", "handlers": ["input"], "props": ["value": "4", "type": "number", "accessibilityLabel": "Port"]],
            ["op": "children", "id": 1, "ids": [2, 11, 12, 13, 14]],
            ["op": "frame", "id": 11, "x": 10.0, "y": 40.0, "w": 280.0, "h": 24.0],
            ["op": "frame", "id": 12, "x": 10.0, "y": 70.0, "w": 280.0, "h": 24.0],
            ["op": "frame", "id": 13, "x": 10.0, "y": 100.0, "w": 280.0, "h": 24.0],
            ["op": "frame", "id": 14, "x": 10.0, "y": 130.0, "w": 280.0, "h": 24.0],
        ])
        XCTAssertEqual(AXClient().fieldRoles(), ["Project title": "AXTextField", "Search fonts": "AXTextField",
                                                 "Keybinding for New thread": "AXTextField", "Port": "AXIncrementor"])
    }

    /// A `menu`'s row taking the focus is the menu's highlight, as AppKit's
    /// menu has one (measured with an `NSMenu` and this observer): the menu
    /// says its selected child changed, serves the row as that child and
    /// the row as selected. As the menu closes it says so, and the focus it
    /// gives back is told after that, so VoiceOver leaves the menu for it.
    func testAMenusFocusedRowIsItsHighlight() throws {
        let p = try fixture([])
        let client = AXClient()
        try client.listen(["AXMenuOpened", "AXMenuClosed", "AXSelectedChildrenChanged", "AXFocusedUIElementChanged"])
        p.apply(wireBatch([
            ["op": "create", "id": 20, "kind": "view", "props": ["popover": "auto", "testId": "layer"]],
            ["op": "create", "id": 10, "kind": "view", "props": ["accessibilityRole": "menu", "accessibilityLabel": "Environment actions", "testId": "menu"]],
            ["op": "create", "id": 11, "kind": "button", "handlers": ["press"], "props": ["accessibilityRole": "menuitem", "testId": "icon", "accessibilityLabel": "Icon"]],
            ["op": "create", "id": 12, "kind": "button", "handlers": ["press"], "props": ["accessibilityRole": "menuitem", "testId": "rename", "accessibilityLabel": "Rename"]],
            ["op": "children", "id": 10, "ids": [11, 12]],
            ["op": "children", "id": 20, "ids": [10]],
            ["op": "children", "id": 1, "ids": [2, 20]],
            ["op": "frame", "id": 20, "x": 0.0, "y": 40.0, "w": 200.0, "h": 70.0],
            ["op": "frame", "id": 10, "x": 0.0, "y": 0.0, "w": 200.0, "h": 70.0],
            ["op": "frame", "id": 11, "x": 0.0, "y": 0.0, "w": 200.0, "h": 30.0],
            ["op": "frame", "id": 12, "x": 0.0, "y": 35.0, "w": 200.0, "h": 30.0],
        ]))
        p.syncAccessibility()
        func heard() -> [String] {
            defer { client.clear() }
            return client.notifications().map { "\($0.name) \(client.value(of: $0.element, "AXIdentifier") ?? "")" }
        }
        // The trigger opens the menu.
        XCTAssertTrue(window.makeFirstResponder(p.views[2]))
        client.clear()
        p.menus.show(try XCTUnwrap(p.views[20]), from: try XCTUnwrap(p.views[2]))
        p.syncAccessibility()
        XCTAssertEqual(client.identifiers(of: "menu", "AXSelectedChildren"), [], "no row is highlighted yet")
        XCTAssertEqual(heard(), ["AXMenuOpened menu"])

        // ↓: the app moves the focus to a row.
        XCTAssertTrue(window.makeFirstResponder(p.views[11]))
        XCTAssertTrue(heard().contains("AXSelectedChildrenChanged menu"))
        XCTAssertEqual(client.identifiers(of: "menu", "AXSelectedChildren"), ["icon"])
        XCTAssertEqual(client.attributes(of: "icon", ["AXSelected"])?["AXSelected"], "1")
        XCTAssertEqual(client.attributes(of: "rename", ["AXSelected"])?["AXSelected"], "0")
        XCTAssertTrue(window.makeFirstResponder(p.views[12]))
        XCTAssertTrue(heard().contains("AXSelectedChildrenChanged menu"))
        XCTAssertEqual(client.identifiers(of: "menu", "AXSelectedChildren"), ["rename"])

        // Escape: the popover closes and gives the focus back to the trigger.
        XCTAssertTrue(p.menus.key(escape(window)))
        _ = p.menus.key(escape(window, up: true))
        p.syncAccessibility()
        XCTAssertTrue(window.firstResponder === p.views[2], "the focus is the trigger's again")
        let closing = heard()
        XCTAssertEqual(closing.filter { $0.hasPrefix("AXMenu") }.map { $0.split(separator: " ").first.map(String.init) }, ["AXMenuClosed"], "\(closing)")
        let closed = try XCTUnwrap(closing.firstIndex { $0.hasPrefix("AXMenuClosed") })
        XCTAssertEqual(closing[(closed + 1)...].last, "AXFocusedUIElementChanged page-button", "\(closing)")

        // A menu in the page that the app removes after focusing its invoker
        // says it closed, and the focus is told after that.
        p.apply(wireBatch([["op": "children", "id": 1, "ids": [2, 10]]]))
        p.syncAccessibility()
        XCTAssertTrue(window.makeFirstResponder(p.views[11]))
        client.clear()
        XCTAssertTrue(window.makeFirstResponder(p.views[2]))
        p.apply(wireBatch([["op": "children", "id": 1, "ids": [2]]]))
        p.syncAccessibility()
        let removed = heard()
        let removedAt = try XCTUnwrap(removed.firstIndex { $0.hasPrefix("AXMenuClosed") }, "\(removed)")
        XCTAssertEqual(removed[(removedAt + 1)...].last, "AXFocusedUIElementChanged page-button", "\(removed)")

        // A menu that opens with the focus already on a row says so after it opened.
        p.apply(wireBatch([["op": "children", "id": 1, "ids": [2, 10]]]))
        XCTAssertTrue(window.makeFirstResponder(p.views[11]))
        p.syncAccessibility()
        XCTAssertEqual(heard().filter { !$0.hasPrefix("AXFocused") }, ["AXMenuOpened menu", "AXSelectedChildrenChanged menu"])
    }

    private func escape(_ window: NSWindow, up: Bool = false) -> NSEvent {
        NSEvent.keyEvent(with: up ? .keyUp : .keyDown, location: .zero, modifierFlags: [], timestamp: 0,
            windowNumber: window.windowNumber, context: nil, characters: "\u{1b}", charactersIgnoringModifiers: "\u{1b}", isARepeat: false, keyCode: 53)!
    }
}
#endif
