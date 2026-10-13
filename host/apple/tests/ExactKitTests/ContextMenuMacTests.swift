#if os(macOS)
import AppKit
import XCTest
@testable import ExactKit

/// LLP 1021 §5.1 on AppKit: a context menu is its popover's menu rows as an
/// NSMenu (a Mac's has no preview, so the `contextPreview` row is no item),
/// and a picked item presses its row on the next turn, as a button menu's.
final class ContextMenuMacTests: XCTestCase {
    private var windows: [NSWindow] = []
    private var sessions: [ExactSession] = []
    override func tearDown() { windows.forEach { $0.close() }; windows.removeAll(); sessions.forEach { $0.destroy() }; sessions.removeAll() }

    /// A source (1) naming popover 2: a preview row (3) and two items (4, 5)
    /// with an `hr` (6) between them. `inSession`: a session's presenter,
    /// which lays out paragraphs.
    private func presenter(inSession: Bool = false) -> Presenter {
        _ = NSApplication.shared
        let session = inSession ? ExactApp.shared.makeSession(label: "context-menu") : nil
        if let session { sessions.append(session) }
        let p = session?.presenter ?? Presenter()
        let window = NSWindow(contentRect: NSRect(x: 0, y: 0, width: 500, height: 400), styleMask: [.titled], backing: .buffered, defer: false)
        window.isReleasedWhenClosed = false
        window.contentView = p.viewport
        windows.append(window)
        func text(_ id: Int, _ value: String) -> [[String: Any]] {
            [["op": "create", "id": id, "kind": "text", "props": ["text": value]], ["op": "frame", "id": id, "x": 0, "y": 0, "w": 180, "h": 20]]
        }
        func row(_ id: Int, _ label: Int, _ props: [String: String] = [:]) -> [[String: Any]] {
            [["op": "create", "id": id, "kind": "button", "handlers": ["press"], "props": ["popovertarget": "m", "popovertargetaction": "hide"].merging(props) { $1 }],
             ["op": "frame", "id": id, "x": 0, "y": 0, "w": 200, "h": 40], ["op": "children", "id": id, "ids": [label]]]
        }
        let ops: [[String: Any]] = [
            ["op": "create", "id": 1, "kind": "button", "handlers": ["press", "contextmenu"], "props": ["contextPopover": "m"]],
            ["op": "create", "id": 2, "kind": "view", "props": ["popover": "auto", "id": "m", "accessibilityRole": "menu"]],
            ["op": "create", "id": 6, "kind": "view", "props": ["semanticTag": "hr"]],
            ["op": "create", "id": 9, "kind": "view"],
        ] + text(13, "Open the chat") + row(3, 13, ["contextPreview": "true"]) + text(14, "Pin") + row(4, 14)
            + text(15, "Delete") + row(5, 15, ["destructive": "true"])
            + [["op": "children", "id": 2, "ids": [3, 4, 6, 5]], ["op": "children", "id": 9, "ids": [1, 2]], ["op": "roots", "ids": [9]],
               ["op": "frame", "id": 9, "x": 0, "y": 0, "w": 500, "h": 400], ["op": "frame", "id": 1, "x": 20, "y": 20, "w": 120, "h": 30],
               ["op": "frame", "id": 2, "x": 0, "y": 0, "w": 220, "h": 200]]
        p.apply(wireBatch(ops))
        return p
    }

    func testTheContextMenuIsItsPopoversItemsWithoutThePreview() throws {
        let p = presenter()
        let pop = try XCTUnwrap(p.views[2])
        XCTAssertTrue(p.menus.isMenuShaped(pop), "a preview row does not keep it from being a menu")
        let menu = p.menus.menu(of: pop)
        XCTAssertEqual(menu.items.map { $0.isSeparatorItem ? "—" : $0.title }, ["Pin", "—", "Delete"])
        var pressed: [UInt32] = []
        let picked = expectation(description: "the picked row is pressed")
        p.onPress = { pressed.append($0); picked.fulfill() }
        NSApp.sendAction(try XCTUnwrap(menu.items[2].action), to: menu.items[2].target, from: menu.items[2])
        XCTAssertEqual(pressed, [], "on the next turn")
        // That turn, however late a loaded machine runs it: a fixed 50 ms
        // spin of the run loop could return before the main queue's turn.
        wait(for: [picked], timeout: 10)
        XCTAssertEqual(pressed, [5])
    }

    private func turn() {
        let turned = expectation(description: "the next turn")
        DispatchQueue.main.async { turned.fulfill() }
        wait(for: [turned], timeout: 10)
    }

    private func contextKey(_ window: NSWindow, type: NSEvent.EventType = .keyDown,
                            code: UInt16 = 0x6E, flags: NSEvent.ModifierFlags = []) -> NSEvent {
        NSEvent.keyEvent(with: type, location: NSPoint(x: 450, y: 350), modifierFlags: flags, timestamp: 0,
                         windowNumber: window.windowNumber, context: nil, characters: "\u{F735}",
                         charactersIgnoringModifiers: "\u{F735}", isARepeat: false, keyCode: code)!
    }

    /// #235: a real menu key runs the action first, then tracks the updated
    /// native menu. Its point is the focus's centre, not the mouse position.
    func testContextMenuKeyRunsTheActionThenTracksTheNativeMenu() throws {
        try XCTSkipIf(ExactEnv.agentMode, "the agent presents painted popovers")
        let p = presenter(), window = try XCTUnwrap(windows.last)
        let source = try XCTUnwrap(p.views[1])
        window.orderFrontRegardless()
        XCTAssertTrue(window.makeFirstResponder(source))
        source.handlers.insert("key")
        var heard: [String] = [], sample: [String] = []
        p.onKey = { _, key in heard.append("key \(key.chord)") }
        p.onClipboard = { id, kind, line in
            XCTAssertEqual(id, 1); XCTAssertEqual(kind, 10)
            heard.append("context")
            sample = line.components(separatedBy: ",")
            p.apply(wireBatch([["op": "props", "id": 14, "set": ["text": "Pin after context"]]]))
        }
        p.onPress = { heard.append("pick \($0)") }
        var tracked = false
        let center = NotificationCenter.default
        let observer = center.addObserver(forName: NSMenu.didBeginTrackingNotification, object: nil, queue: nil) { note in
            guard let menu = note.object as? NSMenu, menu.items.first?.title == "Pin after context" else { return }
            tracked = true
            RunLoop.current.perform(inModes: [.eventTracking, .default]) {
                menu.performActionForItem(at: 0)
                menu.cancelTracking()
            }
        }
        defer { center.removeObserver(observer) }
        XCTAssertTrue(p.routeKey(contextKey(window), focused: true))
        let deadline = Date(timeIntervalSinceNow: 5)
        while !heard.contains("pick 4") && Date() < deadline { RunLoop.main.run(mode: .default, before: Date(timeIntervalSinceNow: 0.02)) }
        XCTAssertTrue(tracked)
        XCTAssertEqual(heard, ["key ContextMenu", "context", "pick 4"])
        XCTAssertEqual(sample.prefix(4).compactMap(Double.init), [60, 15, 0, 0])
        XCTAssertEqual(sample.dropFirst(6).prefix(2).compactMap(Double.init), [80, 35])
    }

