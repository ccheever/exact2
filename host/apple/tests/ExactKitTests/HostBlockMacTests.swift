// @ref LLP 1115.002 D8.3 — the Mac's proof of the one thing the kernel
// learned for a split view and a sheet (D8.2): one session whose page has
// three of its subtrees in three native views. Two are the panes of an
// `NSSplitView`, each reporting its size as the divider moves; the third is
// a sheet (`beginSheet`) given a maximum and sized from what layout returns.
// A fixture, in no app: the split (stage 6b) and the modal route's sheet
// (6c) are built on what this holds.
#if os(macOS)
import AppKit
import XCTest
@testable import ExactKit

final class HostBlockMacTests: XCTestCase {
    private var windows: [NSWindow] = []
    private var session: ExactSession?
    override func tearDown() {
        session?.destroy(); session = nil
        windows.forEach { $0.close() }; windows = []
        super.tearDown()
    }

    private func plan(_ source: String) throws -> Data {
        let dir = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
        try FileManager.default.createDirectory(at: dir, withIntermediateDirectories: true)
        defer { try? FileManager.default.removeItem(at: dir) }
        try source.write(to: dir.appendingPathComponent("app.contract"), atomically: true, encoding: .utf8)
        let compiler = Process()
        compiler.executableURL = URL(fileURLWithPath: try XCTUnwrap(ProcessInfo.processInfo.environment["EXACT_CONTRACT"]))
        compiler.arguments = ["build", dir.appendingPathComponent("app.contract").path, "-o", dir.appendingPathComponent("app.plan").path]
        compiler.standardError = FileHandle.nullDevice // its colour advice
        try compiler.run(); compiler.waitUntilExit()
        XCTAssertEqual(compiler.terminationStatus, 0)
        return try Data(contentsOf: dir.appendingPathComponent("app.plan"))
    }

    /// The page `host/apple/tests/it/host_block.rs` lays out too, coloured:
    /// a list (a field, a scroller), a detail (a percentage child, an
    /// absolutely positioned one) and a sheet (a form that wraps, rows that
    /// an action adds), all routes of one stack, with the page around them.
    private let page = """
    component Proof
      state rows = [0, 1]
      action more
        rows = rows.concat([rows.length])
      view
        column testId="page" width="100%" height="100%"
          text "Behind the panes" testId="title"
          box testId="stack" position="relative" flex=1 min-height=0
            column testId="list" position="absolute" inset=0 background-color="#e8f0fe"
              input testId="field" value="draft"
              scroll testId="scroller" flex=1 min-height=0
                box testId="tall" height=1200 background-color="#c6dafc"
            column testId="detail" position="absolute" inset=0 background-color="#fef7e0"
              box testId="half" width="50%" height="25%" background-color="#f9ab00"
              box testId="badge" position="absolute" right=10 bottom=10 width=40 height=40 background-color="#c5221f"
            column testId="sheet" position="absolute" inset=0 padding=16 background-color="#e6f4ea"
              row testId="form" flex-wrap="wrap" gap=8
                box width=120 height=30 background-color="#34a853"
                box width=160 height=30 background-color="#188038"
                box width=120 height=30 background-color="#34a853"
              box press=more testId="more" height=44
                text "More"
              for i in rows key=i
                row height=44
                  text `Row ${i}`
          text "Footer" testId="footer"

    """

    /// A view's own drawing, saved where `EXACT_PROOF_SHOTS` says.
    private func shot(_ view: NSView, _ name: String) {
        guard let dir = ProcessInfo.processInfo.environment["EXACT_PROOF_SHOTS"],
              let rep = view.bitmapImageRepForCachingDisplay(in: view.bounds) else { return }
        view.cacheDisplay(in: view.bounds, to: rep)
        try? rep.representation(using: .png, properties: [:])?.write(to: URL(fileURLWithPath: dir).appendingPathComponent(name))
    }

