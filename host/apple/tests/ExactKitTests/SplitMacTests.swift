// @ref LLP 1115.002 D4 — a list and its detail are a split view on the Mac:
// which stack splits (the runner's rule), what each pane holds (the page as
// the list's column, the detail's routes in a pane of their own through the
// kernel's containing block), the widths and minimums AppKit gives three
// columns and two, the selected row, Back, the two headers in one bar (§5
// Q10), a sheet over the split, and what the driver sees.
#if os(macOS)
import AppKit
import XCTest
@testable import ExactKit

final class SplitMacTests: XCTestCase {
    private var window: NSWindow?
    private var session: ExactSession?
    override func tearDown() {
        window?.close(); window = nil
        session?.destroy(); session = nil
    }

    /// A library of books under a root tab bar: its rows link to a book,
    /// a book pushes its notes, and + opens a sheet. `tabs: false` is the
    /// same stack with no tab bar.
    private func app(tabs: Bool) -> String {
        let routes = """
                      if e.name == "library"
                        column navigationKey=`${e.id}` position="absolute" inset=0 display="flex" flex-direction="column" testId="route-library"
                          header display="flex" align-items="center"
                            text "Library" role="heading" aria-level=1 flex=1
                            button press=openAdd aria-label="Add Book" aria-keyshortcuts="Meta+N" testId="add-book"
                              image "symbol:add"
                            input type="search" value=query input=search placeholder="Search" testId="search"
                          list testId="books" flex=1 min-height=0
                            for id in ["a", "b", "c"] key=id
                              link href=path("book", id) testId=`row-${id}`
                                text `Book ${id}`
                      if e.name == "book"
                        column navigationKey=`${e.id}` position="absolute" inset=0 display="flex" flex-direction="column" testId=`route-book-${e.params.id}`
                          header display="flex" align-items="center"
                            text `About ${e.params.id}` role="heading" aria-level=2 flex=1 testId=`title-${e.params.id}`
                            button press=openNotes(e.params.id) aria-label="Notes" testId=`notes-${e.params.id}`
                              image "symbol:pencil"
                            button press=count aria-label="Count" aria-keyshortcuts="Meta+N" testId=`count-${e.params.id}`
                              image "symbol:delete"
                          box testId=`half-${e.params.id}` width="50%" height=40 background-color="#f9ab00"
                          input value=query input=search testId=`field-${e.params.id}`
                      if e.name == "notes"
                        column navigationKey=`${e.id}` position="absolute" inset=0 testId="route-notes"
                          text "Notes on the book"
                      if e.name == "add"
                        column navigationKey=`${e.id}` navigationPresentation="modal" position="absolute" inset=0 testId="route-add"
                          header display="flex" align-items="center"
                            button id="back" press=back testId="add-cancel"
                              text "Cancel"
                            text "New Book" role="heading"
                          text "A form"
                      if e.name == "stats"
                        column navigationKey=`${e.id}` position="absolute" inset=0 testId="route-stats"
                          text "Stats"
        """
        let head = """
        routes nav
        \(tabs ? "  tab library \"/\"\n    book \"/book/:id\"\n    notes \"/notes/:id\"\n    add \"/add\"\n  tab stats \"/stats\"" : "  library \"/\"\n    book \"/book/:id\"\n    notes \"/notes/:id\"\n    add \"/add\"")

        component App
          state query = ""
          state counted = 0
          action pick(name: string)
            nav = select(nav, name)
          action follow(url: string)
            nav = go(nav, url)
          action back
            nav = back(nav)
          action openAdd
            nav = push(nav, "/add")
          action openNotes(id: string)
            nav = push(nav, path("notes", id))
          action search(v: string)
            query = v
          action count
            counted = counted + 1
          view
            main navigationKey=`${top(nav).id}` navigationBack="back" navigate=follow testId="app"
              width="100%" height="100%" display="flex" flex-direction="column"

        """
        if !tabs {
            let dedented = routes.split(separator: "\n", omittingEmptySubsequences: false).map { String($0.dropFirst(6)) }.joined(separator: "\n")
            return head + "      for e in stack(nav) key=e.id\n" + dedented + "\n"
        }
        return head + """
              column flex=1 min-height=0 position="relative" testId="panels"
                for t in nav.tabs key=t.name
                  column role="tabpanel" id=`panel-${t.name}` position="absolute" inset=0
                    for e in t.stack key=e.id

        """ + routes + """

              row role="tablist" testId="tabs" aria-label="Places"
                button role="tab" aria-controls="panel-library" aria-selected=(nav.tab == "library") press=pick("library") testId="tab-library" flex=1
                  image "symbol:calendar"
                  text "Library"
                button role="tab" aria-controls="panel-stats" aria-selected=(nav.tab == "stats") press=pick("stats") testId="tab-stats" flex=1
                  image "symbol:chart"
                  text "Stats"

        """
    }

    private func boot(tabs: Bool, size: CGSize, source: String? = nil) throws -> ExactSession {
        _ = NSApplication.shared
        let dir = URL(fileURLWithPath: FileManager.default.currentDirectoryPath)
            .appendingPathComponent("../../target/split-test-" + UUID().uuidString).standardizedFileURL
        try FileManager.default.createDirectory(at: dir, withIntermediateDirectories: true)
        defer { try? FileManager.default.removeItem(at: dir) }
        let contract = dir.appendingPathComponent("app.contract"), plan = dir.appendingPathComponent("app.plan")
        try (source ?? app(tabs: tabs)).write(to: contract, atomically: true, encoding: .utf8)
        let compiler = Process(), said = Pipe()
        compiler.executableURL = URL(fileURLWithPath: try XCTUnwrap(ProcessInfo.processInfo.environment["EXACT_CONTRACT"]))
        compiler.arguments = ["build", contract.path, "-o", plan.path]
        compiler.standardError = said
        try compiler.run(); compiler.waitUntilExit()
        XCTAssertEqual(compiler.terminationStatus, 0, String(decoding: said.fileHandleForReading.readDataToEndOfFile(), as: UTF8.self))
        let session = ExactApp.shared.makeSession(label: "split-" + UUID().uuidString)
        self.session = session
        XCTAssertNil(session.boot(plan: try Data(contentsOf: plan), size: size).error)
        return session
    }

    private func node(_ testId: String) throws -> NodeView {
        try XCTUnwrap(session?.presenter.views.values.first { $0.props["testId"] == testId }, testId)
    }
    private func has(_ testId: String) -> Bool { session?.presenter.views.values.contains { $0.props["testId"] == testId } == true }
    private func press(_ testId: String) throws {
        let session = try XCTUnwrap(self.session)
        session.presenter.press(try node(testId).id)
    }

    /// The window's owner, as ExactMac composes it: a full-size-content
    /// window whose content is the split view, and the window's toolbar.
    private func compose(_ view: ExactView, width: CGFloat, sidebar: Bool, minimum: CGFloat = 320, autosave: String? = nil,
                         window kind: NSWindow.Type = NSWindow.self) -> ExactSidebarController {
        let window = kind.init(contentRect: NSRect(x: 0, y: 0, width: width, height: 500),
                               styleMask: [.titled, .resizable, .fullSizeContentView], backing: .buffered, defer: false)
        window.isReleasedWhenClosed = false
        self.window = window
        window.contentView = NSView(frame: NSRect(x: 0, y: 0, width: width, height: 500))
        let split = ExactSidebarController(view: view, autosaveName: autosave, minimumPageWidth: minimum, sidebar: sidebar)
        split.install(in: window)
        window.setContentSize(NSSize(width: width, height: 500))
        XCTAssertTrue(view.attachWindowToolbar(to: window))
        window.layoutIfNeeded()
        return split
    }
    private func width(_ window: NSWindow, _ width: CGFloat) {
        window.setContentSize(NSSize(width: width, height: 500))
        window.layoutIfNeeded()
    }
    private func items(_ window: NSWindow) -> [String] { window.toolbar?.items.map(\.itemIdentifier.rawValue) ?? [] }