    func testContextMenuHonorsKeyCancellationAndOnlyTheUnmodifiedKeyDown() throws {
        let p = presenter(), window = try XCTUnwrap(windows.last), source = try XCTUnwrap(p.views[1])
        source.props.removeValue(forKey: "contextPopover")
        source.handlers.insert("key")
        XCTAssertTrue(window.makeFirstResponder(source))
        var heard = 0, prevent = true
        p.onKey = { _, _ in p.defaultPrevented = prevent }
        p.onClipboard = { _, _, _ in heard += 1; p.defaultPrevented = true }
        XCTAssertTrue(p.routeKey(contextKey(window), focused: true))
        XCTAssertEqual(heard, 0, "a key handler cancelled the default")
        prevent = false
        XCTAssertFalse(p.routeKey(contextKey(window, type: .keyUp), focused: true))
        XCTAssertFalse(p.routeKey(contextKey(window, code: 109, flags: .shift), focused: true), "Shift+F10")
        XCTAssertFalse(p.routeKey(contextKey(window, flags: .control), focused: true))
        XCTAssertFalse(p.routeKey(contextKey(window), focused: false), "another session has the focus")
        XCTAssertEqual(heard, 0)
        XCTAssertTrue(p.routeKey(contextKey(window), focused: true))
        XCTAssertEqual(heard, 1)
        XCTAssertFalse(p.defaultPrevented, "the context action's flag cannot leak into another event")
    }

    func testContextMenuUsesTheFocusAfterKeyHandlersAndKeepsThatContextTarget() throws {
        let p = presenter(), window = try XCTUnwrap(windows.last), source = try XCTUnwrap(p.views[1])
        source.props.removeValue(forKey: "contextPopover")
        source.handlers.insert("key")
        p.apply(wireBatch([
            ["op": "create", "id": 7, "kind": "button", "handlers": ["contextmenu"]],
            ["op": "frame", "id": 7, "x": 180, "y": 20, "w": 100, "h": 30],
            ["op": "children", "id": 9, "ids": [1, 2, 7]],
        ]))
        let second = try XCTUnwrap(p.views[7])
        XCTAssertTrue(window.makeFirstResponder(source))
        p.onKey = { _, _ in window.makeFirstResponder(second) }
        var heard: [UInt32] = []
        p.onClipboard = { id, _, _ in heard.append(id); window.makeFirstResponder(source) }
        XCTAssertTrue(p.routeKey(contextKey(window), focused: true))
        XCTAssertEqual(heard, [7], "the key action moved focus; the context action does not retarget itself")
    }

    func testContextMenuCarriesDrawnCoordinatesToTheNearestHandler() throws {
        let p = presenter(), window = try XCTUnwrap(windows.last), source = try XCTUnwrap(p.views[1])
        source.props.removeValue(forKey: "contextPopover")
        p.apply(wireBatch([
            ["op": "create", "id": 7, "kind": "button"],
            ["op": "frame", "id": 7, "x": 10, "y": 4, "w": 20, "h": 10],
            ["op": "children", "id": 1, "ids": [7]],
            ["op": "present", "id": 1, "property": "translate", "x": 100.0, "y": 0.0],
        ]))
        XCTAssertTrue(window.makeFirstResponder(try XCTUnwrap(p.views[7])))
        var heard: [UInt32] = [], sample: [String] = []
        p.onClipboard = { id, _, line in heard.append(id); sample = line.components(separatedBy: ",") }
        XCTAssertTrue(p.routeKey(contextKey(window), focused: true))
        XCTAssertEqual(heard, [1])
        XCTAssertEqual(sample.prefix(2).compactMap(Double.init), [20, 9])
        XCTAssertEqual(sample.dropFirst(6).prefix(2).compactMap(Double.init), [140, 29])
    }

    func testContextMenuKeepsFieldEditingAndCompositionWithTheirOwner() throws {
        let p = presenter(), window = try XCTUnwrap(windows.last)
        p.apply(wireBatch([
            ["op": "create", "id": 7, "kind": "input", "props": ["value": ""]],
            ["op": "frame", "id": 7, "x": 0, "y": 0, "w": 100, "h": 30],
            ["op": "children", "id": 1, "ids": [7]],
        ]))
        XCTAssertTrue(window.makeFirstResponder(try XCTUnwrap(p.views[7]?.field)))
        var heard = 0
        p.onClipboard = { _, _, _ in heard += 1 }
        XCTAssertFalse(p.routeKey(contextKey(window), focused: true))
        let editor = try XCTUnwrap(window.firstResponder as? NSTextView)
        editor.setMarkedText("ㅎ", selectedRange: NSRange(location: 1, length: 0), replacementRange: NSRange(location: NSNotFound, length: 0))
        XCTAssertFalse(p.routeKey(contextKey(window), focused: true))
        XCTAssertTrue(editor.hasMarkedText())
        XCTAssertEqual(heard, 0, "an ancestor's context menu cannot take the editor's key")
    }

    func testContextMenuFindsAFrameTranslatedIntoView() throws {
        let p = presenter(), window = try XCTUnwrap(windows.last), source = try XCTUnwrap(p.views[1])
        source.props.removeValue(forKey: "contextPopover")
        p.apply(wireBatch([
            ["op": "frame", "id": 1, "x": -150, "y": 20, "w": 120, "h": 30],
            ["op": "present", "id": 1, "property": "translate", "x": 200.0, "y": 0.0],
        ]))
        XCTAssertTrue(window.makeFirstResponder(source))
        var sample: [String] = []
        p.onClipboard = { _, _, line in sample = line.components(separatedBy: ",") }
        XCTAssertTrue(p.routeKey(contextKey(window), focused: true))
        XCTAssertEqual(sample.prefix(2).compactMap(Double.init), [60, 15])
        XCTAssertEqual(sample.dropFirst(6).prefix(2).compactMap(Double.init), [110, 35])
    }

    func testContextMenuFallsBackInsideItsSessionWhenTheKeyHandlerRemovesFocus() throws {
        let p = presenter(), window = try XCTUnwrap(windows.last), source = try XCTUnwrap(p.views[1])
        source.handlers.insert("key")
        p.views[9]?.handlers.insert("contextmenu")
        XCTAssertTrue(window.makeFirstResponder(source))
        p.onKey = { _, _ in
            p.apply(wireBatch([["op": "children", "id": 9, "ids": [2]], ["op": "destroy", "id": 1]]))
            window.makeFirstResponder(p.viewport)
        }
        var heard: [UInt32] = [], sample: [String] = []
        p.onClipboard = { id, _, line in heard.append(id); sample = line.components(separatedBy: ",") }
        XCTAssertTrue(p.routeKey(contextKey(window), focused: true))
        XCTAssertEqual(heard, [9])
        XCTAssertEqual(sample.prefix(2).compactMap(Double.init), [1, 1])
    }

    private final class NativeKeySink: NSView { override var acceptsFirstResponder: Bool { true } }
    func testContextMenuDoesNotTakeAnEmbeddedViewsKeys() throws {
        let p = presenter(), window = try XCTUnwrap(windows.last), source = try XCTUnwrap(p.views[1])
        let embedded = NativeKeySink(frame: source.bounds)
        source.addSubview(embedded)
        XCTAssertTrue(window.makeFirstResponder(embedded))
        var heard = 0
        p.onClipboard = { _, _, _ in heard += 1 }
        XCTAssertFalse(p.routeKey(contextKey(window), focused: true))
        XCTAssertEqual(heard, 0)
    }