    func testABlockDoesNotBubblePressesToItsAuthoredHome() throws {
        _ = NSApplication.shared
        let p = Presenter()
        let window = NSWindow(contentRect: NSRect(x: 0, y: 0, width: 600, height: 300),
                              styleMask: [.titled], backing: .buffered, defer: false)
        window.isReleasedWhenClosed = false
        windows.append(window)
        let content = NSView(frame: window.contentLayoutRect)
        window.contentView = content
        p.viewport.frame = NSRect(x: 0, y: 0, width: 300, height: 300)
        content.addSubview(p.viewport)
        let pane = BlockView(frame: NSRect(x: 300, y: 0, width: 300, height: 300))
        content.addSubview(pane)
        p.apply(wireBatch([
            ["op": "create", "id": 1, "kind": "view", "handlers": ["press"]],
            ["op": "create", "id": 2, "kind": "view"],
            ["op": "create", "id": 3, "kind": "view"],
            ["op": "create", "id": 4, "kind": "text", "props": ["text": "Selectable detail"]],
            ["op": "children", "id": 1, "ids": [2]],
            ["op": "children", "id": 2, "ids": [3, 4]],
            ["op": "roots", "ids": [1]],
            ["op": "frame", "id": 1, "x": 0, "y": 0, "w": 300, "h": 300],
            ["op": "frame", "id": 2, "x": 0, "y": 0, "w": 300, "h": 300],
            ["op": "frame", "id": 3, "x": 10, "y": 10, "w": 100, "h": 40],
            ["op": "frame", "id": 4, "x": 10, "y": 60, "w": 200, "h": 30],
        ]))
        let home = try XCTUnwrap(p.views[1]), detail = try XCTUnwrap(p.views[2])
        let plain = try XCTUnwrap(p.views[3]), paragraph = try XCTUnwrap(p.views[4])
        p.blocks.show(detail, in: pane, block: .size(pane.bounds.size))
        XCTAssertTrue(p.eventParent(of: detail) === home, "non-press events keep their authored path")
        XCTAssertTrue(detail.superview === pane)
        var pressed: [UInt32] = []
        p.onPress = { pressed.append($0) }
        for node in [plain, paragraph] {
            let point = node.convert(NSPoint(x: 5, y: 5), to: nil)
            func event(_ type: NSEvent.EventType) throws -> NSEvent {
                try XCTUnwrap(NSEvent.mouseEvent(with: type, location: point, modifierFlags: [], timestamp: 0,
                    windowNumber: window.windowNumber, context: nil, eventNumber: 0, clickCount: 1, pressure: 1))
            }
            node.mouseDown(with: try event(.leftMouseDown))
            XCTAssertNil(p.mouseChain.bubbledPress, "a block is no presentation crossing")
            node.mouseUp(with: try event(.leftMouseUp))
            XCTAssertEqual(pressed, [], "a detail click reaches no handler in the list")
        }
        XCTAssertFalse(paragraph.hasPressableAncestor)
        XCTAssertTrue(paragraph.selectsText)
    }

