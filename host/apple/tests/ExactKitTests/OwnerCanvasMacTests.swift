#if os(macOS)
import AppKit
import XCTest
@testable import ExactKit
@testable import ExactSurfaces

/// LLP 1072 §8.5: canvas draws run in a turn of their own on the owner, not in
/// the turns main waits on. A list of 2D canvas rows is scrolled down and
/// back with its slices and its draws on the owner, retiring canvases and
/// making new ones, while main reads each draw's lists and replays them.
/// The macOS host draws synchronously in the app; the test turns the
/// asynchronous path on, so the Thread Sanitizer (`swift test
/// --sanitize=thread`) sees the owner drawing while main applies.
final class OwnerCanvasMacTests: XCTestCase {
    override class func setUp() { super.setUp(); ExactSurfaces.install() } // LLP 1047.001 D4
    private var window: NSWindow?
    override func tearDown() { window?.close(); window = nil; ExactSession.asyncFills = false }

    func testCanvasRowsScrolledWithDrawsOnTheOwnerShowTheirBitmaps() throws {
        _ = NSApplication.shared
        let dir = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
        try FileManager.default.createDirectory(at: dir, withIntermediateDirectories: true)
        defer { try? FileManager.default.removeItem(at: dir) }
        let source = """
        shape Location
          lat: number
          lon: number
        shape Station
          id: string
          name: string
          zone: number
          distance: number
        component App
          resource location = defaultLocation() as shape Location
          resource allStations = stations(location) as shape list<Station>
          view
            column width="100%" height="100%"
              list virtualized=true estimated-item-height=90 flex=1 min-height=0 width="100%" testId="feed"
                each s in allStations key=s.id
                  column width="100%" padding=8
                    text `${s.name}, zone ${s.zone}`
                    canvas surface=map(allStations, s.id, allStations, s.zone) width="100%" height=60 testId="map"

        """
        try source.write(to: dir.appendingPathComponent("app.contract"), atomically: true, encoding: .utf8)
        let compiler = Process()
        compiler.executableURL = URL(fileURLWithPath: try XCTUnwrap(ProcessInfo.processInfo.environment["EXACT_CONTRACT"], "build.mjs --test builds the Contract compiler and names it"))
        compiler.arguments = ["build", dir.appendingPathComponent("app.contract").path, "-o", dir.appendingPathComponent("app.plan").path]
        try compiler.run(); compiler.waitUntilExit()
        XCTAssertEqual(compiler.terminationStatus, 0)
        let bytes = try Data(contentsOf: dir.appendingPathComponent("app.plan"))

        ExactSession.asyncFills = true
        let session = ExactApp.shared.makeSession(label: "owner-canvas")
        defer { session.destroy() }
        // Batches are decoded on the owner: count, under a lock, the turns
        // that left a draw owed and the ones that drew.
        let counts = NSLock()
        var owed = 0, drew = 0
        session.runtime.observeBatch = { _, batch in
            counts.lock(); defer { counts.unlock() }
            if batch.canvasOwed { owed += 1 }
            if batch.ops.contains(where: { $0.op == .canvas2d }) { drew += 1 }
        }
        defer { session.runtime.observeBatch = nil }
        let view = ExactView(session: session)
        let window = NSWindow(contentRect: NSRect(x: 0, y: 0, width: 320, height: 240), styleMask: [.titled], backing: .buffered, defer: false)
        window.isReleasedWhenClosed = false
        window.contentView = view
        self.window = window
        view.layoutSubtreeIfNeeded()
        XCTAssertNil(session.boot(plan: bytes, size: CGSize(width: 320, height: 240)).error)
        func spin(_ seconds: Double) { RunLoop.main.run(until: Date().addingTimeInterval(seconds)) }
        func canvases() -> [NodeView] { session.presenter.views.values.filter { $0.kind == "canvas2d" } }
        let deadline = Date().addingTimeInterval(5)
        while canvases().count < 2, Date() < deadline { RunLoop.main.run(mode: .default, before: Date().addingTimeInterval(0.02)) }
        let list = try XCTUnwrap(session.presenter.views.values.first { $0.kind == "list" })
        let clip = try XCTUnwrap(list.scroll?.contentView)
        let extent = (list.scroll?.documentView?.frame.height ?? 0) - clip.bounds.height
        XCTAssertGreaterThan(extent, 500, "the list is taller than its port")
        // Down and back, twice, a little each frame: rows retire and are made
        // again, each new canvas drawn on the owner while main replays.
        for _ in 0..<2 {
            for y in stride(from: 0.0, through: extent, by: 45) { clip.scroll(to: NSPoint(x: 0, y: y)); spin(0.004) }
            for y in stride(from: extent, through: 0.0, by: -45) { clip.scroll(to: NSPoint(x: 0, y: y)); spin(0.004) }
        }
        // Settle: every draw owed has run and landed, and every replay shown.
        let settle = Date().addingTimeInterval(5)
        repeat {
            session.drainFill()
            spin(0.02)
            session.presenter.canvas2d.waitForReplays()
        } while (session.canvasInFlight || canvases().contains { !$0.showsBitmap }) && Date() < settle
        counts.lock(); let (owedTurns, drawTurns) = (owed, drew); counts.unlock()
        XCTAssertGreaterThan(owedTurns, 0, "turns main waited on left their draws owed")
        XCTAssertGreaterThan(drawTurns, 0, "draws ran in turns of their own")
        let shown = canvases()
        XCTAssertGreaterThanOrEqual(shown.count, 2, "the list still shows canvas rows")
        for c in shown { XCTAssertTrue(c.showsBitmap, "canvas \(c.id) shows its bitmap") }
    }
}

private extension NodeView {
    /// A replayed bitmap under the canvas's layer.
    var showsBitmap: Bool {
        func deep(_ l: CALayer) -> Bool { l.contents != nil || (l.sublayers ?? []).contains(where: deep) }
        return layer.map(deep) ?? false
    }
}
#endif
