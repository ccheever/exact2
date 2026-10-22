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
    /// What was heard once the main run loop has turned for `seconds`:
    /// by default, until a dialog drawn in the page has been named.
    func notifications(after seconds: TimeInterval = Presenter.dialogNameDelay + 0.2) -> [(name: String, info: [String: Any], element: AXUIElement)] {
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
    /// Find an element by an attribute, usually its AXIdentifier.
    private static func find(_ identifier: String, in element: AXUIElement, depth: Int = 0, attribute: String = "AXIdentifier") -> AXUIElement? {
        if value(element, attribute) as? String == identifier { return element }
        guard depth < 40 else { return nil }
        for child in value(element, "AXChildren") as? [AXUIElement] ?? [] {
            if let hit = find(identifier, in: child, depth: depth + 1, attribute: attribute) { return hit }
        }
        return nil
    }
    /// The element whose AXIdentifier is `identifier`, as a client holds it.
    func element(_ identifier: String) -> AXUIElement? {
        let app = app
        return onClient { Self.find(identifier, in: app) }
    }
    /// Its attributes among `names`, each as text; nil when no element matches.
    func attributes(of identifier: String, _ names: [String], matching attribute: String = "AXIdentifier") -> [String: String]? {
        let app = app
        return onClient {
            guard let element = Self.find(identifier, in: app, attribute: attribute) else { return nil }
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
    /// The identifiers of every element served from the application down, as VoiceOver can reach them.
    func identifiers() -> [String] {
        let app = app
        return onClient {
            var out: [String] = []
            func walk(_ element: AXUIElement, _ depth: Int) {
                if let id = Self.value(element, "AXIdentifier") as? String { out.append(id) }
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
        KeyboardNavigation.agent = nil
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
        // The system's default, not the machine's: on, ordering the window
        // front would give its first key view (the page's button) the focus.
        KeyboardNavigation.agent = false
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

    /// VoiceOver lands on the element the opening commit's `focus()`
    /// chose, once the commit's commands have run, not on the alertdialog
    /// the batch left with no focus in it; then the dialog's title and
    /// message are queued after the button's words, at low priority, so
    /// neither cuts the other off, as an `NSAlert` is read by its title and
    /// message.
    func testAnAlertDialogLandsVoiceOverOnTheButtonItsCommitFocused() throws {
        let p = try fixture([])
        let client = AXClient()
        try client.listen(["AXLayoutChanged", "AXAnnouncementRequested", "AXFocusedUIElementChanged"])
        p.apply(wireBatch([
            ["op": "create", "id": 10, "kind": "view",
             "props": ["accessibilityRole": "alertdialog", "accessibilityModal": "true", "accessibilityLabel": "Remove Lane Third from this device?",
                       "accessibilityDescribedBy": "message", "testId": "confirm"]],
            ["op": "create", "id": 11, "kind": "text", "props": ["text": "This forgets its pairing.", "id": "message"]],
            ["op": "create", "id": 12, "kind": "button", "handlers": ["press"], "props": ["accessibilityLabel": "Cancel", "testId": "cancel", "id": "cancel"]],
            ["op": "children", "id": 10, "ids": [11, 12]],
            ["op": "children", "id": 1, "ids": [2, 10]],
            ["op": "frame", "id": 10, "x": 50.0, "y": 50.0, "w": 300.0, "h": 150.0],
            ["op": "frame", "id": 11, "x": 10.0, "y": 10.0, "w": 280.0, "h": 20.0],
            ["op": "frame", "id": 12, "x": 10.0, "y": 100.0, "w": 80.0, "h": 30.0],
        ]))
        // The commit's command, run once the batch is applied (`ExactSession.apply`).
        p.focusElement(["cancel"])
        XCTAssertTrue(p.focusedNode === p.views[12], "focus() took the focus")
        let heard = client.notifications().map { "\($0.name) \(client.value(of: $0.element, "AXIdentifier") ?? "")"
            + ((($0.info["AXUIElementsKey"] as? [AXUIElement])?.compactMap { AXClient.value($0, "AXIdentifier") as? String }).map { " \($0)" } ?? "") }
        XCTAssertEqual(heard, ["AXLayoutChanged confirm [\"cancel\"]", "AXFocusedUIElementChanged cancel", "AXAnnouncementRequested confirm"])
        let named = try XCTUnwrap(client.notifications(after: 0).last)
        XCTAssertEqual(named.info["AXAnnouncementKey"] as? String, "Remove Lane Third from this device?, dialog, This forgets its pairing.")
        XCTAssertEqual(named.info["AXPriorityKey"] as? Int, NSAccessibilityPriorityLevel.low.rawValue)
        let dialog = try XCTUnwrap(client.attributes(of: "confirm", ["AXRole", "AXSubrole", "AXRoleDescription", "AXDescription", "AXHelp"]))
        XCTAssertEqual(dialog, ["AXRole": "AXGroup", "AXSubrole": "AXApplicationAlertDialog", "AXRoleDescription": "dialog",
                                "AXDescription": "Remove Lane Third from this device?", "AXHelp": "This forgets its pairing."])
    }

    /// A dialog closed over another modal (the Commit dialog over a sheet
    /// drawn in the page) sends VoiceOver to the focus its closing commit
    /// gave back, in the modal now in front, as AppKit's sheet over a sheet
    /// ends with the focus change of the first responder under it
    /// (measured), and the sheet, named as it opened, is not named again.
    /// Chosen before that `focus()` ran, VoiceOver went to the page behind
    /// the sheet.
    func testADialogClosedOverAModalSendsVoiceOverToTheFocusGivenBack() throws {
        let p = try fixture([
            ["op": "create", "id": 20, "kind": "view", "props": ["accessibilityRole": "dialog", "accessibilityModal": "true", "accessibilityLabel": "Right panel", "testId": "sheet"]],
            ["op": "create", "id": 21, "kind": "button", "handlers": ["press"], "props": ["accessibilityLabel": "Git action options", "testId": "chevron", "id": "chevron"]],
            ["op": "children", "id": 20, "ids": [21]],
            ["op": "children", "id": 1, "ids": [2, 20]],
            ["op": "frame", "id": 20, "x": 100.0, "y": 0.0, "w": 300.0, "h": 300.0],
            ["op": "frame", "id": 21, "x": 10.0, "y": 10.0, "w": 120.0, "h": 30.0],
        ])
        RunLoop.main.run(until: Date(timeIntervalSinceNow: 0.05))
        p.apply(wireBatch([
            ["op": "create", "id": 10, "kind": "view", "props": ["accessibilityRole": "dialog", "accessibilityModal": "true", "accessibilityLabel": "Commit changes", "testId": "commit"]],
            ["op": "create", "id": 11, "kind": "input", "handlers": ["input"], "props": ["value": "", "accessibilityLabel": "Commit message", "testId": "message", "autofocus": "true"]],
            ["op": "children", "id": 10, "ids": [11]],
            ["op": "children", "id": 1, "ids": [2, 20, 10]],
            ["op": "frame", "id": 10, "x": 120.0, "y": 60.0, "w": 260.0, "h": 120.0],
            ["op": "frame", "id": 11, "x": 10.0, "y": 10.0, "w": 240.0, "h": 24.0],
        ]))
        p.syncAccessibility()
        RunLoop.main.run(until: Date(timeIntervalSinceNow: 0.05))
        XCTAssertTrue(p.macAccessibilityModal === p.views[10], "the Commit dialog is the modal")
        XCTAssertTrue(p.focusedNode === p.views[11])
        let client = AXClient()
        try client.listen(["AXLayoutChanged", "AXAnnouncementRequested", "AXFocusedUIElementChanged"])
        // Escape: the app closes the dialog and gives the focus back to the chevron.
        p.apply(wireBatch([["op": "children", "id": 1, "ids": [2, 20]]]))
        p.focusElement(["chevron"])
        XCTAssertTrue(p.macAccessibilityModal === p.views[20], "the sheet is the modal again")
        let heard = client.notifications().map { "\($0.name) \(client.value(of: $0.element, "AXIdentifier") ?? "")"
            + ((($0.info["AXUIElementsKey"] as? [AXUIElement])?.compactMap { AXClient.value($0, "AXIdentifier") as? String }).map { " \($0)" } ?? "") }
        XCTAssertEqual(heard, ["AXLayoutChanged sheet [\"chevron\"]", "AXFocusedUIElementChanged chevron"])
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

        // Opened by VO-Space, which presses the trigger and moves no focus,
        // and closed by Escape: the menu says it closed and no focus is
        // told, as AppKit's `NSMenu` (measured), so VoiceOver stays where
        // it was; told the focus left elsewhere, it went to a switch beside
        // the trigger.
        p.menus.show(try XCTUnwrap(p.views[20]), from: try XCTUnwrap(p.views[2]))
        p.syncAccessibility()
        XCTAssertEqual(heard(), ["AXMenuOpened menu"])
        let shownMenu = try XCTUnwrap(client.element("menu"))
        XCTAssertTrue(p.menus.key(escape(window)))
        _ = p.menus.key(escape(window, up: true))
        p.syncAccessibility()
        XCTAssertTrue(window.firstResponder === p.views[2])
        let escaped = client.notifications()
        client.clear()
        XCTAssertEqual(escaped.map(\.name), ["AXMenuClosed"])
        XCTAssertTrue(escaped.first.map { CFEqual($0.element, shownMenu) } ?? false, "on the menu")

        // A menu in the page that the app removes after focusing its invoker
        // says it closed as it leaves the window, on itself, and the focus is
        // told after that.
        p.apply(wireBatch([["op": "children", "id": 1, "ids": [2, 10]]]))
        p.syncAccessibility()
        XCTAssertTrue(window.makeFirstResponder(p.views[11]))
        let pageMenu = try XCTUnwrap(client.element("menu"))
        _ = heard()
        XCTAssertTrue(window.makeFirstResponder(p.views[2]))
        p.apply(wireBatch([["op": "children", "id": 1, "ids": [2]]]))
        p.syncAccessibility()
        let removed = client.notifications().filter { $0.name != "AXSelectedChildrenChanged" }
        client.clear()
        XCTAssertEqual(removed.map(\.name), ["AXMenuClosed", "AXFocusedUIElementChanged"])
        XCTAssertTrue(removed.first.map { CFEqual($0.element, pageMenu) } ?? false, "on the menu, not the page")
        XCTAssertEqual(removed.last.flatMap { AXClient.value($0.element, "AXIdentifier") as? String }, "page-button")

        // A menu that opens with the focus already on a row says so after it opened.
        p.apply(wireBatch([["op": "children", "id": 1, "ids": [2, 10]]]))
        XCTAssertTrue(window.makeFirstResponder(p.views[11]))
        p.syncAccessibility()
        XCTAssertEqual(heard().filter { !$0.hasPrefix("AXFocused") }, ["AXMenuOpened menu", "AXSelectedChildrenChanged menu"])
    }


    /// An `inert` page is out of what VoiceOver reaches, as it is out of a
    /// browser's accessibility tree (the T3 Code clone's chat behind its
    /// Settings page). A menu in a popover over it, holding an `aria-modal`
    /// list, closed by Escape, gives the focus back to its trigger before it
    /// says it closed, and VoiceOver is sent there after: the layout change
    /// names the trigger, then the trigger's focus is told
    /// (realinput-1016f row 8: VoiceOver landed on the page's thread title).
    func testAMenuOverAnInertPageSendsVoiceOverBackToItsTrigger() throws {
        let p = try fixture([])
        let client = AXClient()
        try client.listen(["AXMenuOpened", "AXMenuClosed", "AXFocusedUIElementChanged", "AXLayoutChanged", "AXUIElementDestroyed"])
        p.apply(wireBatch([
            ["op": "create", "id": 3, "kind": "view", "props": ["inert": "true", "testId": "page"]],
            ["op": "create", "id": 5, "kind": "button", "handlers": ["press"], "props": ["testId": "thread-title", "accessibilityLabel": "Thread actions for Lane"]],
            ["op": "create", "id": 4, "kind": "view", "props": ["testId": "settings"], "style": ["background_color": [255, 255, 255, 255]]],
            ["op": "create", "id": 20, "kind": "view", "props": ["popover": "auto", "testId": "layer"]],
            ["op": "create", "id": 10, "kind": "view", "props": ["accessibilityRole": "menu", "accessibilityLabel": "More actions", "testId": "menu"]],
            ["op": "create", "id": 15, "kind": "view", "props": ["accessibilityModal": "true", "tabindex": "-1", "testId": "keys"]],
            ["op": "create", "id": 11, "kind": "button", "handlers": ["press"], "props": ["accessibilityRole": "menuitem", "testId": "icon", "accessibilityLabel": "Icon"]],
            ["op": "children", "id": 15, "ids": [11]],
            ["op": "children", "id": 10, "ids": [15]],
            ["op": "children", "id": 20, "ids": [10]],
            ["op": "children", "id": 3, "ids": [5]],
            ["op": "create", "id": 6, "kind": "button", "handlers": ["press"], "props": ["testId": "switch", "accessibilityRole": "switch", "accessibilityLabel": "Local environment"]],
            ["op": "children", "id": 4, "ids": [2, 6, 20]],
            ["op": "children", "id": 1, "ids": [3, 4]],
            ["op": "frame", "id": 6, "x": 0.0, "y": 100.0, "w": 60.0, "h": 30.0],
            ["op": "frame", "id": 3, "x": 0.0, "y": 0.0, "w": 400.0, "h": 300.0],
            ["op": "frame", "id": 4, "x": 0.0, "y": 0.0, "w": 400.0, "h": 300.0],
            ["op": "frame", "id": 5, "x": 0.0, "y": 40.0, "w": 200.0, "h": 30.0],
            ["op": "frame", "id": 20, "x": 0.0, "y": 40.0, "w": 200.0, "h": 40.0],
            ["op": "frame", "id": 10, "x": 0.0, "y": 0.0, "w": 200.0, "h": 40.0],
            ["op": "frame", "id": 15, "x": 0.0, "y": 0.0, "w": 200.0, "h": 40.0],
            ["op": "frame", "id": 11, "x": 0.0, "y": 0.0, "w": 200.0, "h": 30.0],
        ]))
        p.syncAccessibility()
        let page = client.identifiers()
        XCTAssertTrue(page.contains("page-button"), "\(page)")
        XCTAssertFalse(page.contains("thread-title"), "the inert page's button is out of VoiceOver's reach: \(page)")
        XCTAssertFalse(p.axElements(roots: [p.viewport])["elements"].map { "\($0)" }?.contains("thread-title") ?? false, "and out of the agent's tree")
        func heard() -> [String] {
            defer { client.clear() }
            return client.notifications().map { note in
                let named = (note.info["AXUIElementsKey"] as? [AXUIElement])?.compactMap { AXClient.value($0, "AXIdentifier") as? String } ?? []
                return (["\(note.name) \(client.value(of: note.element, "AXIdentifier") ?? "")"] + named).joined(separator: " ")
            }
        }
        XCTAssertTrue(window.makeFirstResponder(p.views[2]))
        p.menus.show(try XCTUnwrap(p.views[20]), from: try XCTUnwrap(p.views[2]))
        XCTAssertTrue(window.makeFirstResponder(p.views[11]))
        p.syncAccessibility()
        _ = heard()

        XCTAssertTrue(p.menus.key(escape(window)))
        _ = p.menus.key(escape(window, up: true))
        p.syncAccessibility()
        XCTAssertTrue(window.firstResponder === p.views[2], "the focus is the trigger's again")
        let closing = heard().filter { !$0.hasPrefix("AXUIElementDestroyed") }
        XCTAssertEqual(closing, ["AXMenuClosed menu", "AXLayoutChanged  page-button", "AXFocusedUIElementChanged page-button"])
        XCTAssertFalse(client.identifiers().contains("thread-title"), "the page is still out of reach with the menu gone")

        // Opened by VO-Space with the keyboard focus elsewhere (a switch
        // below the trigger) and closed by Escape with no focus ever in it:
        // VoiceOver goes back to the trigger it pressed, as from an
        // `NSMenu`, and the focus stays where it was, not to the switch
        // that holds the focus.
        XCTAssertTrue(window.makeFirstResponder(p.views[6]))
        p.menus.show(try XCTUnwrap(p.views[20]), from: try XCTUnwrap(p.views[2]))
        p.syncAccessibility()
        _ = heard()
        XCTAssertTrue(p.menus.key(escape(window)))
        _ = p.menus.key(escape(window, up: true))
        p.syncAccessibility()
        XCTAssertTrue(window.firstResponder === p.views[6], "the focus stays on the switch")
        XCTAssertEqual(heard().filter { !$0.hasPrefix("AXUIElementDestroyed") }, ["AXMenuClosed menu", "AXLayoutChanged  page-button"])
        XCTAssertTrue(window.makeFirstResponder(p.views[2]))
        _ = heard()

        // Closed by the app instead: the batch takes the menu away and its
        // command gives the focus back. VoiceOver is sent to the trigger the
        // command focused, not to the page the batch left the focus on
        // ("scroll area").
        p.menus.show(try XCTUnwrap(p.views[20]), from: try XCTUnwrap(p.views[2]))
        XCTAssertTrue(window.makeFirstResponder(p.views[11]))
        p.syncAccessibility()
        _ = heard()
        p.views[2]?.props["id"] = "trigger"
        p.apply(wireBatch([["op": "children", "id": 4, "ids": [2, 6]]]))
        p.focusElement(["trigger"])
        XCTAssertTrue(window.firstResponder === p.views[2])
        XCTAssertEqual(heard().filter { !$0.hasPrefix("AXUIElementDestroyed") },
                       ["AXMenuClosed menu", "AXLayoutChanged  page-button", "AXFocusedUIElementChanged page-button"])

        // Opened with the focus on the trigger and moving none, then taken
        // away by a batch whose command focuses another control: VoiceOver
        // goes to that control, not to the trigger the popover closed on.
        p.views[6]?.props["id"] = "switch"
        p.apply(wireBatch([["op": "children", "id": 4, "ids": [2, 6, 20]]]))
        p.menus.show(try XCTUnwrap(p.views[20]), from: try XCTUnwrap(p.views[2]))
        p.syncAccessibility()
        _ = heard()
        p.apply(wireBatch([["op": "children", "id": 4, "ids": [2, 6]]]))
        p.focusElement(["switch"])
        XCTAssertTrue(window.firstResponder === p.views[6])
        XCTAssertEqual(heard().filter { !$0.hasPrefix("AXUIElementDestroyed") },
                       ["AXMenuClosed menu", "AXLayoutChanged  switch", "AXFocusedUIElementChanged switch"])

        // The page no longer inert, its button is VoiceOver's again.
        p.apply(wireBatch([["op": "props", "id": 3, "set": [:], "clear": ["inert"]]]))
        p.syncAccessibility()
        XCTAssertTrue(client.identifiers().contains("thread-title"))
    }

    /// A dialog drawn in the page whose field takes the focus sends
    /// VoiceOver to the field, then is named, queued after the field's
    /// words: AppKit tells a sheet by its creation, then the focus in it
    /// (measured here), and the sheet's title is read from its window,
    /// which a group in the page does not have. Announced before the focus,
    /// at high priority, the dialog's name cut the field's off.
    func testADialogIsNamedAfterTheFocusInIt() throws {
        let p = try fixture([])
        let client = AXClient()
        try client.listen(["AXAnnouncementRequested", "AXLayoutChanged", "AXFocusedUIElementChanged", "AXSheetCreated"])
        func heard() -> [String] {
            defer { client.clear() }
            return client.notifications().map { note in
                let named = (note.info["AXUIElementsKey"] as? [AXUIElement])?.compactMap { AXClient.value($0, "AXDescription") as? String } ?? []
                return ([note.name, client.value(of: note.element, "AXRole") ?? "", client.value(of: note.element, "AXDescription") ?? ""] + named)
                    .filter { !$0.isEmpty }.joined(separator: " ")
            }
        }
        // AppKit's own: a sheet whose field is its first responder.
        let sheet = NSWindow(contentRect: NSRect(x: 0, y: 0, width: 200, height: 100), styleMask: [.titled], backing: .buffered, defer: false)
        let field = NSTextField(frame: NSRect(x: 10, y: 10, width: 150, height: 24))
        field.setAccessibilityLabel("Sheet field")
        sheet.contentView?.addSubview(field)
        sheet.initialFirstResponder = field
        client.clear()
        window.beginSheet(sheet)
        let appKit = heard()
        window.endSheet(sheet)
        let created = try XCTUnwrap(appKit.firstIndex { $0.hasPrefix("AXSheetCreated") }, "\(appKit)")
        let fieldFocus = try XCTUnwrap(appKit.lastIndex { $0 == "AXFocusedUIElementChanged AXTextField Sheet field" }, "\(appKit)")
        XCTAssertLessThan(created, fieldFocus, "AppKit says the sheet, then the focus in it: \(appKit)")
        XCTAssertFalse(appKit.contains { $0.hasPrefix("AXAnnouncementRequested") }, "\(appKit)")
        _ = heard()

        p.apply(wireBatch([
            ["op": "create", "id": 10, "kind": "view",
             "props": ["accessibilityRole": "dialog", "accessibilityModal": "true", "accessibilityLabel": "Rename project", "testId": "rename"]],
            ["op": "create", "id": 11, "kind": "input", "handlers": ["input"], "props": ["value": "", "accessibilityLabel": "Project title", "autofocus": "true"]],
            ["op": "children", "id": 10, "ids": [11]],
            ["op": "children", "id": 1, "ids": [2, 10]],
            ["op": "frame", "id": 10, "x": 50.0, "y": 50.0, "w": 300.0, "h": 150.0],
            ["op": "frame", "id": 11, "x": 10.0, "y": 10.0, "w": 280.0, "h": 24.0],
        ]))
        p.syncAccessibility()
        XCTAssertTrue(p.focusedNode === p.views[11], "the field took the focus")
        let opened = client.notifications()
        XCTAssertEqual(heard(), ["AXLayoutChanged AXGroup Rename project Project title", "AXFocusedUIElementChanged AXTextField Project title",
                                 "AXAnnouncementRequested AXGroup Rename project"])
        XCTAssertEqual(opened.last?.info["AXAnnouncementKey"] as? String, "Rename project, dialog")
        XCTAssertEqual(opened.last?.info["AXPriorityKey"] as? Int, NSAccessibilityPriorityLevel.low.rawValue)

        // An alert dialog over it with no focus in it is read where VoiceOver
        // lands, on it, and not named again; closed, the field is told
        // again as the modal it is in comes back, and that dialog, still
        // up, is not named again.
        p.apply(wireBatch([
            ["op": "create", "id": 30, "kind": "view",
             "props": ["accessibilityRole": "alertdialog", "accessibilityModal": "true", "accessibilityLabel": "Discard this environment?"]],
            ["op": "children", "id": 1, "ids": [2, 10, 30]],
            ["op": "frame", "id": 30, "x": 80.0, "y": 80.0, "w": 200.0, "h": 80.0],
        ]))
        XCTAssertTrue(p.macAccessibilityModal === p.views[30], "the alert dialog is the modal")
        XCTAssertEqual(heard(), ["AXLayoutChanged AXScrollArea Discard this environment?"], "no focus in it: VoiceOver goes to the dialog")
        p.apply(wireBatch([["op": "children", "id": 1, "ids": [2, 10]]]))
        XCTAssertEqual(heard(), ["AXLayoutChanged AXGroup Rename project Project title", "AXFocusedUIElementChanged AXTextField Project title"])
    }

    /// AppKit posts the focus change itself as a view becomes the first
    /// responder of the active application's key window (measured), and the
    /// landing then posts only its layout change. A window that is key in
    /// an application that is not active hears nothing from AppKit, so the
    /// landing tells the focus, once.
    func testTheFocusIsToldOnceWhenAppKitDoesNotTellIt() throws {
        let p = try fixture([])
        p.views[2]?.props["id"] = "trigger"
        // A test process is not the active application.
        final class KeyWindow: NSWindow { override var isKeyWindow: Bool { true } }
        window.orderOut(nil)
        window = KeyWindow(contentRect: NSRect(x: 0, y: 0, width: 400, height: 300), styleMask: [.titled], backing: .buffered, defer: false)
        window.isReleasedWhenClosed = false
        window.contentView = p.viewport
        window.orderFront(nil)
        let client = AXClient()
        try client.listen(["AXLayoutChanged", "AXFocusedUIElementChanged"])
        func heard() -> [String] {
            defer { client.clear() }
            return client.notifications().map { note in
                let named = (note.info["AXUIElementsKey"] as? [AXUIElement])?.compactMap { AXClient.value($0, "AXDescription") as? String } ?? []
                return ([note.name, client.value(of: note.element, "AXRole") ?? "", client.value(of: note.element, "AXDescription") ?? ""] + named)
                    .filter { !$0.isEmpty }.joined(separator: " ")
            }
        }
        p.apply(wireBatch([
            ["op": "create", "id": 10, "kind": "view",
             "props": ["accessibilityRole": "dialog", "accessibilityModal": "true", "accessibilityLabel": "Rename project", "testId": "rename"]],
            ["op": "create", "id": 11, "kind": "input", "handlers": ["input"], "props": ["value": "", "accessibilityLabel": "Project title", "autofocus": "true"]],
            ["op": "children", "id": 10, "ids": [11]],
            ["op": "children", "id": 1, "ids": [2, 10]],
            ["op": "frame", "id": 10, "x": 50.0, "y": 50.0, "w": 300.0, "h": 150.0],
            ["op": "frame", "id": 11, "x": 10.0, "y": 10.0, "w": 280.0, "h": 24.0],
        ]))
        p.syncAccessibility()
        XCTAssertFalse(NSApp.isActive)
        XCTAssertTrue(p.focusedNode === p.views[11], "the field took the focus")
        XCTAssertEqual(heard().filter { !$0.hasPrefix("AXAnnouncement") },
                       ["AXLayoutChanged AXGroup Rename project Project title", "AXFocusedUIElementChanged AXTextField Project title"])
        // Closed, its commit's `focus()` gives the trigger the focus back.
        p.apply(wireBatch([["op": "children", "id": 1, "ids": [2]]]))
        p.focusElement(["trigger"])
        XCTAssertEqual(heard(), ["AXLayoutChanged AXScrollArea More actions", "AXFocusedUIElementChanged AXButton More actions"])
    }

    /// A dialog in a popover whose field autofocuses (the font picker),
    /// opened by VO-Space on its trigger, sends VoiceOver to the field and
    /// names the dialog after it. The layout change is posted on the dialog:
    /// posted on the page's scroll view, VoiceOver went to the scroll view's
    /// child that holds the field, the dialog, said "Interface font family,
    /// dialog" and stayed there (realinput-1016i H-10; measured beside
    /// AppKit's `NSPopover`, which VoiceOver reads as "Search fonts, search
    /// text field", then "In …, popover"). AppKit's popover names itself as
    /// the focus enters it, so the host names only one painted in the page.
    func testAPopoverDialogSendsVoiceOverToTheFieldItFocused() throws {
        let p = try fixture([
            ["op": "create", "id": 20, "kind": "view",
             "props": ["popover": "auto", "accessibilityRole": "dialog", "accessibilityModal": "true", "accessibilityLabel": "Interface font family", "testId": "picker"]],
            ["op": "create", "id": 21, "kind": "view", "props": ["accessibilityRole": "group", "accessibilityLabel": "Search", "testId": "search-group"]],
            ["op": "create", "id": 22, "kind": "input", "handlers": ["input"], "props": ["value": "", "accessibilityLabel": "Search fonts", "testId": "search", "autofocus": "true"]],
            ["op": "children", "id": 21, "ids": [22]],
            ["op": "children", "id": 20, "ids": [21]],
            ["op": "children", "id": 1, "ids": [2, 20]],
            ["op": "frame", "id": 20, "x": 0.0, "y": 40.0, "w": 300.0, "h": 150.0],
            ["op": "frame", "id": 21, "x": 10.0, "y": 10.0, "w": 280.0, "h": 24.0],
            ["op": "frame", "id": 22, "x": 0.0, "y": 0.0, "w": 280.0, "h": 24.0],
        ])
        let client = AXClient()
        try client.listen(["AXLayoutChanged", "AXFocusedUIElementChanged", "AXAnnouncementRequested"])
        func heard() -> [String] {
            defer { client.clear() }
            return client.notifications().map { note in
                let named = (note.info["AXUIElementsKey"] as? [AXUIElement])?.compactMap { AXClient.value($0, "AXDescription") as? String } ?? []
                let words = (note.info["AXAnnouncementKey"] as? String).map { ["\"\($0)\"", "priority \(note.info["AXPriorityKey"] ?? "")"] } ?? []
                return ([note.name, client.value(of: note.element, "AXRole") ?? "", client.value(of: note.element, "AXDescription") ?? ""] + named + words)
                    .filter { !$0.isEmpty }.joined(separator: " ")
            }
        }
        XCTAssertTrue(window.makeFirstResponder(p.views[2]))
        _ = heard()
        let picker = try XCTUnwrap(p.views[20]), trigger = try XCTUnwrap(p.views[2])
        p.menus.show(picker, from: trigger)
        XCTAssertTrue(p.focusedNode === p.views[22], "the field took the focus")
        XCTAssertTrue(p.macAccessibilityModal === picker, "the picker is the modal")
        let low = NSAccessibilityPriorityLevel.low.rawValue
        XCTAssertEqual(heard(), ["AXLayoutChanged AXGroup Interface font family Search fonts", "AXFocusedUIElementChanged AXTextField Search fonts",
                                 "AXAnnouncementRequested AXGroup Interface font family \"Interface font family, dialog\" priority \(low)"])
        // Closed with the focus given back: VoiceOver goes to the trigger, from the page.
        p.menus.close(picker)
        XCTAssertEqual(heard(), ["AXLayoutChanged AXScrollArea More actions", "AXFocusedUIElementChanged AXButton More actions"])

        // Closed before it is named: it is not named.
        p.menus.show(picker, from: trigger)
        RunLoop.main.run(until: Date(timeIntervalSinceNow: 0.05))
        p.menus.close(picker)
        XCTAssertFalse(heard().contains { $0.hasPrefix("AXAnnouncementRequested") }, "a dialog gone is not named")

        // AppKit's popover: VoiceOver names it as the focus enters it, so the host does not.
        p.menus.painted = false
        p.menus.show(picker, from: trigger)
        XCTAssertTrue(p.focusedNode === p.views[22], "the field took the focus")
        XCTAssertTrue(picker.window.map(p.menus.hosts) ?? false, "the picker is in AppKit's popover")
        XCTAssertEqual(heard(), ["AXLayoutChanged AXGroup Interface font family Search fonts", "AXFocusedUIElementChanged AXTextField Search fonts"])
        p.menus.close(picker)
    }

    /// A named `region` is AppKit's group with the landmark-region subrole
    /// and says "region", as Chromium and WebKit serve one (Core-AAM); an
    /// unnamed one is no landmark (the T3 Code clone's zoomable image).
    func testANamedRegionIsALandmarkRegion() throws {
        let p = try fixture([
            ["op": "create", "id": 10, "kind": "view", "handlers": ["key"],
             "props": ["accessibilityRole": "region", "accessibilityLabel": "large-grid.png, zoomable image", "tabindex": "0", "testId": "zoom"]],
            ["op": "create", "id": 11, "kind": "view", "props": ["accessibilityRole": "region", "testId": "plain"]],
            ["op": "create", "id": 12, "kind": "button", "handlers": ["press"], "props": ["accessibilityLabel": "Inside"]],
            ["op": "children", "id": 11, "ids": [12]],
            ["op": "children", "id": 1, "ids": [2, 10, 11]],
            ["op": "frame", "id": 10, "x": 0.0, "y": 40.0, "w": 200.0, "h": 150.0],
            ["op": "frame", "id": 11, "x": 0.0, "y": 200.0, "w": 200.0, "h": 50.0],
            ["op": "frame", "id": 12, "x": 0.0, "y": 0.0, "w": 100.0, "h": 30.0],
        ])
        let region = try XCTUnwrap(AXClient().attributes(of: "zoom", ["AXRole", "AXSubrole", "AXRoleDescription", "AXDescription"]))
        XCTAssertEqual(region, ["AXRole": "AXGroup", "AXSubrole": "AXLandmarkRegion", "AXRoleDescription": "region",
                                "AXDescription": "large-grid.png, zoomable image"])
        XCTAssertNil(AXClient().attributes(of: "plain", ["AXRole"]), "an unnamed region is no element")
        let tree = p.axElements(roots: [p.viewport])["elements"] as? [[String: Any]] ?? []
        XCTAssertEqual(tree.first { $0["testId"] as? String == "zoom" }?["role"] as? String, "region")
    }

    /// An `aria-modal` box with no role (a row that records a shortcut,
    /// mounted around the field the focus went to) is served by its
    /// elements: the page's scroll view holds its field, whose parent is the
    /// scroll view, and VoiceOver is sent to the field from the scroll view.
    /// Served as itself it was an `AXUnknown` holding a field whose parent
    /// named the scroll view, the layout change posted on it reached no
    /// client, and VoiceOver's cursor stayed on the scroll area while the
    /// keyboard was in the field (realinput-1017k FN15), then lost the field
    /// to the window (F-2).
    func testARoleLessModalIsServedByItsElements() throws {
        let p = try fixture([
            ["op": "create", "id": 5, "kind": "view", "props": ["accessibilityRole": "group", "accessibilityLabel": "Keybindings", "testId": "list"]],
            ["op": "create", "id": 6, "kind": "view", "handlers": ["focusin"], "props": ["testId": "idle-row"]],
            ["op": "create", "id": 7, "kind": "input", "handlers": ["keycapture", "blur"],
             "props": ["value": "", "accessibilityLabel": "Keybinding for new keybinding", "placeholder": "Unassigned", "testId": "idle"]],
            ["op": "children", "id": 6, "ids": [7]],
            ["op": "children", "id": 5, "ids": [6]],
            ["op": "children", "id": 1, "ids": [2, 5]],
            ["op": "frame", "id": 5, "x": 0.0, "y": 40.0, "w": 400.0, "h": 100.0],
            ["op": "frame", "id": 6, "x": 0.0, "y": 10.0, "w": 300.0, "h": 30.0],
            ["op": "frame", "id": 7, "x": 0.0, "y": 0.0, "w": 280.0, "h": 24.0],
        ])
        XCTAssertTrue(window.makeFirstResponder(p.keyView(of: try XCTUnwrap(p.views[7]))))
        RunLoop.main.run(until: Date(timeIntervalSinceNow: 0.05))
        let client = AXClient()
        try client.listen(["AXLayoutChanged", "AXFocusedUIElementChanged"])
        // The focus in the idle field starts recording: the app swaps in the recorder, modal, its field autofocused.
        p.apply(wireBatch([
            ["op": "create", "id": 10, "kind": "view", "handlers": ["focusin"], "props": ["accessibilityModal": "true", "testId": "recording-row"]],
            ["op": "create", "id": 11, "kind": "input", "handlers": ["keycapture", "blur"],
             "props": ["value": "", "accessibilityLabel": "Keybinding for new keybinding", "placeholder": "Press shortcut", "testId": "recorder", "autofocus": "true"]],
            ["op": "children", "id": 10, "ids": [11]],
            ["op": "children", "id": 5, "ids": [10]],
            ["op": "frame", "id": 10, "x": 0.0, "y": 10.0, "w": 300.0, "h": 30.0],
            ["op": "frame", "id": 11, "x": 0.0, "y": 0.0, "w": 280.0, "h": 24.0],
        ]))
        p.syncAccessibility()
        XCTAssertTrue(p.macAccessibilityModal === p.views[10], "the row is the modal")
        XCTAssertTrue(p.focusedNode === p.views[11], "the recorder took the focus")
        let heard = client.notifications().map { note in
            let named = (note.info["AXUIElementsKey"] as? [AXUIElement])?.compactMap { AXClient.value($0, "AXDescription") as? String } ?? []
            return ([note.name, client.value(of: note.element, "AXRole") ?? "", client.value(of: note.element, "AXDescription") ?? ""] + named)
                .filter { !$0.isEmpty }.joined(separator: " ")
        }
        XCTAssertEqual(heard, ["AXLayoutChanged AXScrollArea Keybinding for new keybinding",
                               "AXFocusedUIElementChanged AXTextField Keybinding for new keybinding"])
        // What a client walks: the scroll view's one child is the field, and the field's parent is the scroll view.
        let app = AXUIElementCreateApplication(getpid())
        let served: [String] = { () -> [String] in
            let done = DispatchSemaphore(value: 0)
            nonisolated(unsafe) var out: [String] = []
            DispatchQueue.global().async {
                func role(_ e: AXUIElement) -> String { AXClient.value(e, "AXRole") as? String ?? "?" }
                let window = (AXClient.value(app, "AXChildren") as? [AXUIElement])?.first { role($0) == "AXWindow" }
                let scroll = (window.flatMap { AXClient.value($0, "AXChildren") as? [AXUIElement] } ?? []).first { role($0) == "AXScrollArea" }
                for child in scroll.flatMap({ AXClient.value($0, "AXChildren") as? [AXUIElement] }) ?? [] {
                    let parent = AXClient.value(child, "AXParent").map { role($0 as! AXUIElement) } ?? "none"
                    out.append("\(role(child)) \(AXClient.value(child, "AXDescription") as? String ?? "") in \(parent)")
                }
                done.signal()
            }
            while done.wait(timeout: .now()) == .timedOut { RunLoop.main.run(mode: .default, before: Date(timeIntervalSinceNow: 0.005)) }
            return out
        }()
        XCTAssertEqual(served, ["AXTextField Keybinding for new keybinding in AXScrollArea"])
        XCTAssertFalse(client.identifiers().contains("recording-row"), "the role-less row is no element")
    }

    private func escape(_ window: NSWindow, up: Bool = false) -> NSEvent {
        NSEvent.keyEvent(with: up ? .keyUp : .keyDown, location: .zero, modifierFlags: [], timestamp: 0,
            windowNumber: window.windowNumber, context: nil, characters: "\u{1b}", charactersIgnoringModifiers: "\u{1b}", isARepeat: false, keyCode: 53)!
    }
}
// AX reads initialize AppKit's accessibility machinery. Keep them in a process
// apart from the observer tests, which measure its initial native notifications.
final class ComboboxAccessibilityClientMacTests: XCTestCase {
    func testAComboboxInputServesItsRoleValueAndExpandedStateToVoiceOver() throws {
        guard AXIsProcessTrusted() else { throw XCTSkip("this process is not trusted for accessibility") }
        _ = NSApplication.shared
        NSApp.setActivationPolicy(.accessory)
        NSApp.finishLaunching()
        for type in ["search", "text"] {
            let label = "Book (\(type))"
            let p = Presenter()
            let window = NSWindow(contentRect: NSRect(x: 0, y: 0, width: 400, height: 300), styleMask: [.titled], backing: .buffered, defer: false)
            window.isReleasedWhenClosed = false
            window.contentView = p.viewport
            defer { window.close() }
            p.apply(wireBatch([
                ["op": "create", "id": 1, "kind": "view"],
                ["op": "roots", "ids": [1]],
                ["op": "frame", "id": 1, "x": 0.0, "y": 0.0, "w": 400.0, "h": 300.0],
                ["op": "create", "id": 11, "kind": "input", "handlers": ["input"],
                 "props": ["type": type, "value": "Pira", "accessibilityRole": "combobox", "accessibilityLabel": label, "accessibilityExpanded": "false", "testId": "book-combo"]],
                ["op": "children", "id": 1, "ids": [11]],
                ["op": "frame", "id": 11, "x": 10.0, "y": 40.0, "w": 280.0, "h": 24.0],
            ]))
            p.syncAccessibility()
            window.orderFront(nil)
            window.layoutIfNeeded()
            let client = AXClient(), attributes = ["AXRole", "AXRoleDescription", "AXDescription", "AXValue", "AXExpanded"]
            let field = try XCTUnwrap(p.views[11]?.field)
            XCTAssertEqual(field is NSSearchField, type == "search")
            XCTAssertEqual(field.accessibilityRole(), .comboBox)
            XCTAssertEqual(field.cell?.accessibilityAttributeValue(.role) as? String, "AXComboBox")
            let collapsed = try XCTUnwrap(client.attributes(of: label, attributes, matching: "AXDescription"), "the \(type) input")
            XCTAssertEqual(collapsed["AXRole"], "AXComboBox")
            XCTAssertEqual(collapsed["AXRoleDescription"], NSAccessibility.Role.comboBox.description(with: nil))
            XCTAssertEqual(collapsed["AXDescription"], label)
            XCTAssertEqual(collapsed["AXValue"], "Pira")
            XCTAssertEqual(collapsed["AXExpanded"], "0")
            p.apply(wireBatch([["op": "props", "id": 11, "set": ["accessibilityExpanded": "true", "value": "Piranesi"]]]))
            let expanded = try XCTUnwrap(client.attributes(of: label, attributes, matching: "AXDescription"))
            XCTAssertEqual(expanded["AXExpanded"], "1")
            XCTAssertEqual(expanded["AXValue"], "Piranesi")
            p.apply(wireBatch([["op": "props", "id": 11, "clear": ["accessibilityRole", "accessibilityExpanded"]]]))
            XCTAssertEqual(client.attributes(of: label, attributes, matching: "AXDescription")?["AXRole"], "AXTextField")
        }
    }

}
#endif