    func testTheRunnerReadsFramesAndHitsInTheDetailPanesSpace() throws {
        let source = app(tabs: false)
            .replacingOccurrences(of: "  state query = \"\"", with: "  state observed = \"unread\"\n  state hit = \"unread\"\n  state query = \"\"")
            .replacingOccurrences(of: "  action count\n", with: "  action readGeometry\n    let f = frame(\"geometry-card\")\n    observed = `${f.x},${f.y},${f.width}`\n    hit = match elementFromPoint(321 + f.width / 2, f.y + 5) { case some(id) => id, case none => \"none\" }\n  action count\n")
            .replacingOccurrences(of: "box testId=`half-${e.params.id}`", with: "box id=\"geometry-card\" testId=`half-${e.params.id}`")
            .replacingOccurrences(of: "testId=`field-${e.params.id}`", with: "testId=`field-${e.params.id}`\n            button testId=\"read-geometry\" press=readGeometry\n              text \"Read\"\n            text `${observed} ${hit}` testId=\"geometry-result\"")
        let session = try boot(tabs: false, size: CGSize(width: 1000, height: 500), source: source)
        let columns = compose(ExactView(session: session), width: 1000, sidebar: false)
        let window = try XCTUnwrap(self.window)
        try press("row-a")
        func check(dx: CGFloat = 0) throws {
            window.layoutIfNeeded()
            session.presenter.settlePump()
            try press("read-geometry")
            let card = try node("half-a"), clip = session.presenter.viewport.contentView
            XCTAssertTrue(card.isDescendant(of: columns.detailPane))
            let shown = clip.convert(card.bounds, from: card).offsetBy(dx: -clip.bounds.minX, dy: -clip.bounds.minY)
            let words = try node("geometry-result").accessibleText.split(separator: " ")
            let frame = try XCTUnwrap(words.first).split(separator: ",").compactMap { Double($0) }
            XCTAssertEqual(frame.count, 3)
            guard frame.count == 3 else { return }
            XCTAssertGreaterThan(shown.minX, 200)
            XCTAssertEqual(frame[0], shown.minX + dx, accuracy: 0.5)
            XCTAssertEqual(frame[1], shown.minY, accuracy: 0.5)
            XCTAssertEqual(frame[2], shown.width, accuracy: 0.5)
            XCTAssertEqual(words.last, "geometry-card")
        }
        try check()
        width(window, 1200)
        try check()
        window.orderFrontRegardless()
        columns.detailPane.wantsLayer = true
        window.displayIfNeeded()
        CATransaction.flush()
        let layer = try XCTUnwrap(columns.detailPane.layer)
        let restingFrame = columns.detailPane.frame
        let end = layer.position.x
        let motion = CABasicAnimation(keyPath: "position.x")
        motion.fromValue = end - 80; motion.toValue = end
        motion.duration = 1; motion.speed = 0; motion.timeOffset = 0.5
        motion.fillMode = .both; motion.isRemovedOnCompletion = false
        layer.add(motion, forKey: "split-placement-test")
        CATransaction.flush()
        let deadline = Date(timeIntervalSinceNow: 30)
        while layer.presentation() == nil, Date() < deadline { RunLoop.main.run(mode: .default, before: Date(timeIntervalSinceNow: 0.05)) }
        XCTAssertEqual(try XCTUnwrap(layer.presentation()).position.x, end - 40, accuracy: 0.1)
        XCTAssertEqual(columns.detailPane.frame, restingFrame, "presentation movement does not resize or move the model pane")
        try check(dx: -40)
        layer.removeAnimation(forKey: "split-placement-test")
        try check()
    }