    /// The action names AppKit's accessibility server gives an assistive
    /// app for `element`: its `CopyActionNames` handler calls this function.
    /// The in-process `accessibilityActionNames()` lists legacy ones only.
    private func offered(_ element: Any) throws -> Set<String> {
        typealias Names = @convention(c) (AnyObject) -> Unmanaged<NSArray>?
        let symbol = try XCTUnwrap(dlsym(UnsafeMutableRawPointer(bitPattern: -2), "NSAccessibilityEntryPointActionNames"))
        return Set(unsafeBitCast(symbol, to: Names.self)(element as AnyObject)?.takeUnretainedValue() as? [String] ?? [])
    }

    /// X84: VoiceOver's show menu (AXShowMenu, VO-⇧-M) takes the ContextMenu
    /// key's path at the node, as Chrome sends both through
    /// `ShowNonLocatedContextMenu`: its `contextmenu` at its drawn middle,
    /// and a link run's at the run. It is offered only where one answers.
    func testShowMenuRunsTheContextActionWhereOneAnswers() throws {
        let p = presenter(inSession: true), source = try XCTUnwrap(p.views[1])
        source.props.removeValue(forKey: "contextPopover")
        func run(_ id: Int, _ text: String, _ props: [String: String] = [:], _ handlers: [String] = []) -> [String: Any] {
            ["id": id, "parent": 8, "paint": true, "props": props.merging(["text": text]) { $1 }, "handlers": handlers, "style": ["font_size": 18.0]]
        }
        p.apply(wireBatch([
            ["op": "create", "id": 7, "kind": "button", "handlers": ["press"]],
            ["op": "frame", "id": 7, "x": 180, "y": 20, "w": 100, "h": 30],
            ["op": "create", "id": 8, "kind": "text", "style": ["font_size": 18.0]],
            ["op": "paragraph", "id": 8, "runs": [run(81, "Write to "), run(82, "the list", ["href": "mailto:list@example.test"], ["press", "contextmenu"]),
                                                 run(83, " or "), run(84, "the team", ["href": "mailto:team@example.test"], ["press"])]],
            ["op": "frame", "id": 8, "x": 20, "y": 80, "w": 400, "h": 28],
            ["op": "children", "id": 9, "ids": [1, 2, 7, 8]],
        ]))
        p.viewport.layoutSubtreeIfNeeded()
        let plain = try XCTUnwrap(p.views[7]), links = try XCTUnwrap(p.views[8]?.accessibilityChildren() as? [NSAccessibilityElement])
        XCTAssertEqual(links.map { $0.accessibilityLabel() }, ["the list", "the team"])
        XCTAssertEqual(try offered(source), ["AXPress", "AXShowMenu"])
        XCTAssertEqual(try offered(links[0]), ["AXPress", "AXShowMenu"])
        XCTAssertEqual(try offered(plain), ["AXPress"], "nothing answers it")
        XCTAssertEqual(try offered(links[1]), ["AXPress"])
        var heard: [UInt32] = [], samples: [[String]] = []
        p.onClipboard = { id, kind, line in
            XCTAssertEqual(kind, 10)
            heard.append(id); samples.append(line.components(separatedBy: ","))
        }
        XCTAssertFalse(plain.accessibilityPerformShowMenu())
        XCTAssertTrue(source.accessibilityPerformShowMenu())
        // AppKit's server performs only an action it offers: a run's
        // element is no view, and its base show menu raises.
        if try offered(links[0]).contains("AXShowMenu") { XCTAssertTrue(links[0].accessibilityPerformShowMenu()) }
        XCTAssertEqual(heard, [1, 82])
        XCTAssertEqual(samples.first?.prefix(4).compactMap(Double.init), [60, 15, 0, 0], "the node's middle, no button held")
        XCTAssertEqual(samples.first?.dropFirst(6).prefix(2).compactMap(Double.init), [80, 35])
        let rect = links[0].accessibilityFrame(), window = try XCTUnwrap(windows.last)
        let middle = p.client(window.convertFromScreen(NSRect(x: rect.midX, y: rect.midY, width: 0, height: 0)).origin)
        if samples.count > 1 {
            XCTAssertEqual(Double(samples[1][6]) ?? 0, Double(middle.x), accuracy: 1, "the run's middle")
            XCTAssertEqual(Double(samples[1][7]) ?? 0, Double(middle.y), accuracy: 1)
            XCTAssertEqual(samples[1][2...3].compactMap(Double.init), [0, 0])
        }
        p.apply(wireBatch([["op": "props", "id": 1, "set": ["disabled": "true"]]]))
        XCTAssertEqual(try offered(source), ["AXPress"], "a disabled node hears no contextmenu")
        XCTAssertFalse(source.accessibilityPerformShowMenu())
        XCTAssertEqual(heard, [1, 82])
    }

