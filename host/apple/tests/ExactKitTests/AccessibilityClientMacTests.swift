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
    private var heard: [(name: String, info: [String: Any])] = []
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
            let callback: AXObserverCallbackWithInfo = { _, _, name, info, context in
                let client = Unmanaged<AXClient>.fromOpaque(context!).takeUnretainedValue()
                client.lock.lock(); defer { client.lock.unlock() }
                client.heard.append((name as String, info as? [String: Any] ?? [:]))
            }
            guard AXObserverCreateWithInfoCallback(getpid(), callback, &observer) == .success, let observer else { return false }
            for name in names where AXObserverAddNotification(observer, app, name as CFString, Unmanaged.passUnretained(self).toOpaque()) != .success { return false }
            return true
        }
        guard ok, let observer else { throw XCTSkip("no AX observer on this process") }
        CFRunLoopAddSource(CFRunLoopGetMain(), AXObserverGetRunLoopSource(observer), .defaultMode)
    }
    /// What was heard once the main run loop has turned for `seconds`.
    func notifications(after seconds: TimeInterval = 0.3) -> [(name: String, info: [String: Any])] {
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

        // A plain dialog is not announced: VoiceOver reads it as it moves there.
        client.clear()
        p.apply(wireBatch([["op": "props", "id": 10, "set": ["accessibilityModal": "false"]]]))
        p.syncAccessibility(changed: [10])
        p.apply(wireBatch([["op": "props", "id": 10, "set": ["accessibilityModal": "true", "accessibilityRole": "dialog"]]]))
        p.syncAccessibility(changed: [10])
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
    }
}
#endif