    func testTheDriverCapturesAndReadsBothPanes() throws {
        let session = try boot(tabs: false, size: CGSize(width: 900, height: 500))
        let columns = compose(ExactView(session: session), width: 900, sidebar: false)
        try press("row-a")
        let agent = Agent(session: session)
        let picture = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString + ".png")
        defer { try? FileManager.default.removeItem(at: picture) }
        let capture = agent.screenshot(["path": picture.path])
        XCTAssertNil(capture["error"], "\(capture)")
        XCTAssertEqual(capture["w"] as? Double, Double(columns.view.bounds.width))
        let ax = agent.accessibilityElementsTree([:])["ax"] as? [String: Any]
        let exposed = (ax?["elements"] as? [[String: Any]] ?? []).compactMap { $0["testId"] as? String }
        XCTAssertTrue(exposed.contains("row-a"), "the list is still exposed")
        XCTAssertTrue(exposed.contains("field-a"), "the detail is exposed too: \(exposed)")
        let box = try node("half-a")
        let frame = agent.box(box)
        let point = NSPoint(x: frame.minX + 10, y: frame.minY + 10)
        let pixels = agent.sample(["points": [[point.x, point.y]]])["values"] as? [[Double]]
        let pixel = try XCTUnwrap(pixels?.first)
        XCTAssertGreaterThan(pixel[0], 0.8)
        XCTAssertLessThan(pixel[2], 0.1)
        for name in ["row-a", "half-a"] {
            let reply = agent.tap(["id": Int(try node(name).id), "hover": true])
            XCTAssertNil(reply["error"], "\(name): \(reply)")
        }
        let split = session.presenter.navigation.split
        split.listOwnsWindow = false
        XCTAssertEqual(window?.title, "About a")
        XCTAssertTrue(try node("title-a").isHiddenOrHasHiddenAncestor)
        split.listOwnsWindow = true
        XCTAssertEqual(window?.title, "Library")
        XCTAssertFalse(try node("title-a").isHiddenOrHasHiddenAncestor)
    }

    func testAnUnsaidWindowGroundLetsTheSplitSurfaceShowThrough() throws {
        let session = try boot(tabs: false, size: CGSize(width: 900, height: 500))
        let p = session.presenter
        _ = compose(ExactView(session: session), width: 900, sidebar: false)
        try press("row-a")
        let role: [String: Any] = ["sys": "windowBackgroundColor", "c": [[255, 255, 255, 255], [30, 30, 30, 255]]]
        let route = try node("route-book-a")
        p.apply(wireBatch([["op": "style", "id": route.id, "style": ["background_color": role]]]))
        XCTAssertNil(route.layer?.backgroundColor)
        p.apply(wireBatch([["op": "style", "id": route.id, "style": ["background_color": role, "written": ["background_color"]]]]))
        XCTAssertNotNil(route.layer?.backgroundColor, "an author's explicit role still paints")
    }

    func testPublicLayoutSamplesAndScreenshotClicksShareThePageOrigin() throws {
        let source = app(tabs: true).replacingOccurrences(of: "box testId=`half-${e.params.id}`", with: "box press=count testId=`half-${e.params.id}`")
        let session = try boot(tabs: true, size: CGSize(width: 1100, height: 500), source: source)
        let columns = compose(ExactView(session: session), width: 1100, sidebar: true)
        try press("row-a")
        let window = try XCTUnwrap(window), p = session.presenter, agent = Agent(session: session)
        window.setFrameOrigin(NSPoint(x: -20000, y: -20000))
        window.orderFrontRegardless()
        XCTAssertNotNil(window.toolbar)
        XCTAssertGreaterThan(columns.sidebarWidth, 100)
        let card = try node("half-a")
        let layout = try XCTUnwrap(agent.layout()["nodes"] as? [[String: Any]])
        let frame = try XCTUnwrap(layout.first { $0["id"] as? Int == Int(card.id) })
        let x = try XCTUnwrap(frame["x"] as? Double), y = try XCTUnwrap(frame["y"] as? Double)
        let sampled = agent.sample(["points": [[x + 5, y + 10]]])
        let pixel = try XCTUnwrap((sampled["values"] as? [[Double]])?.first)
        XCTAssertGreaterThan(pixel[0], 0.8)
        XCTAssertLessThan(pixel[2], 0.1, "a public layout point samples the orange card")

        let path = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString + ".png")
        defer { try? FileManager.default.removeItem(at: path) }
        let capture = agent.screenshot(["path": path.path])
        XCTAssertNil(capture["error"])
        let image = try XCTUnwrap(NSBitmapImageRep(data: Data(contentsOf: path)))
        let scale = CGFloat(image.pixelsWide) / CGFloat(try XCTUnwrap(capture["w"] as? Double))
        var right = 0, top = image.pixelsHigh
        for py in 0..<image.pixelsHigh { for px in 0..<image.pixelsWide {
            guard let c = image.colorAt(x: px, y: py)?.usingColorSpace(.sRGB),
                  c.redComponent > 0.9, c.greenComponent > 0.5, c.greenComponent < 0.8, c.blueComponent < 0.05 else { continue }
            right = max(right, px); top = min(top, py)
        } }
        XCTAssertGreaterThan(right, 0)
        XCTAssertLessThan(top, image.pixelsHigh)
        let reply = agent.tap(["id": Int(card.id), "hover": true, "x": CGFloat(right) / scale - 5, "y": CGFloat(top) / scale + 10])
        XCTAssertNil(reply["error"], "\(reply)")
        let point = try XCTUnwrap(p.agentPointer)
        let hit = window.contentView?.hitTest(point)
        XCTAssertTrue(hit === card || hit?.isDescendant(of: card) == true, "a screenshot point sends the pointer to the card")
    }

    func testAnUnsplitStackClosesADetailPaneRestoredByAppKit() throws {
        let session = try boot(tabs: true, size: CGSize(width: 1100, height: 500))
        try press("tab-stats")
        let view = ExactView(session: session)
        let columns = compose(view, width: 1100, sidebar: true)
        let window = try XCTUnwrap(self.window), detail = try XCTUnwrap(columns.splitViewItems.last)
        XCTAssertFalse(session.presenter.navigation.split.active)
        XCTAssertFalse(columns.splits)
        // AppKit can restore a divider after the controller's initial collapse.
        detail.isCollapsed = false
        window.layoutIfNeeded()
        session.presenter.navigation.split.sync()
        window.layoutIfNeeded()
        XCTAssertTrue(detail.isCollapsed)
        XCTAssertFalse(columns.splits)
        XCTAssertEqual(view.frame.width, window.contentView!.bounds.width - columns.sidebarWidth - columns.splitView.dividerThickness, accuracy: 1)
    }

    private func listeningDetail() throws -> (Presenter, NSWindow, [NodeView]) {
        let handlers = "focusin=heardFocus focusout=heardFocus keycapture=heardKey key=heardKey keyup=heardKey"
        let source = app(tabs: true)
            .replacingOccurrences(of: "  action count\n", with: "  action heardFocus(event: FocusEvent)\n  action heardKey(key: string)\n  action count\n")
            .replacingOccurrences(of: "testId=\"app\"", with: "testId=\"app\" \(handlers)")
            .replacingOccurrences(of: "column role=\"tabpanel\"", with: "column role=\"tabpanel\" testId=`panel-${t.name}` \(handlers)")
            .replacingOccurrences(of: "testId=`route-book-${e.params.id}`", with: "testId=`route-book-${e.params.id}` \(handlers)")
            .replacingOccurrences(of: "testId=`field-${e.params.id}`", with: "testId=`field-${e.params.id}` id=`field-${e.params.id}` \(handlers)")
        let session = try boot(tabs: true, size: CGSize(width: 1100, height: 500), source: source)
        let columns = compose(ExactView(session: session), width: 1100, sidebar: true)
        let window = try XCTUnwrap(self.window)
        window.setFrameOrigin(NSPoint(x: -20000, y: -20000))
        window.orderFrontRegardless()
        try press("row-b")
        let path = try ["field-b", "route-book-b", "panel-library", "app"].map(node)
        XCTAssertTrue(path[1].superview === columns.detailPane)
        XCTAssertFalse(path[1].isDescendant(of: path[2]), "AppKit reparents the route, not the authored event path")
        XCTAssertTrue(window.makeFirstResponder(nil))
        session.presenter.focusEvents.reset()
        return (session.presenter, window, path)
    }

    func testExitingAnAncestorRetiresItsFocusedPaneRoute() throws {
        let (p, window, path) = try listeningDetail()
        let fieldNode = path[0], route = path[1], panel = path[2]
        let field = try XCTUnwrap(fieldNode.field)
        XCTAssertTrue(window.makeFirstResponder(field))
        let editor = try XCTUnwrap(field.currentEditor())
        XCTAssertFalse(route.isDescendant(of: panel))
        let pane = try XCTUnwrap(route.superview), frame = route.frame
        p.apply(wireBatch([["op": "exit", "id": Int(panel.id)]]))
        XCTAssertNil(p.views[route.id])
        XCTAssertNil(p.views[fieldNode.id])
        XCTAssertFalse(window.firstResponder === editor)
        XCTAssertNil(field.currentEditor())
        let members = try XCTUnwrap(p.leaving[panel.id]).members
        XCTAssertTrue(members.contains { $0 === route })
        XCTAssertTrue(members.contains { $0 === fieldNode })
        XCTAssertEqual(Set(members.map(\.id)).count, members.count)
        XCTAssertTrue(route.superview === pane, "the exit keeps the pane content mounted")
        XCTAssertTrue(route.window === window)
        XCTAssertEqual(route.frame, frame)
        XCTAssertFalse(route.isHiddenOrHasHiddenAncestor)
        XCTAssertTrue(route.routeInert)
        XCTAssertTrue(route.isAccessibilityHidden())
        XCTAssertTrue(p.navigation.split.holds(route))
        p.apply(wireBatch([]))
        XCTAssertTrue(route.superview === pane, "a later batch must not release it either")
        p.apply(wireBatch([["op": "destroy", "id": Int(panel.id)]]))
        XCTAssertNil(route.superview)
        XCTAssertFalse(p.navigation.split.holds(route))
    }

    func testDetailFocusBubblesThroughItsAuthoredAncestorsAndStops() throws {
        let (p, window, path) = try listeningDetail()
        let field = try XCTUnwrap(path[0].field)
        var heard: [String] = [], stop: UInt32?, outgoing: XCTestExpectation?
        let names = Dictionary(uniqueKeysWithValues: path.map { ($0.id, $0.props["testId"]!) })
        p.onFocusEvent = { id, kind, target, related in
            heard.append("\(names[id] ?? "unexpected"):\(kind)")
            XCTAssertEqual(target, "field-b")
            XCTAssertNil(related)
            if id == stop { p.propagationStopped = true }
            if id == path[0].id, kind == "focusout" { outgoing?.fulfill() }
        }
        for stopping in [false, true] {
            stop = stopping ? path[2].id : nil
            let expected = path.prefix(stopping ? 3 : 4).map { names[$0.id]! }
            heard.removeAll()
            XCTAssertTrue(window.makeFirstResponder(field))
            XCTAssertNotNil(field.currentEditor())
            XCTAssertEqual(heard, expected.map { "\($0):focusin" })
            heard.removeAll()
            outgoing = expectation(description: "the detail field blurs")
            XCTAssertTrue(window.makeFirstResponder(nil))
            wait(for: [try XCTUnwrap(outgoing)], timeout: 5)
            XCTAssertEqual(heard, expected.map { "\($0):focusout" })
            outgoing = nil
        }
    }

    func testDetailKeysCaptureAndBubbleThroughItsAuthoredAncestorsAndStop() throws {
        let (p, window, path) = try listeningDetail()
        XCTAssertTrue(window.makeFirstResponder(try XCTUnwrap(path[0].field)))
        var heard: [String] = [], stop: String?
        let names = Dictionary(uniqueKeysWithValues: path.map { ($0.id, $0.props["testId"]!) })
        p.onKey = { id, press in
            let event = "\(names[id] ?? "unexpected"):\(press.capture ? "capture" : press.up ? "up" : "down")"
            heard.append(event)
            XCTAssertEqual(press.chord, "k")
            XCTAssertEqual(press.code, "KeyK")
            if event == stop { p.propagationStopped = true; p.defaultPrevented = true }
        }
        func key(_ type: NSEvent.EventType) throws -> NSEvent {
            try XCTUnwrap(NSEvent.keyEvent(with: type, location: .zero, modifierFlags: [], timestamp: 0, windowNumber: window.windowNumber,
                                         context: nil, characters: "k", charactersIgnoringModifiers: "k", isARepeat: false, keyCode: 40))
        }
        let capture = path.reversed().map { "\(names[$0.id]!):capture" }
        let bubble = path.map { "\(names[$0.id]!):down" }
        XCTAssertFalse(p.keyDown(try key(.keyDown), in: window))
        XCTAssertEqual(heard, capture + bubble)
        heard.removeAll()
        p.keyUp(try key(.keyUp), in: window)
        XCTAssertEqual(heard, path.map { "\(names[$0.id]!):up" })
        for (at, expected) in [("panel-library:capture", Array(capture.prefix(2))),
                               ("panel-library:down", capture + Array(bubble.prefix(3)))] {
            stop = at; heard.removeAll()
            XCTAssertTrue(p.keyDown(try key(.keyDown), in: window))
            XCTAssertEqual(heard, expected)
        }
        stop = "panel-library:up"; heard.removeAll()
        p.keyUp(try key(.keyUp), in: window)
        XCTAssertEqual(heard, path.prefix(3).map { "\(names[$0.id]!):up" })
    }

    func testAListAndItsDetailAreTwoPanesBesideTheSidebar() throws {
        let session = try boot(tabs: true, size: CGSize(width: 1100, height: 500))
        let p = session.presenter, split = p.navigation.split
        let view = ExactView(session: session)
        // Wanted by the runner's rule; nothing is claimed by mounting, and the stack pushes.
        XCTAssertTrue(view.wantsWindowSplit)
        XCTAssertFalse(split.active)
        let columns = compose(view, width: 1100, sidebar: true)
        let window = try XCTUnwrap(self.window)
        XCTAssertTrue(split.active)
        XCTAssertTrue(columns.splits)
        let divider = columns.splitView.dividerThickness
        let library = try node("route-library"), books = try node("books")
        // Three columns: AppKit's sidebar, the list at the rule's width, and the detail with the rest.
        XCTAssertEqual(columns.sidebarWidth, 144, accuracy: 1)
        XCTAssertEqual(columns.listColumnWidth, ExactSidebarController.listWidth, accuracy: 1)
        XCTAssertEqual(columns.detailWidth, 1100 - columns.sidebarWidth - columns.listColumnWidth - divider, accuracy: 1)
        XCTAssertEqual(library.frame.width, columns.listColumnWidth, accuracy: 1, "the page is the list's column")
        XCTAssertNotNil(p.tables.feeds[books.id], "whose list is a table")
        // Nothing selected: the pane says so, in AppKit's words.
        XCTAssertEqual(columns.detail.saysNothing, SplitDetail.noSelection)
        XCTAssertEqual(SplitDetail.noSelection, "No Selection")
        XCTAssertEqual(window.title, "Library")
        XCTAssertFalse(p.navigation.canGoBack)

        // A row is pressed: its route is in the detail's pane, laid out at the pane's size, beside a list that stays live.
        let feed = try XCTUnwrap(p.tables.feeds[books.id])
        feed.table.selectRowIndexes([1], byExtendingSelection: false)
        XCTAssertTrue(feed.pressRow(1))
        let book = try node("route-book-b"), pane = columns.detailPane
        XCTAssertTrue(book.superview === pane)
        XCTAssertEqual(book.frame, NSRect(origin: .zero, size: pane.bounds.size))
        XCTAssertEqual(pane.bounds.width, columns.detailWidth, accuracy: 1)
        XCTAssertEqual(try node("half-b").frame.width, pane.bounds.width / 2, accuracy: 0.5, "a percentage is the pane's")
        XCTAssertEqual(library.frame.width, columns.listColumnWidth, accuracy: 1)
        XCTAssertFalse(library.isHidden)
        XCTAssertFalse(library.inert, "the list is live beside its detail")
        XCTAssertFalse(book.isHidden)
        XCTAssertNil(columns.detail.saysNothing)
        XCTAssertEqual(feed.table.selectedRow, 1, "the row whose detail shows stays selected")
        XCTAssertEqual(split.shown?.row, try node("row-b").id)
        // The list owns the window (§5 Q10): its heading is the title, and the detail's stays the first line of its pane.
        XCTAssertEqual(window.title, "Library")
        let title = try node("title-b")
        XCTAssertFalse(title.isHiddenOrHasHiddenAncestor)
        XCTAssertTrue(try node("notes-b").isHidden, "its buttons are the bar's")
        let add = try node("add-book"), notes = try node("notes-b"), count = try node("count-b"), search = try node("search")
        XCTAssertEqual(items(window), ["NSToolbarToggleSidebarItem", "NSToolbarSidebarTrackingSeparatorItemIdentifier", "exact.command.\(add.id)",
                                       "exact.split.separator", "exact.command.\(notes.id)", "exact.command.\(count.id)", "NSToolbarFlexibleSpaceItem", "exact.search.\(search.id)"])
        XCTAssertEqual(p.covers.delivered[notes.id], .whole)
        XCTAssertNil(p.covers.delivered[title.id])
        // What ARIA says of a button is what its item's element says, in a bar with the divider's separator
        // among its items: the bar's elements are its items in order, a separator being none.
        func element(_ label: String) throws -> NSAccessibilityProtocol {
            let bar = try XCTUnwrap(window.accessibilityChildren()?.compactMap { $0 as? NSAccessibilityProtocol }.first { $0.accessibilityRole() == .toolbar })
            return try XCTUnwrap((bar.accessibilityChildren() ?? []).compactMap { $0 as? NSAccessibilityProtocol }.first { $0.accessibilityLabel() == label }, label)
        }
        p.apply(wireBatch([["op": "props", "id": Int(count.id), "set": ["accessibilityDisabled": "true"]]]))
        window.contentView?.superview?.layoutSubtreeIfNeeded()
        XCTAssertFalse(try element("Count").isAccessibilityEnabled(), "the detail's button, after the separator")
        XCTAssertTrue(try element("Notes").isAccessibilityEnabled())
        XCTAssertTrue(try element("Add Book").isAccessibilityEnabled())
        p.apply(wireBatch([["op": "props", "id": Int(count.id), "set": ["accessibilityDisabled": "false"]]]))
        XCTAssertTrue(try element("Count").isAccessibilityEnabled())
        // The detail has no Back: nothing was pushed from it.
        XCTAssertFalse(p.navigation.canGoBack)

        // Another row, which this app pushes over the first: it is that row's detail, and still nothing leads Back.
        feed.table.selectRowIndexes([2], byExtendingSelection: false)
        XCTAssertTrue(feed.pressRow(2))
        let other = try node("route-book-c")
        XCTAssertTrue(other.superview === pane)
        XCTAssertTrue(book.isHidden)
        XCTAssertEqual(split.shown?.base, 2)
        XCTAssertFalse(p.navigation.canGoBack)
        XCTAssertEqual(feed.table.selectedRow, 2)

        // A push inside the detail: the window's Back leads from it, and the row stays selected.
        p.press(try node("notes-c").id)
        let pushed = try node("route-notes")
        XCTAssertTrue(pushed.superview === pane)
        XCTAssertEqual(pushed.frame.size, pane.bounds.size)
        XCTAssertTrue(p.navigation.canGoBack)
        XCTAssertEqual(feed.table.selectedRow, 2)
        XCTAssertEqual(window.title, "Library")
        // Back is at the bar's leading end, after the sidebar's own items, and came with the push.
        XCTAssertEqual(Array(items(window).prefix(3)), ["NSToolbarToggleSidebarItem", "NSToolbarSidebarTrackingSeparatorItemIdentifier", "exact.navigation.back"])
        XCTAssertTrue(p.navigation.goBack())
        XCTAssertFalse(has("route-notes"))
        XCTAssertFalse(other.isHidden)
        XCTAssertFalse(p.navigation.canGoBack)
        XCTAssertFalse(items(window).contains("exact.navigation.back"), "and goes with it")

        // The divider moves: the detail is laid out at its pane's new size.
        columns.splitView.setPosition(columns.sidebarWidth + 400, ofDividerAt: columns.listDivider)
        window.layoutIfNeeded()
        XCTAssertEqual(columns.listColumnWidth, 400, accuracy: 1)
        XCTAssertEqual(other.frame.size, pane.bounds.size)
        XCTAssertEqual(try node("half-c").frame.width, pane.bounds.width / 2, accuracy: 0.5)

        // Another tab, whose stack pushes: the pane goes, the page has the window beside the sidebar, and the book is not shown.
        try press("tab-stats")
        XCTAssertFalse(columns.splits)
        XCTAssertEqual(view.frame.width, 1100 - columns.sidebarWidth, accuracy: 1)
        XCTAssertTrue(other.isHidden)
        XCTAssertTrue(other.inert)
        XCTAssertFalse(items(window).contains("exact.split.separator"))
        // And back: the split as the person left it, with the same route in the same views.
        try press("tab-library")
        XCTAssertTrue(columns.splits)
        XCTAssertEqual(columns.listColumnWidth, 400, accuracy: 1)
        XCTAssertTrue(other.superview === pane)
        XCTAssertFalse(other.isHidden)
        XCTAssertEqual(other.frame.size, pane.bounds.size)

        // The person takes the selection off the row: the pane shows nothing, as the stack goes back to its list.
        feed.table.deselectAll(nil)
        XCTAssertFalse(has("route-book-c"))
        XCTAssertFalse(has("route-book-b"))
        XCTAssertEqual(columns.detail.saysNothing, "No Selection")
        XCTAssertEqual(feed.table.selectedRow, -1)
    }

    func testTheColumnsNarrowAsAppKitsDoAndTheWindowStopsAtTheirMinimums() throws {
        let session = try boot(tabs: true, size: CGSize(width: 1300, height: 500))
        let view = ExactView(session: session)
        let columns = compose(view, width: 1300, sidebar: true)
        let window = try XCTUnwrap(self.window)
        let divider = columns.splitView.dividerThickness
        func widths() -> [CGFloat] { [columns.sidebarWidth, columns.listColumnWidth, columns.detailWidth].map { $0.rounded() } }
        XCTAssertEqual(widths(), [144, 320, 1300 - 144 - 320 - divider])
        // The detail gives first, down to the page's minimum.
        width(window, 900)
        XCTAssertEqual(widths(), [144, 320, 900 - 144 - 320 - divider])
        width(window, 144 + 320 + 320 + divider)
        XCTAssertEqual(widths(), [144, 320, 320])
        // Then the list, down to AppKit's 200 for a content list.
        width(window, 144 + 260 + 320 + divider)
        XCTAssertEqual(widths(), [144, 260, 320])
        // And then the window stops: the sidebar's 144, the list's 200, the detail's minimum and the dividers.
        width(window, 300)
        let least = 144 + ExactSidebarController.listMinimum + 320 + divider
        XCTAssertEqual(window.contentView?.frame.width ?? 0, least, accuracy: 1)
        XCTAssertEqual(widths(), [144, 200, 320])
        // Wider again: the list has its width back, as AppKit's own content list does (measured), and the detail the rest.
        width(window, 1000)
        XCTAssertEqual(widths(), [144, 320, 1000 - 144 - 320 - divider])
    }

    func testWithNoTabBarTheWindowIsTheListAndItsDetail() throws {
        let session = try boot(tabs: false, size: CGSize(width: 900, height: 500))
        let p = session.presenter
        let view = ExactView(session: session)
        XCTAssertFalse(view.wantsWindowSidebar)
        XCTAssertTrue(view.wantsWindowSplit)
        let columns = compose(view, width: 900, sidebar: false)
        let window = try XCTUnwrap(self.window)
        XCTAssertTrue(columns.splits)
        XCTAssertFalse(columns.hasSidebar)
        XCTAssertEqual(columns.listColumnWidth, 320, accuracy: 1)
        XCTAssertEqual(columns.detailWidth, 900 - 320 - columns.splitView.dividerThickness, accuracy: 1)
        try press("row-a")
        let book = try node("route-book-a"), add = try node("add-book"), notes = try node("notes-a"), count = try node("count-a"), search = try node("search")
        XCTAssertTrue(book.superview === columns.detailPane)
        // No sidebar's button; the list's items, the separator on the one divider, the detail's.
        XCTAssertEqual(items(window), ["exact.command.\(add.id)", "exact.split.separator",
                                       "exact.command.\(notes.id)", "exact.command.\(count.id)", "NSToolbarFlexibleSpaceItem", "exact.search.\(search.id)"])
        XCTAssertEqual(window.title, "Library")
        // The window stops at the list's 200, the divider and the detail's minimum.
        width(window, 200)
        XCTAssertEqual(window.contentView?.frame.width ?? 0, 200 + 320 + columns.splitView.dividerThickness, accuracy: 1)
        XCTAssertFalse(p.navigation.canGoBack)
    }

    /// The text pump paints the detail's pane as it paints the page: a route
    /// there is out of the root, and its kept heading, outside every scroll
    /// view, is in view. Left out, a rasterized heading kept the pixels of
    /// the appearance it was first painted in (black in Dark Mode).
    func testTheDetailPanesTextIsThePumpsToPaint() throws {
        let session = try boot(tabs: false, size: CGSize(width: 900, height: 500))
        let columns = compose(ExactView(session: session), width: 900, sidebar: false)
        try press("row-a")
        let title = try node("title-a")
        XCTAssertTrue(title.isDescendant(of: columns.detailPane))
        XCTAssertNil(title.enclosingScrollView)
        let shown = session.presenter.selection.shownParagraphs
        XCTAssertTrue(shown.contains { $0 === title })
        XCTAssertTrue(TextViewportIndex(shown).candidates().contains { $0 === title })
        XCTAssertEqual(shown.count, Set(shown.map(ObjectIdentifier.init)).count, "each paragraph once")
    }

    func testARetainedListObeysTheCurrentSplitRule() throws {
        let source = """
        routes nav
          library "/"
            book "/book/:id"
        component App
          state mode = "shown"
          state ids = ["a"]
          state splitWidth = ""
          action follow(url: string)
            nav = go(nav, url)
          action back
            nav = back(nav)
          action change(value: string)
            mode = value
          action width(value: string)
            splitWidth = value
          action empty
            ids = []
          action refill
            ids = ["a"]
          view
            main navigationKey=`${top(nav).id}` navigationBack="back" navigate=follow
              width="100%" height="100%"
              for e in stack(nav) key=e.id
                column navigationKey=`${e.id}` navigationSplit=splitWidth position="absolute" inset=0
                  if e.name == "library"
                    button press=change("hidden") testId="hide"
                      text "Hide"
                    button press=change("shown") testId="show"
                      text "Show"
                    button press=change("removed") testId="remove"
                      text "Remove"
                    button press=change("sideways") testId="sideways"
                      text "Sideways"
                    button press=change("commands") testId="commands"
                      text "Commands"
                    button press=width("300") testId="force"
                      text "Force"
                    button press=width("") testId="unforce"
                      text "Unforce"
                    button press=empty testId="empty"
                      text "Empty"
                    button press=refill testId="refill"
                      text "Refill"
                    if mode != "removed"
                      list testId="changing-list" appearance="none" width=100 height=100 display=(mode == "hidden" ? "none" : "flex")
                        flex-direction=(mode == "sideways" ? "row" : "column")
                        if mode != "commands"
                          for id in ids key=id
                            link href=path("book", id) display="block" width=50 height=50 testId=`row-${id}`
                              text id
                        if mode == "commands"
                          button press=refill
                            text "Local command"
                        if ids.length == 0
                          text "No Results"
                  if e.name == "book"
                    text "Detail"
        """
        let session = try boot(tabs: false, size: CGSize(width: 900, height: 500), source: source)
        let split = session.presenter.navigation.split
        let columns = compose(ExactView(session: session), width: 900, sidebar: false)
        let list = try node("changing-list")
        XCTAssertTrue(split.active)
        try press("empty")
        XCTAssertTrue(split.active, "an existing list keeps its empty detail")
        XCTAssertEqual(split.shown?.table, list.id)
        try press("refill")
        XCTAssertTrue(split.active)
        // One fixed-size row in a fixed-size list: changing direction moves
        // no frame, so the host must still read the rule on the empty batch.
        try press("sideways")
        XCTAssertFalse(split.active, "a carousel is not a vertical list")
        try press("show")
        XCTAssertTrue(split.active)
        try press("commands")
        XCTAssertFalse(split.active, "non-navigation actions do not retain a split")
        try press("show")
        XCTAssertTrue(split.active)
        try press("hide")
        XCTAssertFalse(split.active, "an authored hidden list no longer asks for a split")
        XCTAssertFalse(columns.splits)
        try press("show")
        XCTAssertTrue(split.active)
        try press("remove")
        XCTAssertFalse(has("changing-list"))
        XCTAssertFalse(split.active, "a removed list must not keep its cached split")
        XCTAssertFalse(columns.splits)
        try press("force")
        XCTAssertTrue(split.active, "a current explicit width opts in without a list")
        XCTAssertNil(split.shown?.table)
        try press("unforce")
        XCTAssertFalse(split.active, "clearing that width does not retain it")
        try press("show")
        XCTAssertTrue(split.active)
        XCTAssertEqual(split.shown?.table, try node("changing-list").id, "the current list owns the split")
    }

    func testAnUnsupportedDeclaredToolbarKeepsBothSplitHeadersPainted() throws {
        let session = try boot(tabs: false, size: CGSize(width: 1100, height: 500))
        let p = session.presenter
        _ = compose(ExactView(session: session), width: 1100, sidebar: false)
        try press("row-a")
        let library = try node("route-library"), notes = try node("notes-a")
        let children = library.container.subviews.compactMap { ($0 as? NodeView).map { Int($0.id) } }
        p.apply(wireBatch([
            ["op": "create", "id": 10000, "kind": "view", "props": ["accessibilityRole": "toolbar", "toolbarPlacement": "window"]],
            ["op": "create", "id": 10001, "kind": "image", "props": ["symbolName": "book"]],
            ["op": "children", "id": 10000, "ids": [10001]],
            ["op": "children", "id": Int(library.id), "ids": children + [10000]]
        ]))
        XCTAssertNil(p.toolbar.header)
        XCTAssertNil(p.toolbar.beside)
        XCTAssertFalse(try node("add-book").isHiddenOrHasHiddenAncestor)
        XCTAssertFalse(notes.isHiddenOrHasHiddenAncestor)
        XCTAssertNil(p.covers.delivered[notes.id])
        XCTAssertTrue(p.toolbar.items.isEmpty)
    }

    func testASheetOverTheSplitLeavesBothPanesUnderItAndTheListsKeyWinsATie() throws {
        let session = try boot(tabs: true, size: CGSize(width: 1100, height: 500))
        let p = session.presenter
        let view = ExactView(session: session)
        let columns = compose(view, width: 1100, sidebar: true)
        let window = try XCTUnwrap(self.window)
        window.setFrameOrigin(NSPoint(x: -20000, y: -20000))
        window.orderFrontRegardless()
        try press("row-b")
        let book = try node("route-book-b"), library = try node("route-library")
        XCTAssertTrue(book.superview === columns.detailPane)
        // ⌘N is declared in both panes: the list's is the window's, whichever pane has the focus.
        let field = try XCTUnwrap(try node("field-b").field)
        XCTAssertTrue(window.makeFirstResponder(field))
        XCTAssertTrue(view.ownsShortcutFocus(), "the detail's pane holds this session's focus")
        // The page's Tab order goes through the pane, where the route was written: its field is a stop of the page's loop.
        p.keyViewLoopStale = true
        p.flushKeyViewLoop()
        var stop: NSView? = p.viewport.nextKeyView, reached = false
        for _ in 0..<64 {
            guard let at = stop, !reached else { break }
            reached = at === field
            stop = at.nextKeyView
        }
        XCTAssertTrue(reached, "Tab from the page reaches the detail's field")
        let key = try XCTUnwrap(NSEvent.keyEvent(with: .keyDown, location: .zero, modifierFlags: .command, timestamp: 0, windowNumber: window.windowNumber,
                                                 context: nil, characters: "n", charactersIgnoringModifiers: "n", isARepeat: false, keyCode: 45))
        XCTAssertTrue(p.shortcuts.perform(key))
        // The list's + opened its sheet: a modal route over the split, on the window, with both panes shown under it.
        let form = try node("route-add")
        let opened = expectation(description: "the sheet arrives")
        DispatchQueue.main.asyncAfter(deadline: .now() + 0.6) { opened.fulfill() }
        wait(for: [opened], timeout: 5)
        XCTAssertNotNil(window.attachedSheet)
        XCTAssertTrue(form.window === window.attachedSheet)
        XCTAssertFalse(library.isHidden)
        XCTAssertFalse(book.isHidden)
        XCTAssertTrue(library.inert)
        XCTAssertTrue(book.inert)
        XCTAssertTrue(book.superview === columns.detailPane)
        XCTAssertEqual(window.title, "Library")
        // Its Cancel: the stack goes back, and the split is as it was.
        p.press(try node("add-cancel").id, fromNativeMenu: true)
        let closed = expectation(description: "the sheet leaves")
        DispatchQueue.main.asyncAfter(deadline: .now() + 0.6) { closed.fulfill() }
        wait(for: [closed], timeout: 5)
        XCTAssertFalse(has("route-add"))
        XCTAssertFalse(library.inert)
        XCTAssertFalse(book.inert)
        XCTAssertTrue(book.superview === columns.detailPane)
    }

    func testUnderTheDriversOwnChromeTheStackPushesAndTheDriverSeesTheSplit() throws {
        let session = try boot(tabs: true, size: CGSize(width: 1100, height: 500))
        let p = session.presenter, split = p.navigation.split
        let view = ExactView(session: session)
        let columns = compose(view, width: 1100, sidebar: true)
        try press("row-a")
        let book = try node("route-book-a"), library = try node("route-library")
        var summary = split.summary
        XCTAssertEqual(summary["installed"] as? Bool, true)
        XCTAssertEqual(summary["detail"] as? Int, Int(book.id))
        XCTAssertEqual(summary["row"] as? Int, Int(try node("row-a").id))
        XCTAssertEqual(summary["listWidth"] as? Double, 320)
        // The driver's `back` from a split's detail: as a person leaves it, the selection off its row.
        let went = try XCTUnwrap(p.navigation.agentBack())
        XCTAssertEqual(went.way, "route")
        XCTAssertEqual(went.refused, "")
        XCTAssertFalse(has("route-book-a"))
        XCTAssertNil(p.navigation.agentBack(), "and from the list there is nowhere to go back to")

        // The driver's own chrome: the stack pushes in the page, as authored.
        try press("row-a")
        let again = try node("route-book-a")
        XCTAssertTrue(again.superview === columns.detailPane)
        split.painted = true
        XCTAssertFalse(split.active)
        XCTAssertFalse(columns.splits)
        XCTAssertFalse(again.superview === columns.detailPane)
        XCTAssertTrue(library.isHidden, "a pushed route covers its list")
        XCTAssertTrue(p.navigation.canGoBack)
        summary = split.summary
        XCTAssertEqual(summary["installed"] as? Bool, false)
        split.painted = false
        XCTAssertTrue(columns.splits)
        XCTAssertTrue(again.superview === columns.detailPane)
        XCTAssertFalse(library.isHidden)
    }

    /// A popover declared in a route the split shows in its detail pane opens,
    /// painted and as AppKit's: the pane is a block's native container, and
    /// whether the popover is in the page walks out of it to the route's
    /// authored home (theme2 12:32; it had never opened there, the pane being
    /// under no page root).
    func testAPopoverInTheDetailPaneOpens() throws {
        for painted in [true, false] {
            let source = app(tabs: false).replacingOccurrences(
                of: "testId=`field-${e.params.id}`",
                with: "testId=`field-${e.params.id}`\n            button testId=\"pop-open\" popovertarget=\"bookpop\"\n              text \"More\"\n            column id=\"bookpop\" testId=\"bookpop\" popover=\"auto\" width=100 height=40"
            ).replacingOccurrences(
                of: "      for e in stack(nav) key=e.id\n",
                with: "      button testId=\"list-pop-open\" popovertarget=\"listpop\"\n        text \"Filter\"\n      column id=\"listpop\" testId=\"listpop\" popover=\"manual\" width=100 height=40\n      for e in stack(nav) key=e.id\n")
            let session = try boot(tabs: false, size: CGSize(width: 1000, height: 500), source: source)
            _ = compose(ExactView(session: session), width: 1000, sidebar: false)
            let window = try XCTUnwrap(self.window)
            window.setFrameOrigin(NSPoint(x: 120, y: 120))
            window.orderFrontRegardless()
            let p = session.presenter
            p.menus.painted = painted
            func settle() { for _ in 0..<20 { p.settlePump(); session.drainFill(); RunLoop.main.run(until: Date().addingTimeInterval(0.02)) } }
            try press("row-a")
            settle()
            window.layoutIfNeeded()
            let pop = try node("bookpop"), opener = try node("pop-open")
            XCTAssertTrue(p.navigation.split.holds(try node("route-book-a")), "the book is in the detail pane")
            XCTAssertFalse(opener.isDescendant(of: p.root), "its invoker is under no page root")
            try press("pop-open")
            let end = Date().addingTimeInterval(3)
            while !p.menus.isOpen(pop), Date() < end { settle() }
            XCTAssertTrue(p.menus.isOpen(pop), painted ? "painted" : "AppKit's popover")
            if !painted { XCTAssertEqual(p.menus.windowed.count, 1) }
            if painted {
                // In the pane's own layer, beside its invoker: not clamped into the list.
                let pane = try XCTUnwrap(p.navigation.split.pane(holding: opener))
                XCTAssertTrue(pop.isDescendant(of: pane))
                let shown = pop.convert(pop.bounds, to: nil), invoker = opener.convert(opener.bounds, to: nil)
                XCTAssertEqual(shown.minX, invoker.minX, accuracy: 1, "\(shown) under \(invoker)")
                // A menu-shaped one (AppKit's menu) pops up there too, not clamped into the list.
                let menu = NSMenu()
                menu.addItem(withTitle: "Row", action: nil, keyEquivalent: "")
                menu.minimumWidth = 100
                XCTAssertEqual(p.menus.popUpPoint(menu, pop, in: opener).x, 0, accuracy: 1, "under its invoker's left edge")
                // Escape closes it with the keyboard still in the pane.
                XCTAssertTrue(window.makeFirstResponder(try XCTUnwrap(try node("field-a").field)))
                let escape = try XCTUnwrap(NSEvent.keyEvent(with: .keyDown, location: .zero, modifierFlags: [], timestamp: 0,
                    windowNumber: window.windowNumber, context: nil, characters: "\u{1b}", charactersIgnoringModifiers: "\u{1b}", isARepeat: false, keyCode: 53))
                XCTAssertTrue(p.menus.key(escape))
                XCTAssertFalse(p.menus.isOpen(pop), "Escape from the pane's field")
                // Open again, then the split ends (the driver's own chrome): the
                // route goes back into the page, and the open popover's layer with it.
                try press("pop-open")
                let again = Date().addingTimeInterval(3)
                while !p.menus.isOpen(pop), Date() < again { settle() }
                XCTAssertTrue(pop.isDescendant(of: pane))
                // And one opened after it with no invoker (`showPopover`), in the page's layer.
                let later = try node("listpop")
                p.menus.action("showPopover", ["listpop"])
                let opened = Date().addingTimeInterval(3)
                while !p.menus.isOpen(later), Date() < opened { settle() }
                // A depth from the pane's own ranking must not travel with the
                // layer: a box ranked there that stays when the split ends (as a
                // retained route of another stack would) gives the pane's layers
                // a deeper depth than the page's.
                let paneLayer = try XCTUnwrap(pop.superview)
                let retained = NodeView(id: 9_999_001, kind: "view", presenter: p)
                pane.addSubview(retained, positioned: .below, relativeTo: nil)
                retained.setRank(2)
                XCTAssertGreaterThan(paneLayer.layer?.zPosition ?? 0, 0.0015, "deeper by the pane's own ranking")
                p.navigation.split.painted = true
                settle()
                XCTAssertFalse(p.navigation.split.active, "no split")
                XCTAssertTrue(p.menus.isOpen(pop) && p.menus.isOpen(later))
                XCTAssertTrue(pop.isDescendant(of: p.viewport), "its layer follows the route into the page")
                let layers = p.viewport.subviews
                let (first, second) = (layers.firstIndex { pop.isDescendant(of: $0) }, layers.firstIndex { later.isDescendant(of: $0) })
                XCTAssertLessThan(try XCTUnwrap(first), try XCTUnwrap(second), "under the one opened after it")
                let depths = (paneLayer.layer?.zPosition, later.superview?.layer?.zPosition)
                XCTAssertEqual(depths.0, depths.1, "ranked among the page's layers, so their order paints")
                retained.removeFromSuperview()
            }
            session.destroy(); self.session = nil; window.close(); self.window = nil
        }
    }

    /// A route mounted in the pane after a painted popover opened from it (a
    /// push) goes under the popover's layer, which stays on top of the pane,
    /// so a click on the popover reaches it, as it paints.
    func testARouteMountedInThePaneStaysUnderAnOpenPopover() throws {
        let source = app(tabs: false).replacingOccurrences(
            of: "testId=`field-${e.params.id}`",
            with: "testId=`field-${e.params.id}`\n            button testId=\"pane-pop-open\" popovertarget=\"panepop\"\n              text \"Keep\""
        ).replacingOccurrences(
            of: "      for e in stack(nav) key=e.id\n",
            with: "      column id=\"panepop\" testId=\"panepop\" popover=\"manual\" width=100 height=40\n        box testId=\"in-pop\" width=100 height=40\n      for e in stack(nav) key=e.id\n")
        let session = try boot(tabs: false, size: CGSize(width: 1000, height: 500), source: source)
        _ = compose(ExactView(session: session), width: 1000, sidebar: false)
        let window = try XCTUnwrap(self.window)
        window.setFrameOrigin(NSPoint(x: 120, y: 120))
        window.orderFrontRegardless()
        let p = session.presenter
        p.menus.painted = true
        func settle() { for _ in 0..<20 { p.settlePump(); session.drainFill(); RunLoop.main.run(until: Date().addingTimeInterval(0.02)) } }
        try press("row-a")
        settle()
        let pop = try node("panepop")
        try press("pane-pop-open")
        let end = Date().addingTimeInterval(3)
        while !p.menus.isOpen(pop), Date() < end { settle() }
        let pane = try XCTUnwrap(p.navigation.split.pane(holding: try node("pane-pop-open")))
        let layer = try XCTUnwrap(pop.superview)
        XCTAssertTrue(layer.superview === pane)
        try press("notes-a")
        settle()
        XCTAssertTrue(has("route-notes"), "notes pushed into the pane")
        XCTAssertTrue(p.menus.isOpen(pop), "a manual popover stays open")
        XCTAssertTrue(pane.subviews.last === layer, "the layer is back on top of the pane")
        let inside = try node("in-pop")
        let point = inside.convert(NSPoint(x: inside.bounds.midX, y: inside.bounds.midY), to: nil)
        let hit = try XCTUnwrap(window.contentView?.hitTest(window.contentView!.convert(point, from: nil)))
        XCTAssertTrue(hit.isDescendant(of: inside), "the click reaches the popover: \(type(of: hit))")
    }

    /// `source` with each line that contains `marker` replaced by `lines`,
    /// written at that line's indent.
    private func replacing(_ marker: String, with lines: [String], in source: String) -> String {
        source.components(separatedBy: "\n").flatMap { line -> [String] in
            guard line.contains(marker) else { return [line] }
            let indent = String(line.prefix { $0 == " " })
            return lines.map { indent + $0 }
        }.joined(separator: "\n")
    }

    /// LLP 1116.006 D2: the box the root route marks
    /// `navigationSplit="placeholder"` is what the empty pane shows, in
    /// place of AppKit's "No Selection": laid out at the pane's size, its
    /// content centred, its size kept when the author's rows change, and
    /// `display: none` in its route again while a detail shows.
    func testTheAuthorsPlaceholderIsTheEmptyPanes() throws {
        let source = replacing("list testId=\"books\"", with: [
            "column navigationSplit=\"placeholder\" width=50 margin-left=(counted > 0 ? 30 : 0) testId=\"own\"",
            "  text \"Pick a book\" testId=\"own-text\"",
            "  button press=count testId=\"own-count\"",
            "    text \"Count\"",
            "list testId=\"books\" flex=1 min-height=0",
        ], in: app(tabs: false))
        let session = try boot(tabs: false, size: CGSize(width: 1000, height: 500), source: source)
        let columns = compose(ExactView(session: session), width: 1000, sidebar: false)
        let split = session.presenter.navigation.split, pane = columns.detailPane
        let own = try node("own"), library = try node("route-library")
        func shown() throws {
            XCTAssertTrue(own.superview === pane)
            XCTAssertEqual(own.style["display"]?.string, "flex")
            XCTAssertEqual(own.frame, NSRect(origin: .zero, size: pane.bounds.size))
            XCTAssertNil(columns.detail.saysNothing, "no words of AppKit's over the author's")
            XCTAssertEqual(split.summary["placeholder"] as? Int, Int(own.id))
            let words = try node("own-text"), text = pane.convert(words.bounds, from: words)
            XCTAssertEqual(text.midX, pane.bounds.midX, accuracy: 1)
        }
        try shown()
        // The author's margin changes, which moves the box and sends no style: the pane's box stands, as the web shell's `!important` does.
        try press("own-count")
        try shown()
        // A detail: the pane is the route's, and the box is no box in the list's column.
        try press("row-a")
        XCTAssertTrue(try node("route-book-a").superview === pane)
        XCTAssertTrue(own.isDescendant(of: library))
        XCTAssertEqual(own.style["display"]?.string, "none")
        XCTAssertTrue(split.summary["placeholder"] is NSNull)
        XCTAssertTrue(split.closeDetail())
        try shown()
        // A wider window: the box follows the pane.
        width(try XCTUnwrap(window), 1200)
        try shown()
        // The driver's own chrome pushes: the box is in its route, hidden.
        split.painted = true
        XCTAssertTrue(own.isDescendant(of: library))
        XCTAssertEqual(own.style["display"]?.string, "none")
    }

    /// The library with its books in the painted grouped form (D6), as
    /// Excord's channels are: a section of rows whose press is an action
    /// that pushes the book. A book's Next replaces it with the next.
    private func paintedList() -> String {
        let swapping = app(tabs: false).replacingOccurrences(of: "  action count\n", with: "  action swap(url: string)\n    nav = replace(nav, url)\n  action count\n")
        let next = replacing("box testId=`half-${e.params.id}`", with: [
            "box testId=`half-${e.params.id}` width=\"50%\" height=40 background-color=\"#f9ab00\"",
            "button press=swap(path(\"book\", e.params.id == \"a\" ? \"b\" : e.params.id == \"b\" ? \"c\" : \"a\")) testId=`next-${e.params.id}`",
            "  text \"Next\"",
        ], in: swapping)
        let grouped = next.replacingOccurrences(of: "list testId=\"books\"", with: "list testId=\"books\" appearance=\"auto\" listStyle=\"plain\"")
        // A grouped list's rows are in sections; the link rows go.
        let bare = replacing("text `Book ${id}`", with: [], in: replacing("link href=path(\"book\", id)", with: [], in: grouped))
        return replacing("for id in [\"a\", \"b\", \"c\"] key=id", with: [
            "section",
            "  for id in (counted > 1 ? [\"a\", \"b\"] : [\"a\", \"b\", \"c\"]) key=id",
            "    button press=follow(path(\"book\", id)) testId=`row-${id}`",
            "      row gap=8 align-items=\"center\" flex=1 min-width=0",
            "        text `Book ${id}` flex=1 testId=`name-${id}`",
            "        image \"symbol:forward-chevron\" testId=`chevron-${id}`",
            "  button press=count testId=\"row-count\"",
            "    text \"Count\"",
            "  input value=query input=search placeholder=\"Filter\" testId=\"row-field\"",
        ], in: bare)
    }

    /// LLP 1115.002 D5: the list's column draws no disclosure chevron, as
    /// a hand-built Mac list beside its detail has none, also where the list
    /// is no table but the painted grouped form (D6), as Excord's channels
    /// are. Pushed, the rows draw them again.
    func testAPaintedListBesideItsDetailDrawsNoChevrons() throws {
        let session = try boot(tabs: false, size: CGSize(width: 1000, height: 500), source: paintedList())
        _ = compose(ExactView(session: session), width: 1000, sidebar: false)
        let p = session.presenter, split = p.navigation.split
        XCTAssertTrue(split.active)
        XCTAssertNil(p.tables.feeds[try node("books").id], "the grouped form is painted")
        func drawn(_ id: String) throws -> CGFloat? { try node(id).symbolView?.alphaValue }
        XCTAssertEqual(try drawn("chevron-a"), 0)
        XCTAssertEqual(try drawn("chevron-c"), 0)
        try press("row-b")
        XCTAssertEqual(try drawn("chevron-b"), 0, "beside its detail too")
        split.painted = true
        XCTAssertEqual(try drawn("chevron-a"), 1, "a pushed stack's rows keep theirs")
        split.painted = false
        XCTAssertEqual(try drawn("chevron-a"), 0)
    }

    /// LLP 1115.002 D5: the painted list's row whose detail shows is drawn
    /// selected, as a table's row is: AppKit's selection, the separators
    /// beside it not drawn, and while the window is key with the keyboard
    /// in the list or nowhere, emphasized, its text in the selected item's
    /// ink. The selection moves with the detail and goes with it.
    func testAPaintedListMarksTheRowWhoseDetailShows() throws {
        let session = try boot(tabs: false, size: CGSize(width: 1000, height: 500), source: paintedList())
        let columns = compose(ExactView(session: session), width: 1000, sidebar: false, window: KeyedWindow.self)
        let window = try XCTUnwrap(self.window)
        let p = session.presenter, split = p.navigation.split
        func mark(_ id: String) throws -> RowSelectionLayer? {
            let row = try node(id)
            return row.superview?.layer?.sublayers?.lazy.compactMap { $0 as? RowSelectionLayer }.first { $0.row === row }
        }
        func separated(_ id: String) throws -> Bool? {
            try node(id).layer?.sublayers?.lazy.compactMap { $0 as? GroupedSeparatorLayer }.first.map { !$0.isHidden }
        }
        func fill(_ emphasized: Bool, in id: String) throws -> CGColor? {
            var color: CGColor?
            try node(id).effectiveAppearance.performAsCurrentDrawingAppearance {
                color = (emphasized ? NSColor.selectedContentBackgroundColor : .unemphasizedSelectedContentBackgroundColor).cgColor
            }
            return color
        }
        func ink(_ id: String) throws -> [Double] {
            let spec = try node(id).paragraphSpec()
            return (spec.runs.first?.color ?? spec.color).map { ($0 * 100).rounded() / 100 }
        }
        XCTAssertTrue(split.active)
        XCTAssertNil(try mark("row-a"))
        XCTAssertEqual(try separated("row-a"), true)
        XCTAssertEqual(try separated("row-b"), true)
        let plain = try ink("name-b")

        // A row is pressed: its detail shows, and the row is drawn selected, 8 points before its text.
        try press("row-b")
        XCTAssertTrue(try node("route-book-b").superview === columns.detailPane)
        XCTAssertEqual(split.summary["marked"] as? Int, Int(try node("row-b").id))
        let row = try node("row-b"), selection = try XCTUnwrap(mark("row-b"))
        let card = try XCTUnwrap(row.superview), text = card.convert(try node("name-b").bounds, from: try node("name-b"))
        XCTAssertEqual(selection.frame.minX, text.minX - 8, accuracy: 0.5)
        XCTAssertEqual(selection.frame.maxX, row.frame.maxX - row.number("padding_right") + 8, accuracy: 0.5)
        XCTAssertEqual(selection.frame.minY, row.frame.minY)
        XCTAssertEqual(selection.frame.height, row.frame.height)
        XCTAssertEqual(selection.cornerRadius, 7)
        XCTAssertNil(try mark("row-a"))
        XCTAssertEqual(try separated("row-a"), false, "no separator above the selected row")
        XCTAssertEqual(try separated("row-b"), false, "nor under it")
        // Another row, whose press pushes over the first: it is that row's detail, with no Back, and the
        // selection moves, and so do the separators it hides.
        try press("row-c")
        XCTAssertFalse(p.navigation.canGoBack)
        XCTAssertNil(try mark("row-b"))
        XCTAssertNotNil(try mark("row-c"))
        XCTAssertEqual(try separated("row-a"), true)
        XCTAssertEqual(try separated("row-b"), false)

        // In the key window, with the keyboard nowhere: emphasized, and its text the selected item's. A
        // test process cannot make its window key, so the window says it is, and says when it stops.
        let keyed = try XCTUnwrap(window as? KeyedWindow)
        keyed.key = true
        NotificationCenter.default.post(name: NSWindow.didBecomeKeyNotification, object: keyed)
        XCTAssertTrue(window.makeFirstResponder(nil))
        XCTAssertEqual(try mark("row-c")?.backgroundColor, try fill(true, in: "row-c"))
        XCTAssertEqual(try ink("name-c"), [255, 255, 255, 255])
        XCTAssertEqual(try ink("name-a"), plain, "the rows beside it are as they were")
        // A row whose press only acts, writing no router: the detail is still the other row's.
        try press("row-count")
        XCTAssertEqual(split.shown?.row, try node("row-c").id)
        XCTAssertNotNil(try mark("row-c"))
        XCTAssertNil(try mark("row-count"))
        // A field in the list has the keyboard: emphasized no more, as a table would not be first responder.
        XCTAssertTrue(window.makeFirstResponder(try node("row-field").field))
        XCTAssertEqual(try mark("row-c")?.backgroundColor, try fill(false, in: "row-c"))
        XCTAssertTrue(window.makeFirstResponder(nil))
        XCTAssertEqual(try mark("row-c")?.backgroundColor, try fill(true, in: "row-c"))
        // The keyboard goes to the detail: a table there would no longer be first responder.
        XCTAssertTrue(window.makeFirstResponder(try node("field-c").field))
        XCTAssertEqual(try mark("row-c")?.backgroundColor, try fill(false, in: "row-c"))
        XCTAssertEqual(try ink("name-c"), plain)
        XCTAssertTrue(window.makeFirstResponder(nil))
        XCTAssertEqual(try ink("name-c"), [255, 255, 255, 255])
        // Another window takes key.
        keyed.key = false
        NotificationCenter.default.post(name: NSWindow.didResignKeyNotification, object: keyed)
        XCTAssertEqual(try mark("row-c")?.backgroundColor, try fill(false, in: "row-c"))
        XCTAssertEqual(try ink("name-c"), plain)
        keyed.key = true
        NotificationCenter.default.post(name: NSWindow.didBecomeKeyNotification, object: keyed)
        XCTAssertEqual(try ink("name-c"), [255, 255, 255, 255])

        // A control in the detail replaces its book with another: the route moved where no row's
        // press or link took it, so no row is selected (LLP 1127 D3), and the pane begins where it did.
        try press("next-c")
        let swapped = try node("route-book-a")
        XCTAssertTrue(swapped.superview === columns.detailPane)
        XCTAssertEqual(split.shown?.detail, swapped)
        XCTAssertFalse(p.navigation.canGoBack)
        XCTAssertNil(try mark("row-c"))
        XCTAssertNil(try mark("row-a"))
        XCTAssertTrue(split.summary["marked"] is NSNull)
        XCTAssertEqual(try ink("name-c"), plain)
        XCTAssertEqual(try separated("row-b"), true)
        // Its row is pressed: selected again.
        try press("row-a")
        XCTAssertNotNil(try mark("row-a"))

        // The detail goes: no row is selected, and the separators are back.
        XCTAssertTrue(split.closeDetail())
        XCTAssertNil(try mark("row-a"))
        XCTAssertEqual(try ink("name-a"), plain)
        XCTAssertEqual(try separated("row-a"), true)
        XCTAssertTrue(split.summary["marked"] is NSNull)
        // Pushed, as under the driver's own chrome: the list is no split's, and marks nothing.
        try press("row-a")
        XCTAssertNotNil(try mark("row-a"))
        split.painted = true
        XCTAssertNil(try mark("row-a"))
        split.painted = false

        // The selected row leaves the list while its detail shows: its selection goes with it.
        try press("row-c")
        let holder = try XCTUnwrap(try node("row-c").superview)
        XCTAssertNotNil(try mark("row-c"))
        try press("row-count")
        XCTAssertFalse(has("row-c"))
        XCTAssertNotNil(split.shown?.detail)
        XCTAssertFalse(holder.layer?.sublayers?.contains { $0 is RowSelectionLayer } ?? false)
    }

    /// A list of the author's own rows (`appearance="none"`) is painted as
    /// written: no selection is drawn on its row, as none is on a card.
    func testTheAuthorsOwnRowsAreNotMarked() throws {
        let source = app(tabs: false).replacingOccurrences(of: "list testId=\"books\"", with: "list testId=\"books\" appearance=\"none\"")
        let session = try boot(tabs: false, size: CGSize(width: 1000, height: 500), source: source)
        let columns = compose(ExactView(session: session), width: 1000, sidebar: false)
        let split = session.presenter.navigation.split
        XCTAssertNil(session.presenter.tables.feeds[try node("books").id])
        try press("row-a")
        XCTAssertTrue(try node("route-book-a").superview === columns.detailPane)
        XCTAssertTrue(split.summary["marked"] is NSNull)
        XCTAssertFalse(try node("row-a").superview?.layer?.sublayers?.contains { $0 is RowSelectionLayer } ?? false)
    }

    /// A node in the detail's pane is hovered as one in the page is: the
    /// pane is beside the viewport, not in it.
    func testHoverReachesTheDetailPane() throws {
        let source = app(tabs: false)
            .replacingOccurrences(of: "  state query = \"\"", with: "  state hot = false\n  state query = \"\"")
            .replacingOccurrences(of: "  action count\n", with: "  action hovered(over: bool)\n    hot = over\n  action count\n")
        let hovering = replacing("box testId=`half-${e.params.id}`", with: [
            "box testId=`half-${e.params.id}` width=\"50%\" height=40 background-color=\"#f9ab00\" hover=hovered",
            "text (hot ? \"hot\" : \"cold\") testId=\"hot\"",
        ], in: source)
        let session = try boot(tabs: false, size: CGSize(width: 1000, height: 500), source: hovering)
        let columns = compose(ExactView(session: session), width: 1000, sidebar: false)
        try press("row-a")
        let box = try node("half-a"), agent = Agent(session: session)
        XCTAssertTrue(box.isDescendant(of: columns.detailPane))
        let reply = agent.tap(["id": Int(box.id), "hover": true])
        XCTAssertNil(reply["error"], "\(reply)")
        XCTAssertTrue(session.presenter.hoveredNodes.contains { $0 === box })
        XCTAssertEqual(try node("hot").accessibleText, "hot")
        // Over the list: the box is left.
        XCTAssertNil(agent.tap(["id": Int(try node("row-b").id), "hover": true])["error"])
        XCTAssertFalse(session.presenter.hoveredNodes.contains { $0 === box })
        XCTAssertEqual(try node("hot").accessibleText, "cold")
    }
}

/// A window that is key while it says so.
private final class KeyedWindow: NSWindow {
    var key = false
    override var isKeyWindow: Bool { key }
}
#endif