    /// Show menu on an element with no `contextmenu` of its own reaches the
    /// first ancestor whose one answers, as the ContextMenu key's request
    /// and Chrome's (`contextmenu` at the element bubbles), at the element's
    /// drawn middle: a button in a row, a link run in a paragraph, and a link
    /// run in a span run (which the key at that run now reaches too). It is
    /// offered there and not where nothing above answers or the one that
    /// answers is disabled. (issues/20261013-macos-show-menu-ancestor-contextmenu.md)
    func testShowMenuReachesTheNearestAncestorThatAnswers() throws {
        let p = presenter(inSession: true), window = try XCTUnwrap(windows.last)
        func run(_ id: Int, _ text: String, parent: Int, _ props: [String: String] = [:], _ handlers: [String] = []) -> [String: Any] {
            ["id": id, "parent": parent, "paint": !text.isEmpty, "props": props.merging(["text": text]) { $1 }, "handlers": handlers, "style": ["font_size": 18.0]]
        }
        p.apply(wireBatch([
            ["op": "create", "id": 20, "kind": "view", "handlers": ["contextmenu"]],
            ["op": "frame", "id": 20, "x": 180, "y": 20, "w": 200, "h": 40],
            ["op": "create", "id": 21, "kind": "button", "handlers": ["press"]],
            ["op": "frame", "id": 21, "x": 10, "y": 5, "w": 100, "h": 30],
            ["op": "children", "id": 20, "ids": [21]],
            ["op": "create", "id": 8, "kind": "text", "handlers": ["contextmenu"], "style": ["font_size": 18.0]],
            ["op": "paragraph", "id": 8, "runs": [run(81, "Write to ", parent: 8), run(82, "the list", parent: 8, ["href": "mailto:list@example.test"], ["press"])]],
            ["op": "frame", "id": 8, "x": 20, "y": 80, "w": 400, "h": 28],
            ["op": "create", "id": 10, "kind": "text", "style": ["font_size": 18.0]],
            ["op": "paragraph", "id": 10, "runs": [run(101, "See ", parent: 10), run(102, "", parent: 10, [:], ["contextmenu"]),
                                                  run(103, "the docs", parent: 102, ["href": "https://example.test/docs"], ["press"])]],
            ["op": "frame", "id": 10, "x": 20, "y": 130, "w": 400, "h": 28],
            ["op": "create", "id": 7, "kind": "button", "handlers": ["press"]],
            ["op": "frame", "id": 7, "x": 20, "y": 180, "w": 100, "h": 30],
            ["op": "children", "id": 9, "ids": [1, 2, 20, 8, 10, 7]],
        ]))
        p.viewport.layoutSubtreeIfNeeded()
        let button = try XCTUnwrap(p.views[21]), plain = try XCTUnwrap(p.views[7])
        func link(_ paragraph: UInt32, _ id: UInt32) throws -> NSAccessibilityElement {
            let elements = try XCTUnwrap(p.views[paragraph]?.accessibilityChildren() as? [NSAccessibilityElement])
            return try XCTUnwrap(elements.first { ($0 as? AgentOwned)?.agentViewId == id })
        }
        let list = try link(8, 82), docs = try link(10, 103)
        XCTAssertEqual(try offered(button), ["AXPress", "AXShowMenu"], "its row answers")
        XCTAssertEqual(try offered(list), ["AXPress", "AXShowMenu"], "its paragraph answers")
        XCTAssertEqual(try offered(docs), ["AXPress", "AXShowMenu"], "its span run answers")
        XCTAssertEqual(try offered(plain), ["AXPress"], "nothing above answers")
        var heard: [UInt32] = [], samples: [[String]] = []
        p.onClipboard = { id, kind, line in
            XCTAssertEqual(kind, 10)
            heard.append(id); samples.append(line.components(separatedBy: ","))
        }
        XCTAssertFalse(plain.accessibilityPerformShowMenu())
        XCTAssertTrue(button.accessibilityPerformShowMenu())
        if try offered(list).contains("AXShowMenu") { XCTAssertTrue(list.accessibilityPerformShowMenu()) }
        if try offered(docs).contains("AXShowMenu") { XCTAssertTrue(docs.accessibilityPerformShowMenu()) }
        XCTAssertEqual(heard, [20, 8, 102])
        XCTAssertEqual(samples.first?.prefix(4).compactMap(Double.init), [60, 20, 0, 0], "the button's middle in its row, no button held")
        XCTAssertEqual(samples.first?.dropFirst(6).prefix(2).compactMap(Double.init), [240, 40])
        // The run's middle, where Chrome's request lands on an inline element.
        func middle(_ element: NSAccessibilityElement) -> NSPoint {
            let rect = element.accessibilityFrame()
            return p.client(window.convertFromScreen(NSRect(x: rect.midX, y: rect.midY, width: 0, height: 0)).origin)
        }
        for (sample, element) in zip(samples.dropFirst(), [list, docs]) where sample.count > 7 {
            XCTAssertEqual(Double(sample[6]) ?? 0, Double(middle(element).x), accuracy: 1, "the run's middle")
            XCTAssertEqual(Double(sample[7]) ?? 0, Double(middle(element).y), accuracy: 1)
            XCTAssertEqual(sample[2...3].compactMap(Double.init), [0, 0])
        }
        // The ContextMenu key at the focused link run takes the same walk.
        heard = []; samples = []
        let key = try XCTUnwrap(p.views[10]?.syncInlineKeyViews().first { $0.id == 103 })
        XCTAssertTrue(window.makeFirstResponder(key))
        XCTAssertTrue(p.routeKey(contextKey(window), focused: true))
        XCTAssertEqual(heard, [102], "the span run's, as on the web")
        XCTAssertEqual(Double(samples.first?[6] ?? "") ?? 0, Double(middle(docs).x), accuracy: 1)
        // An ancestor that answers but is disabled: nothing, as for the key.
        heard = []
        p.apply(wireBatch([["op": "props", "id": 20, "set": ["disabled": "true"]]]))
        XCTAssertEqual(try offered(button), ["AXPress"])
        XCTAssertFalse(button.accessibilityPerformShowMenu())
        XCTAssertEqual(heard, [])
    }

    /// X84: show menu on a node with a `contextPopover` runs its action and
    /// then tracks the popover's native menu, as the ContextMenu key does. A
    /// default `button`'s accessibility element is its `NSButton`'s cell
    /// (LLP 1069.011 D4; AppKit ignores the button), and its show menu is
    /// its node's: a `contextPopover` is all such a button can have for it.
    func testShowMenuOpensTheContextPopoversMenu() throws {
        try XCTSkipIf(ExactEnv.agentMode, "the agent presents painted popovers")
        let p = presenter(), window = try XCTUnwrap(windows.last), source = try XCTUnwrap(p.views[1])
        func native(_ id: Int, _ props: [String: String], x: Int) -> [[String: Any]] {
            [["op": "create", "id": id, "kind": "control", "props": ["type": "button"].merging(props) { $1 }, "handlers": ["press"], "style": ["appearance": "auto"]],
             ["op": "frame", "id": id, "x": x, "y": 20, "w": 100, "h": 30]]
        }
        p.apply(wireBatch(native(10, ["contextPopover": "m"], x: 180) + native(11, [:], x: 300) + [["op": "children", "id": 9, "ids": [1, 2, 10, 11]]]))
        let button = try XCTUnwrap(p.controls.controls[10]?.cell), plain = try XCTUnwrap(p.controls.controls[11]?.cell)
        XCTAssertTrue(button.isAccessibilityElement())
        XCTAssertEqual(try offered(button), ["AXPress", "AXShowMenu"])
        XCTAssertEqual(try offered(plain), ["AXPress"], "nothing answers it")
        XCTAssertFalse(plain.accessibilityPerformShowMenu())
        window.orderFrontRegardless()
        var heard: [String] = []
        p.onClipboard = { id, _, _ in heard.append("context \(id)") }
        p.onPress = { heard.append("pick \($0)") }
        let center = NotificationCenter.default
        let observer = center.addObserver(forName: NSMenu.didBeginTrackingNotification, object: nil, queue: nil) { note in
            guard let menu = note.object as? NSMenu, menu.items.first?.title == "Pin" else { return }
            heard.append("menu")
            RunLoop.current.perform(inModes: [.eventTracking, .default]) {
                menu.performActionForItem(at: 0)
                menu.cancelTracking()
            }
        }
        defer { center.removeObserver(observer) }
        for (showMenu, expected) in [(source.accessibilityPerformShowMenu, ["context 1", "menu", "pick 4"]), (button.accessibilityPerformShowMenu, ["menu", "pick 4"])] {
            heard = []
            XCTAssertTrue(showMenu())
            let deadline = Date(timeIntervalSinceNow: 5)
            while !heard.contains("pick 4") && Date() < deadline { RunLoop.main.run(mode: .default, before: Date(timeIntervalSinceNow: 0.02)) }
            XCTAssertEqual(heard, expected)
        }
    }

    /// A click at window point `point` as a hand's comes, through the
    /// application: with Control held, AppKit asks the view under it for
    /// `menu(for:)` before any `mouseDown`. The up is queued first for a
    /// control's tracking loop.
    private func click(_ window: NSWindow, at point: NSPoint, flags: NSEvent.ModifierFlags = []) {
        func event(_ type: NSEvent.EventType) -> NSEvent {
            NSEvent.mouseEvent(with: type, location: point, modifierFlags: flags, timestamp: ProcessInfo.processInfo.systemUptime,
                               windowNumber: window.windowNumber, context: nil, eventNumber: 1, clickCount: 1, pressure: type == .leftMouseDown ? 1 : 0)!
        }
        let up = event(.leftMouseUp)
        clicksArriveAsInTheKeyWindow() // a test's process is never active (LLP 1115.000 D7.9)
        NSApp.postEvent(up, atStart: true)
        NSApp.sendEvent(event(.leftMouseDown))
        if NSApp.nextEvent(matching: [.leftMouseUp], until: nil, inMode: .default, dequeue: true) != nil { NSApp.sendEvent(up) }
    }