    func testTwoPanesAndASheetEachHoldASubtreeOfOnePage() throws {
        _ = NSApplication.shared
        let session = ExactApp.shared.makeSession(label: "host-block")
        self.session = session
        XCTAssertNil(session.boot(plan: try plan(page), size: CGSize(width: 800, height: 600)).error)
        // One window: the session's page above, the split's two panes below.
        let window = NSWindow(contentRect: NSRect(x: 0, y: 0, width: 800, height: 1000), styleMask: [.titled, .resizable], backing: .buffered, defer: false)
        window.isReleasedWhenClosed = false
        windows.append(window)
        let content = NSView(frame: NSRect(x: 0, y: 0, width: 800, height: 1000))
        window.contentView = content
        let view = ExactView(session: session)
        view.frame = NSRect(x: 0, y: 400, width: 800, height: 600)
        content.addSubview(view)
        let split = NSSplitView(frame: NSRect(x: 0, y: 0, width: 800, height: 400))
        split.isVertical = true
        split.dividerStyle = .thin
        let left = BlockView(frame: .zero), right = BlockView(frame: .zero)
        split.addArrangedSubview(left); split.addArrangedSubview(right)
        content.addSubview(split)
        split.setPosition(280, ofDividerAt: 0)
        content.layoutSubtreeIfNeeded()
        XCTAssertEqual(left.bounds.size, CGSize(width: 280, height: 400))
        XCTAssertEqual(right.bounds.width, 800 - 280 - split.dividerThickness)

        let p = session.presenter
        func node(_ id: String) throws -> NodeView { try XCTUnwrap(p.views.values.first { $0.props["testId"] == id }, id) }
        let list = try node("list"), detail = try node("detail"), sheet = try node("sheet"), stack = try node("stack")
        let half = try node("half"), badge = try node("badge"), scroller = try node("scroller"), form = try node("form")
        let behind = try ["page", "title", "stack", "footer"].map(node)
        let pageFrames = behind.map(\.frame)
        XCTAssertEqual(stack.frame.width, 800)
        XCTAssertTrue(list.superview === stack.container && sheet.superview === stack.container)
        XCTAssertEqual(list.frame.size, stack.frame.size, "a route fills its stack until a pane holds it")
        // What a person left in the list before the window split.
        let field = try XCTUnwrap(try node("field").field)
        field.stringValue = "typed before the split"
        let scroll = try XCTUnwrap(scroller.scroll)

        // Each pane takes one subtree and reports its size, now and whenever
        // the divider gives it another.
        for (route, pane) in [(list, left), (detail, right)] {
            p.blocks.show(route, in: pane, block: .size(pane.bounds.size))
            pane.sizeChanged = { [weak p, weak route] size in
                if let p, let route { p.blocks.resize(route, .size(size)) }
            }
        }
        XCTAssertTrue(list.superview === left && detail.superview === right)
        XCTAssertEqual(list.frame, NSRect(origin: .zero, size: left.bounds.size))
        XCTAssertEqual(detail.frame, NSRect(origin: .zero, size: right.bounds.size))
        XCTAssertEqual(p.blocks.used[list.id], left.bounds.size)
        // A percentage and an absolutely positioned child are the pane's.
        XCTAssertEqual(half.frame, NSRect(x: 0, y: 0, width: right.bounds.width / 2, height: 100))
        XCTAssertEqual(badge.frame, NSRect(x: right.bounds.width - 50, y: 350, width: 40, height: 40))
        XCTAssertEqual(scroller.frame.width, 280)
        XCTAssertEqual(behind.map(\.frame), pageFrames, "the page around them is laid out as it was")
        XCTAssertTrue(sheet.superview === stack.container, "a route no container took stays the page's")
        scroll.contentView.scroll(to: NSPoint(x: 0, y: 300))
        scroll.reflectScrolledClipView(scroll.contentView)
        shot(content, "host-block-1-panes-280.png")

        // The divider moves: both panes are laid out at their new sizes, and
        // what the person left is where it was, in the same views.
        split.setPosition(200, ofDividerAt: 0)
        content.layoutSubtreeIfNeeded()
        XCTAssertEqual(left.bounds.width, 200)
        XCTAssertEqual(list.frame, NSRect(origin: .zero, size: left.bounds.size))
        XCTAssertEqual(detail.frame, NSRect(origin: .zero, size: right.bounds.size))
        XCTAssertEqual(half.frame.width, right.bounds.width / 2)
        XCTAssertEqual(badge.frame.origin.x, right.bounds.width - 50)
        XCTAssertEqual(scroller.frame.width, 200)
        XCTAssertTrue(try node("field").field === field)
        XCTAssertEqual(field.stringValue, "typed before the split")
        XCTAssertTrue(try node("scroller").scroll === scroll)
        XCTAssertEqual(scroll.contentView.bounds.origin.y, 300, "the scroll position is the node's, and stays")
        XCTAssertEqual(behind.map(\.frame), pageFrames)
        shot(content, "host-block-2-panes-200.png")

        // A sheet takes the third subtree: a maximum in, the content's size
        // out, which is the sheet window's. The sheet never reports its own
        // size back, so nothing feeds on itself.
        let sheetWindow = NSWindow(contentRect: NSRect(x: 0, y: 0, width: 100, height: 100), styleMask: [.titled], backing: .buffered, defer: false)
        sheetWindow.isReleasedWhenClosed = false
        windows.append(sheetWindow)
        let sheetView = BlockView(frame: NSRect(x: 0, y: 0, width: 100, height: 100))
        sheetWindow.contentView = sheetView
        var sizes: [CGSize] = []
        p.blocks.onUsed = { [weak sheet, weak sheetWindow] id, size in
            guard id == sheet?.id else { return }
            sizes.append(size)
            sheetWindow?.setContentSize(size)
        }
        let panes = (list.frame, detail.frame, half.frame)
        p.blocks.show(sheet, in: sheetView, block: .atMost(CGSize(width: 480, height: 400)))
        // The form's three boxes on a line inside the padding; under it the
        // button and two rows.
        let fitted = CGSize(width: 16 + 120 + 8 + 160 + 8 + 120 + 16, height: 16 + 30 + 44 + 2 * 44 + 16)
        XCTAssertEqual(sizes, [fitted])
        XCTAssertEqual(sheetView.frame.size, fitted)
        XCTAssertEqual(sheet.frame, NSRect(origin: .zero, size: fitted))
        XCTAssertEqual(form.frame, NSRect(x: 16, y: 16, width: fitted.width - 32, height: 30))
        window.beginSheet(sheetWindow)
        XCTAssertTrue(window.attachedSheet === sheetWindow)
        XCTAssertTrue(sheet.window === sheetWindow && list.window === window)
        shot(sheetView, "host-block-3-sheet.png")
        // Its content grows by a row: the sheet's window follows.
        let more = try node("more")
        p.press(more.id)
        XCTAssertEqual(sizes.last, CGSize(width: fitted.width, height: fitted.height + 44))
        XCTAssertEqual(sheetView.frame.size, sizes.last)
        // And past the maximum it stops there, the route with it.
        for _ in 0..<5 { p.press(more.id) }
        XCTAssertEqual(sizes.last, CGSize(width: fitted.width, height: 400))
        XCTAssertEqual(sheetView.frame.height, 400)
        XCTAssertEqual(sheet.frame.height, 400)
        shot(sheetView, "host-block-4-sheet-at-its-maximum.png")
        // The page behind the sheet, and the panes, are laid out as before.
        XCTAssertEqual(behind.map(\.frame), pageFrames)
        XCTAssertEqual(panes.0, list.frame)
        XCTAssertEqual(panes.1, detail.frame)
        XCTAssertEqual(panes.2, half.frame)
        XCTAssertEqual(field.stringValue, "typed before the split")

        // Each container gives its subtree back: the routes are the stack's
        // again, laid out in it, in their order.
        window.endSheet(sheetWindow)
        for route in [sheet, detail, list] { p.blocks.release(route) }
        XCTAssertNil(p.blocks.used[sheet.id])
        let routes = stack.container.subviews.compactMap { $0 as? NodeView }
        XCTAssertEqual(routes.map(\.id), [list.id, detail.id, sheet.id])
        for route in routes { XCTAssertEqual(route.frame, NSRect(origin: .zero, size: stack.frame.size)) }
        XCTAssertEqual(behind.map(\.frame), pageFrames)
        XCTAssertEqual(field.stringValue, "typed before the split")
    }
}
#endif
