#if os(macOS)
import AppKit
import XCTest
@testable import ExactKit

/// LLP 1069.011.000 on AppKit: a native button is a window toolbar's item,
/// its prominent style a prominent item (D2), and a tab drawn as a segment
/// from its face (D4), its shortcut kept (D7).
final class NativeContextsMacTests: XCTestCase {
    func testModuleRootsUseTheNamedStoresRuntimeRootsAcrossSessions() throws {
        let base = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
        defer { try? FileManager.default.removeItem(at: base) }
        let first = ExactApp.shared.makeSession(label: "module-store-first")
        let second = ExactApp.shared.makeSession(label: "module-store-second")
        defer { first.destroy(); second.destroy() }
        let roots = ["data": base.appendingPathComponent("data").path,
                     "cache": base.appendingPathComponent("cache").path,
                     "tmp": base.appendingPathComponent("temporary").path]
        first.appFileRoots = roots; second.appFileRoots = roots
        let a = NativeViews.roots(session: first, agent: true, namedStore: "module-store")
        let b = NativeViews.roots(session: second, agent: true, namedStore: "module-store")
        for (written, read) in [(a.data, b.data), (a.cache, b.cache), (a.temporary, b.temporary)] {
            try FileManager.default.createDirectory(atPath: written, withIntermediateDirectories: true)
            try "kept".write(toFile: written + "/probe", atomically: true, encoding: .utf8)
            XCTAssertEqual(try String(contentsOfFile: read + "/probe", encoding: .utf8), "kept")
        }
        let unnamed = NativeViews.roots(session: second, agent: true, namedStore: nil)
        XCTAssertNotEqual(unnamed.data, roots["data"])
        XCTAssertTrue(unnamed.data.contains("exact-agent-"), "an unnamed drive stays away from real app files")
    }
    func testANativeButtonIsAToolbarItemItsProminentStyleProminent() throws {
        _ = NSApplication.shared
        let p = Presenter()
        let w = NSWindow(contentRect: NSRect(x: 0, y: 0, width: 640, height: 480),
                         styleMask: [.titled, .closable], backing: .buffered, defer: false)
        w.isReleasedWhenClosed = false; w.contentView = p.root
        defer { p.toolbar.detach(); w.close() }
        let bar = NodeView(id: 1, kind: "view", presenter: p)
        bar.props = ["accessibilityRole": "toolbar", "toolbarPlacement": "window"]
        let send = NodeView(id: 2, kind: "control", presenter: p)
        send.props = ["type": "button", "accessibilityRole": "button", "buttonStyle": "filled"]
        send.handlers = ["press"]
        let edit = NodeView(id: 3, kind: "button", presenter: p)
        edit.props = ["accessibilityLabel": "Edit"]
        edit.handlers = ["press"]
        p.root.addSubview(bar); bar.addSubview(send); bar.addSubview(edit)
        for n in [bar, send, edit] { p.views[n.id] = n }
        p.buttonFace = { id in
            var f = ButtonFace()
            if id == 2 { f.title = "Send"; f.symbol = "paperplane"; f.style = "filled" }
            return f
        }
        XCTAssertTrue(p.toolbar.attach(to: w))
        let items = try XCTUnwrap(w.toolbar?.items)
        let sendItem = try XCTUnwrap(items.first { $0.itemIdentifier.rawValue == "exact.command.2" })
        let editItem = try XCTUnwrap(items.first { $0.itemIdentifier.rawValue == "exact.command.3" })
        XCTAssertEqual(sendItem.label, "Send", "its title, from its face")
        XCTAssertNotNil(sendItem.image, "its symbol")
        if #available(macOS 26.0, *), LinkedDesign.liquidGlass {
            XCTAssertEqual(sendItem.style, .prominent)
            XCTAssertEqual(editItem.style, .plain, "a custom button stays plain")
        }
        var pressed: [UInt32] = []
        p.onPress = { pressed.append($0) }
        NSApp.sendAction(try XCTUnwrap(sendItem.action), to: sendItem.target, from: sendItem)
        XCTAssertEqual(pressed, [2])
    }

    func testNativeSymbolTabsAreImageSegments() throws {
        _ = NSApplication.shared
        let p = Presenter()
        p.buttonFace = { id in
            var f = ButtonFace()
            f.symbol = ["list.bullet", "square.grid.2x2", "map"][Int(id) - 2]
            f.label = ["List", "Grid", "Map"][Int(id) - 2]
            return f
        }
        let w = NSWindow(contentRect: NSRect(x: 0, y: 0, width: 400, height: 200), styleMask: [.titled], backing: .buffered, defer: false)
        w.isReleasedWhenClosed = false
        w.contentView = p.viewport
        defer { w.close() }
        var ops: [[String: Any]] = [["op": "create", "id": 1, "kind": "view", "props": ["accessibilityRole": "tablist"], "handlers": [], "style": ["text_color": [0, 0, 0, 255]]],
                                    ["op": "frame", "id": 1, "x": 0.0, "y": 0.0, "w": 300.0, "h": 30.0]]
        for (i, id) in [2, 3, 4].enumerated() {
            ops += [["op": "create", "id": id, "kind": "control", "handlers": ["press"],
                     "props": ["type": "button", "accessibilityRole": "tab", "accessibilityLabel": ["List", "Grid", "Map"][i],
                               "accessibilityKeyShortcuts": "Meta+\(i + 1)"],
                     "style": ["appearance": "auto", "text_color": [0, 0, 0, 255]]],
                    ["op": "frame", "id": id, "x": Double(i) * 100, "y": 0.0, "w": 100.0, "h": 30.0]]
        }
        p.apply(wireBatch(ops + [["op": "children", "id": 1, "ids": [2, 3, 4]], ["op": "roots", "ids": [1]]]))
        let segments = try XCTUnwrap(p.views[1]?.subviews.compactMap { $0 as? NSSegmentedControl }.first)
        XCTAssertEqual(segments.segmentCount, 3)
        XCTAssertNotNil(segments.image(forSegment: 0))
        XCTAssertEqual(segments.image(forSegment: 1)?.accessibilityDescription, "Grid")
        // A tab the segment shows keeps its shortcut, though its view is hidden.
        XCTAssertEqual(p.views[3]?.isHidden, true)
        var pressed: [UInt32] = []
        p.onPress = { pressed.append($0) }
        let key = NSEvent.keyEvent(with: .keyDown, location: .zero, modifierFlags: .command, timestamp: 0,
                                   windowNumber: w.windowNumber, context: nil, characters: "2",
                                   charactersIgnoringModifiers: "2", isARepeat: false, keyCode: 0)!
        XCTAssertTrue(p.shortcuts.perform(key))
        XCTAssertEqual(pressed, [3])
    }
}
#endif