    /// X85: a Control-click is the Mac's secondary click, as Chrome sends it:
    /// `contextmenu` at its point, with the primary button and Control held,
    /// and no `click`. On a link (on its title), a drawn button, a box and a
    /// native button whose `contextmenu` or `contextPopover` answers, it runs
    /// that as a right-click does and presses nothing. On selectable text
    /// with no authored handler it selects the word and opens the text menu
    /// (#454), and a plain click still presses.
    func testAControlClickIsTheContextClickWhereOneAnswers() throws {
        try XCTSkipIf(ExactEnv.agentMode, "the agent opens no native menu")
        let p = presenter(inSession: true), window = try XCTUnwrap(windows.last)
        func node(_ id: Int, _ kind: String, _ props: [String: String], _ handlers: [String], y: Int, w: Int = 100, style: [String: Any] = [:]) -> [[String: Any]] {
            [["op": "create", "id": id, "kind": kind, "props": props, "handlers": handlers, "style": style], ["op": "frame", "id": id, "x": 20, "y": y, "w": w, "h": 30]]
        }
        func title(_ id: Int, _ text: String) -> [[String: Any]] {
            [["op": "create", "id": id, "kind": "text", "props": ["text": text], "style": ["font_size": 18.0]], ["op": "frame", "id": id, "x": 0, "y": 0, "w": 100, "h": 30]]
        }
        p.apply(wireBatch(node(20, "button", ["href": "#team", "accessibilityRole": "link"], ["press", "contextmenu"], y: 60) + title(21, "the team")
            + node(22, "button", [:], ["press", "contextmenu"], y: 100) + title(23, "Options")
            + node(24, "view", ["contextPopover": "m"], ["press"], y: 140)
            + node(25, "control", ["type": "button", "contextPopover": "m"], ["press"], y: 180, style: ["appearance": "auto"])
            + node(26, "text", ["text": "Alpha beta gamma"], [], y: 230, w: 300, style: ["font_size": 18.0])
            + [["op": "children", "id": 20, "ids": [21]], ["op": "children", "id": 22, "ids": [23]], ["op": "children", "id": 9, "ids": [1, 2, 20, 22, 24, 25, 26]]]))
        p.viewport.layoutSubtreeIfNeeded()
        window.orderFrontRegardless()
        var heard: [String] = [], sample: [String] = []
        p.onClipboard = { id, kind, line in
            guard kind == 10 else { return }
            heard.append("contextmenu \(id)")
            sample = line.components(separatedBy: ",")
        }
        p.onPress = { heard.append("press \($0)") }
        let center = NotificationCenter.default
        let observer = center.addObserver(forName: NSMenu.didBeginTrackingNotification, object: nil, queue: nil) { note in
            guard let menu = note.object as? NSMenu else { return }
            heard.append("menu " + (menu.items.first?.title ?? ""))
            RunLoop.current.perform(inModes: [.eventTracking, .default]) { menu.cancelTracking() }
        }
        defer { center.removeObserver(observer) }
        func controlClick(_ id: UInt32, at local: NSPoint? = nil) throws -> [String] {
            let view = try XCTUnwrap(p.views[id])
            heard = []
            click(window, at: view.convert(local ?? NSPoint(x: view.bounds.midX, y: view.bounds.midY), to: nil), flags: .control)
            // A `contextPopover`'s menu opens on the next turn; the rest is heard at once.
            let deadline = Date(timeIntervalSinceNow: 5)
            while heard.isEmpty && Date() < deadline { RunLoop.main.run(mode: .default, before: Date(timeIntervalSinceNow: 0.02)) }
            turn()
            return heard
        }
        XCTAssertEqual(try controlClick(21), ["contextmenu 20"], "the link's, from its title; no press")
        XCTAssertEqual(sample.prefix(3).compactMap(Double.init), [50, 15, 1], "at the click's point in the link, the primary button held")
        XCTAssertEqual(sample.last, "Control+")
        XCTAssertEqual(try controlClick(23), ["contextmenu 22"], "a drawn button's")
        XCTAssertEqual(try controlClick(24), ["menu Pin"], "a box's contextPopover")
        XCTAssertEqual(try controlClick(25), ["menu Pin"], "a native button's contextPopover")
        XCTAssertEqual(try controlClick(26, at: NSPoint(x: 6, y: 15)), ["menu Look Up “Alpha”"], "plain text: the word's text menu (#454)")
        XCTAssertEqual(p.selection.selectedText(), "Alpha")
        heard = []
        click(window, at: try XCTUnwrap(p.views[21]).convert(NSPoint(x: 50, y: 15), to: nil))
        XCTAssertEqual(heard, ["press 20"], "a plain click still presses")
    }

    /// X85 on AppKit's own controls: a Control-click on a checkbox, a
    /// switch, a radio, a range, a select, a date field or a tablist's
    /// segmented control inside a node whose `contextmenu` answers runs that
    /// `contextmenu`, as Chrome's does, and the control keeps its value; so
    /// does one on a checkbox with its own. A plain click still checks it.
    func testAControlClickOnANativeControlIsItsNodesContextClick() throws {
        try XCTSkipIf(ExactEnv.agentMode, "the agent opens no native menu")
        let p = presenter(inSession: true), window = try XCTUnwrap(windows.last)
        p.controls.radioGroup = { RadioGroup(json: try! JSONSerialization.data(withJSONObject: ["group": [$0], "next": NSNull(), "previous": NSNull()])) }
        p.selectOptions = { _ in SelectMenu(options: [.init(value: "a", label: "Alpha", disabled: false), .init(value: "b", label: "Beta", disabled: false)], chosen: 0) }
        func node(_ id: Int, _ kind: String, _ props: [String: String], _ handlers: [String], x: Int, y: Int = 10, w: Int = 40, h: Int = 24) -> [[String: Any]] {
            [["op": "create", "id": id, "kind": kind, "props": props, "handlers": handlers, "style": [:]], ["op": "frame", "id": id, "x": x, "y": y, "w": w, "h": h]]
        }
        p.apply(wireBatch(node(30, "view", [:], ["contextmenu"], x: 0, y: 60, w: 500, h: 80)
            + node(31, "control", ["type": "checkbox", "checked": "false"], ["change"], x: 10)
            + node(32, "control", ["type": "checkbox", "accessibilityRole": "switch", "checked": "false"], ["change"], x: 60)
            + node(33, "control", ["type": "radio", "name": "c", "value": "a", "checked": "false"], ["input", "change"], x: 110)
            + node(34, "control", ["type": "range", "value": "0"], ["input", "change"], x: 160, w: 100)
            + node(35, "control", ["type": "select"], ["input", "change"], x: 270, w: 90)
            + node(36, "control", ["type": "date"], ["input", "change"], x: 370, w: 120)
            + node(40, "view", ["accessibilityRole": "tablist"], [], x: 10, y: 44, w: 200, h: 28)
            + node(41, "button", ["accessibilityRole": "tab", "accessibilitySelected": "true"], ["press"], x: 0, y: 0, w: 100, h: 28)
            + node(42, "button", ["accessibilityRole": "tab"], ["press"], x: 100, y: 0, w: 100, h: 28)
            + node(43, "text", ["text": "One"], [], x: 0, y: 0) + node(44, "text", ["text": "Two"], [], x: 0, y: 0)
            + node(37, "control", ["type": "checkbox", "checked": "false"], ["change", "contextmenu"], x: 20, y: 160)
            + [["op": "children", "id": 41, "ids": [43]], ["op": "children", "id": 42, "ids": [44]], ["op": "children", "id": 40, "ids": [41, 42]],
               ["op": "children", "id": 30, "ids": [31, 32, 33, 34, 35, 36, 40]], ["op": "children", "id": 9, "ids": [1, 2, 30, 37]]]))
        p.viewport.layoutSubtreeIfNeeded()
        window.orderFrontRegardless()
        var heard: [String] = []
        p.onClipboard = { id, kind, _ in if kind == 10 { heard.append("contextmenu \(id)") } }
        p.onPress = { heard.append("press \($0)") }
        p.onChecked = { heard.append("checked \($0) \($1)") }
        p.onControlValue = { id, value, input, change in heard.append("\(change ? "change" : input ? "input" : "value") \(id) \(value)") }
        let center = NotificationCenter.default
        let observer = center.addObserver(forName: NSMenu.didBeginTrackingNotification, object: nil, queue: nil) { note in
            guard let menu = note.object as? NSMenu else { return }
            heard.append("menu " + (menu.items.first?.title ?? ""))
            RunLoop.current.perform(inModes: [.eventTracking, .default]) { menu.cancelTracking() }
        }
        defer { center.removeObserver(observer) }
        let segments = try XCTUnwrap(p.segments.control(of: 40))
        func controlClick(_ control: NSView) -> [String] {
            heard = []
            click(window, at: control.convert(NSPoint(x: control.bounds.midX, y: control.bounds.midY), to: nil), flags: .control)
            let deadline = Date(timeIntervalSinceNow: 2)
            while heard.isEmpty && Date() < deadline { RunLoop.main.run(mode: .default, before: Date(timeIntervalSinceNow: 0.02)) }
            turn()
            return heard
        }
        for id: UInt32 in [31, 32, 33, 34, 35] {
            let control = try XCTUnwrap(p.controls.controls[id])
            XCTAssertEqual(controlClick(control), ["contextmenu 30"], "\(type(of: control)) #\(id): the row's contextmenu, and the control's value stays")
            XCTAssertEqual(p.views[id]?.contextClicked, false, "#\(id): its release reached the node")
        }
        // A date field takes no first mouse, and this window is never key,
        // so here AppKit gives it no click at all. In a key window AppKit
        // asks it for `menu(for:)`, has none, and sends it the down, as here.
        let date = try XCTUnwrap(p.controls.controls[36])
        let middle = date.convert(NSPoint(x: date.bounds.midX, y: date.bounds.midY), to: nil)
        func event(_ type: NSEvent.EventType) -> NSEvent {
            NSEvent.mouseEvent(with: type, location: middle, modifierFlags: .control, timestamp: ProcessInfo.processInfo.systemUptime,
                               windowNumber: window.windowNumber, context: nil, eventNumber: 2, clickCount: 1, pressure: type == .leftMouseDown ? 1 : 0)!
        }
        heard = []
        XCTAssertNil(date.menu(for: event(.leftMouseDown)))
        NSApp.postEvent(event(.leftMouseUp), atStart: true)
        date.mouseDown(with: event(.leftMouseDown))
        if let up = NSApp.nextEvent(matching: [.leftMouseUp], until: nil, inMode: .default, dequeue: true) { date.mouseUp(with: up) }
        turn()
        XCTAssertEqual(heard, ["contextmenu 30"], "a date field's")
        XCTAssertEqual(p.views[36]?.contextClicked, false, "its release reached the node")
        // The segmented control takes the same `contextClicked` route as
        // the other native controls (AppKit's own right-click route does
        // not fire for it in a window that is never key).
        XCTAssertEqual(controlClick(segments), ["contextmenu 30"], "a tablist's segmented control")
        XCTAssertEqual(segments.selectedSegment, 0, "no segment chosen")
        XCTAssertEqual(controlClick(try XCTUnwrap(p.controls.controls[37])), ["contextmenu 37"], "a checkbox's own contextmenu")
        XCTAssertEqual([31, 32, 33, 37].map { p.controls.isOn(p.controls.controls[$0]!) }, [false, false, false, false], "nothing checked")
        XCTAssertEqual((p.controls.controls[34] as? NSSlider)?.doubleValue, 0, "the range did not move")
        heard = []
        click(window, at: try XCTUnwrap(p.controls.controls[31]).convert(NSPoint(x: 6, y: 6), to: nil))
        XCTAssertEqual(heard, ["checked 31 true"], "a plain click still checks it")
    }

    /// X77: a run is no view, but the web's run is an element, and a
    /// secondary click on it (a right-click, a Control-click) runs the
    /// nearest run's own `contextmenu` from the run's first line's top left,
    /// as Chrome measures an inline element, then its `contextPopover`'s
    /// menu, and ends there. A run with none leaves the paragraph's handler,
    /// and plain words #454's word and text menu. Show menu on a run's
    /// element opens its `contextPopover` too (X84 ran only its action).
    func testASecondaryClickOnARunRunsTheNearestRunsContextMenu() throws {
        try XCTSkipIf(ExactEnv.agentMode, "the agent opens no native menu")
        let p = presenter(inSession: true), window = try XCTUnwrap(windows.last)
        func run(_ id: Int, _ text: String, parent: Int, _ props: [String: String] = [:], _ handlers: [String] = []) -> [String: Any] {
            ["id": id, "parent": parent, "paint": !text.isEmpty, "props": props.merging(["text": text]) { $1 }, "handlers": handlers, "style": ["font_size": 18.0]]
        }
        p.apply(wireBatch([
            ["op": "create", "id": 8, "kind": "text", "handlers": ["contextmenu"], "style": ["font_size": 18.0]],
            ["op": "paragraph", "id": 8, "runs": [run(81, "Write ", parent: 8), run(82, "the list", parent: 8, ["href": "mailto:list@example.test"], ["contextmenu"]),
                                                 run(83, " or ", parent: 8), run(84, "the team", parent: 8, [:], ["press"]), run(85, "", parent: 8, [:], ["contextmenu"]),
                                                 run(86, " inner", parent: 85), run(87, " menu", parent: 8, ["contextPopover": "m"])]],
            ["op": "frame", "id": 8, "x": 20, "y": 80, "w": 460, "h": 28],
            ["op": "create", "id": 10, "kind": "text", "style": ["font_size": 18.0]],
            ["op": "paragraph", "id": 10, "runs": [run(101, "Alpha beta ", parent: 10), run(102, "gamma", parent: 10, [:], ["contextmenu"])]],
            ["op": "frame", "id": 10, "x": 20, "y": 130, "w": 300, "h": 28],
            ["op": "children", "id": 9, "ids": [1, 2, 8, 10]],
        ]))
        p.viewport.layoutSubtreeIfNeeded()
        window.orderFrontRegardless()
        var heard: [String] = [], sample: [String] = []
        p.onClipboard = { id, kind, line in
            guard kind == 10 else { return }
            heard.append("contextmenu \(id)")
            sample = line.components(separatedBy: ",")
        }
        p.onPress = { heard.append("press \($0)") }
        let center = NotificationCenter.default
        let observer = center.addObserver(forName: NSMenu.didBeginTrackingNotification, object: nil, queue: nil) { note in
            guard let menu = note.object as? NSMenu else { return }
            heard.append("menu " + (menu.items.first?.title ?? ""))
            RunLoop.current.perform(inModes: [.eventTracking, .default]) { menu.cancelTracking() }
        }
        defer { center.removeObserver(observer) }
        // The middle of run `id`'s first line, and that line, in its paragraph.
        func middle(_ id: UInt32) throws -> (paragraph: NodeView, point: NSPoint, line: NSRect) {
            let paragraph = try XCTUnwrap(p.textHost(id)), line = try XCTUnwrap(paragraph.inlineRects(try XCTUnwrap(p.inlineText(id))).first)
            return (paragraph, NSPoint(x: line.midX, y: line.midY), line)
        }
        func secondary(_ id: UInt32, control: Bool = false) throws -> [String] {
            let m = try middle(id), at = m.paragraph.convert(m.point, to: nil)
            heard = []
            if control { click(window, at: at, flags: .control) } else {
                for type in [NSEvent.EventType.rightMouseDown, .rightMouseUp] {
                    NSApp.sendEvent(try XCTUnwrap(NSEvent.mouseEvent(with: type, location: at, modifierFlags: [], timestamp: ProcessInfo.processInfo.systemUptime,
                                                                     windowNumber: window.windowNumber, context: nil, eventNumber: 1, clickCount: 1,
                                                                     pressure: type == .rightMouseDown ? 1 : 0)))
                }
            }
            // A `contextPopover`'s menu opens on the next turn; the rest is heard at once.
            let deadline = Date(timeIntervalSinceNow: 5)
            while heard.isEmpty && Date() < deadline { RunLoop.main.run(mode: .default, before: Date(timeIntervalSinceNow: 0.02)) }
            turn()
            return heard
        }
        let list = try middle(82)
        XCTAssertEqual(try secondary(82), ["contextmenu 82"], "the link run's own; not its paragraph's")
        XCTAssertEqual(Double(sample[0]) ?? -1, Double(list.point.x - list.line.minX), accuracy: 0.01, "from the run's first line")
        XCTAssertEqual(Double(sample[1]) ?? -1, Double(list.point.y - list.line.minY), accuracy: 0.01)
        XCTAssertEqual(sample[2], "2", "the secondary button held")
        XCTAssertEqual(try secondary(82, control: true), ["contextmenu 82"], "a Control-click is the same, and follows no link")
        XCTAssertEqual(sample[2], "1", "the primary button held")
        XCTAssertEqual(sample.last, "Control+")
        let team = try middle(84)
        XCTAssertEqual(try secondary(84), ["contextmenu 8"], "a run with none: the paragraph's")
        XCTAssertEqual(Double(sample[0]) ?? -1, Double(team.point.x - team.paragraph.contentBox().minX), accuracy: 0.01, "from the paragraph's box")
        XCTAssertEqual(try secondary(86), ["contextmenu 85"], "a run inside one that answers: the outer run's")
        XCTAssertEqual(try secondary(87), ["menu Pin"], "a run's contextPopover")
        XCTAssertEqual(try secondary(87, control: true), ["menu Pin"])
        XCTAssertEqual(try secondary(102, control: true), ["contextmenu 102"], "a run's own beside plain words")
        XCTAssertEqual(p.selection.selectedText(), "", "and selects nothing, as in Chrome")
        heard = []
        click(window, at: team.paragraph.convert(team.point, to: nil))
        XCTAssertEqual(heard, ["press 84"], "a plain click still presses")
        let alpha = try XCTUnwrap(p.views[10]), word = NSPoint(x: alpha.contentBox().minX + 6, y: alpha.contentBox().midY)
        heard = []
        click(window, at: alpha.convert(word, to: nil), flags: .control)
        let deadline = Date(timeIntervalSinceNow: 5)
        while heard.isEmpty && Date() < deadline { RunLoop.main.run(mode: .default, before: Date(timeIntervalSinceNow: 0.02)) }
        XCTAssertEqual(heard, ["menu Look Up “Alpha”"], "plain words: the word's text menu (#454)")
        // Show menu on the runs' elements: each one's own, its contextPopover
        // too; "the team" has none and its paragraph's answers.
        let elements = try XCTUnwrap(p.views[8]?.accessibilityChildren() as? [NSAccessibilityElement])
        XCTAssertEqual(elements.map { $0.accessibilityLabel() }, ["the list", "the team", " inner", " menu"])
        XCTAssertEqual(try elements.map { try offered($0) }, [["AXPress", "AXShowMenu"], ["AXPress", "AXShowMenu"], ["AXPress", "AXShowMenu"], ["AXPress", "AXShowMenu"]])
        heard = []
        XCTAssertTrue(elements[3].accessibilityPerformShowMenu())
        let shown = Date(timeIntervalSinceNow: 5)
        while heard.isEmpty && Date() < shown { RunLoop.main.run(mode: .default, before: Date(timeIntervalSinceNow: 0.02)) }
        XCTAssertEqual(heard, ["menu Pin"], "show menu on the run's element")
    }

    func testContextMenuDoesNotOpenForDisabledInertHiddenOrRemovedSources() throws {
        let changes: [(String, [String: Any])] = [
            ("disabled", ["op": "props", "id": 1, "set": ["disabled": "true"]]),
            ("inert", ["op": "props", "id": 1, "set": ["inert": "true"]]),
            ("hidden", ["op": "style", "id": 1, "style": ["display": "none"]]),
            ("removed", ["op": "destroy", "id": 1]),
        ]
        for (name, change) in changes {
            let p = presenter(), window = try XCTUnwrap(windows.last), source = try XCTUnwrap(p.views[1])
            XCTAssertTrue(window.makeFirstResponder(source))
            var heard = 0
            p.onClipboard = { _, _, _ in heard += 1 }
            p.apply(wireBatch([change]))
            XCTAssertFalse(p.routeKey(contextKey(window), focused: true), name)
            XCTAssertEqual(heard, 0, name)
        }
    }
    private func choose(_ item: NSMenuItem) throws {
        NSApp.sendAction(try XCTUnwrap(item.action), to: item.target, from: item)
    }
    private func shape(_ menu: NSMenu) -> [String] {
        menu.items.map { item in
            if item.isSeparatorItem { return "—" }
            var label = item.title + (item.state == .on ? " ✓" : "")
            if let sub = item.submenu { label += " ▸ [" + shape(sub).joined(separator: " | ") + "]" }
            return label
        }
    }

    /// LLP 1021 §5, "Submenus" (#141): a source (1) naming popover `row-menu`
    /// (2): Pinned (4, checked), Copy (7, opening `copy-menu`), an `hr` (6),
    /// Delete (5). `copy-menu` (8): Copy path (10), Copy link (11), an `hr`
    /// (12), Absolute paths (13, checked). `inside` authors `copy-menu` as a
    /// child of `row-menu` rather than beside it.
    private func submenuPresenter(inside: Bool = false, copyProps: [String: String] = [:]) -> Presenter {
        _ = NSApplication.shared
        let p = Presenter()
        let window = NSWindow(contentRect: NSRect(x: 0, y: 0, width: 500, height: 400), styleMask: [.titled], backing: .buffered, defer: false)
        window.isReleasedWhenClosed = false
        window.contentView = p.viewport
        windows.append(window)
        func row(_ id: Int, _ label: String, hides: String = "row-menu", _ props: [String: String] = [:], press: Bool = true) -> [[String: Any]] {
            let target = props["popovertarget"] == nil ? ["popovertarget": hides, "popovertargetaction": "hide"] : [:]
            return [["op": "create", "id": 100 + id, "kind": "text", "props": ["text": label]],
                    ["op": "frame", "id": 100 + id, "x": 0, "y": 0, "w": 180, "h": 20],
                    ["op": "create", "id": id, "kind": "button", "handlers": press ? ["press"] : [], "props": target.merging(props) { $1 }],
                    ["op": "frame", "id": id, "x": 0, "y": 0, "w": 200, "h": 30], ["op": "children", "id": id, "ids": [100 + id]]]
        }
        var ops: [[String: Any]] = [
            ["op": "create", "id": 1, "kind": "button", "handlers": ["contextmenu"], "props": ["contextPopover": "row-menu"]],
            ["op": "create", "id": 2, "kind": "view", "props": ["popover": "auto", "id": "row-menu", "accessibilityRole": "menu", "accessibilityLabel": "Row actions"]],
            ["op": "create", "id": 8, "kind": "view", "props": ["popover": "auto", "id": "copy-menu", "accessibilityRole": "menu", "accessibilityLabel": "Copy"]],
            ["op": "create", "id": 6, "kind": "view", "props": ["semanticTag": "hr"]],
            ["op": "create", "id": 12, "kind": "view", "props": ["semanticTag": "hr"]],
            ["op": "create", "id": 9, "kind": "view"],
        ]
        ops += row(4, "Pinned", ["accessibilityChecked": "true", "accessibilityRole": "menuitemcheckbox"])
        ops += row(7, "Copy", ["popovertarget": "copy-menu", "accessibilityHasPopup": "menu"].merging(copyProps) { $1 }, press: false)
        ops += row(5, "Delete")
        ops += row(10, "Copy path") + row(11, "Copy link") + row(13, "Absolute paths", ["accessibilityChecked": "true"])
        ops += [["op": "children", "id": 2, "ids": inside ? [4, 7, 8, 6, 5] : [4, 7, 6, 5]],
                ["op": "children", "id": 8, "ids": [10, 11, 12, 13]],
                ["op": "children", "id": 9, "ids": inside ? [1, 2] : [1, 2, 8]], ["op": "roots", "ids": [9]],
                ["op": "frame", "id": 9, "x": 0, "y": 0, "w": 500, "h": 400], ["op": "frame", "id": 1, "x": 20, "y": 20, "w": 120, "h": 30],
                ["op": "frame", "id": 2, "x": 0, "y": 0, "w": 220, "h": 200], ["op": "frame", "id": 8, "x": 0, "y": 0, "w": 220, "h": 200]]
        p.apply(wireBatch(ops))
        return p
    }

    /// A row whose `popovertarget` names another menu is a submenu item,
    /// beside its menu or authored inside it; check marks keep working in
    /// it; and choosing a nested item presses its row on the next turn, once
    /// for the whole menu.
    func testARowThatOpensAMenuIsASubmenu() throws {
        for inside in [false, true] {
            let p = submenuPresenter(inside: inside)
            let pop = try XCTUnwrap(p.views[2])
            XCTAssertTrue(p.menus.isMenuShaped(pop), "inside \(inside): a nested popover is no row")
            let menu = p.menus.menu(of: pop)
            XCTAssertEqual(shape(menu), ["Row actions", "Pinned ✓", "Copy ▸ [Copy path | Copy link | — | Absolute paths ✓]", "—", "Delete"], "inside \(inside)")
            let copy = try XCTUnwrap(menu.items.first { $0.title == "Copy" })
            XCTAssertEqual(copy.action, #selector(NSMenu.submenuAction(_:)), "AppKit's own: it opens the submenu and picks nothing")
            XCTAssertTrue(copy.isEnabled)
            let sub = try XCTUnwrap(copy.submenu)
            var pressed: [UInt32] = []
            p.onPress = { pressed.append($0) }
            try choose(sub.items[0])
            try choose(menu.items[1])
            XCTAssertEqual(pressed, [], "on the next turn")
            turn()
            XCTAssertEqual(pressed, [10], "inside \(inside): Copy path, and only it")
        }
    }

    /// A batch before the nested item's turn that points its opener at
    /// another popover, disables or hides the opener, or presents the menu
    /// again, cancels the choice; untouched, it presses.
    func testANestedChoiceDiesWithItsOpener() throws {
        let changes: [(String, (Presenter) -> Void)] = [
            ("untouched", { _ in }),
            ("opener retargeted", { p in p.apply(wireBatch([["op": "props", "id": 7, "set": ["popovertarget": "row-menu"]]])) }),
            ("opener disabled", { p in p.apply(wireBatch([["op": "props", "id": 7, "set": ["disabled": "true"]]])) }),
            ("opener hidden", { p in p.apply(wireBatch([["op": "style", "id": 7, "style": ["display": "none"]]])) }),
        ]
        for (name, change) in changes {
            let p = submenuPresenter()
            let menu = p.menus.menu(of: try XCTUnwrap(p.views[2]))
            var pressed: [UInt32] = []
            p.onPress = { pressed.append($0) }
            let copy = try XCTUnwrap(menu.items.first { $0.title == "Copy" }?.submenu)
            try choose(copy.items[1])
            change(p)
            turn()
            XCTAssertEqual(pressed, name == "untouched" ? [11] : [], name)
        }
    }

    /// A row naming a popover already on its path (a cycle), or one that
    /// hides its target, is an item, not a submenu; a disabled opener is a
    /// disabled submenu item.
    func testACycleOrAHideIsNoSubmenu() throws {
        let cycle = submenuPresenter(copyProps: ["popovertarget": "row-menu"])
        let items = cycle.menus.menu(of: try XCTUnwrap(cycle.views[2])).items
        XCTAssertNil(items.first { $0.title == "Copy" }?.submenu, "a row naming its own menu")
        let hides = submenuPresenter(copyProps: ["popovertargetaction": "hide"])
        XCTAssertNil(hides.menus.menu(of: try XCTUnwrap(hides.views[2])).items.first { $0.title == "Copy" }?.submenu)
        let disabled = submenuPresenter(copyProps: ["disabled": "true"])
        let copy = try XCTUnwrap(disabled.menus.menu(of: try XCTUnwrap(disabled.views[2])).items.first { $0.title == "Copy" })
        XCTAssertNotNil(copy.submenu)
        XCTAssertFalse(copy.isEnabled)
    }

    /// An item's title is its row's accessible name: an `aria-hidden` child
    /// (the web's own `›` on a submenu row) is left out, as the menu draws
    /// its own arrow.
    func testAnAriaHiddenChildIsNotInTheTitle() throws {
        let p = submenuPresenter()
        p.apply(wireBatch([["op": "create", "id": 50, "kind": "text", "props": ["text": "›", "accessibilityElementsHidden": "true"]],
                           ["op": "frame", "id": 50, "x": 0, "y": 0, "w": 10, "h": 20], ["op": "children", "id": 7, "ids": [107, 50]]]))
        let menu = p.menus.menu(of: try XCTUnwrap(p.views[2]))
        XCTAssertNotNil(menu.items.first { $0.title == "Copy" }?.submenu)
    }
}
#endif
