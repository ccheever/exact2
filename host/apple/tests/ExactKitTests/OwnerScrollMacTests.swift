#if os(macOS)
import AppKit
import XCTest
@testable import ExactKit

/// LLP 1072: a list scrolled with its slices built on the owner thread while
/// main paints what they mount. The macOS host keeps slices synchronous in
/// the app; the test turns the asynchronous path on, so the Thread
/// Sanitizer (`swift test --sanitize=thread`) sees owner-side measurement and
/// main-side painting of the same rows at once.
final class OwnerScrollMacTests: XCTestCase {
    private var window: NSWindow?
    override func tearDown() { window?.close(); window = nil; ExactSession.asyncFills = false }

    func testAListScrolledWithSlicesOnTheOwnerKeepsItsRows() throws {
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
              list virtualized=true estimated-item-height=60 flex=1 min-height=0 width="100%" testId="feed"
                each s in allStations key=s.id
                  column width="100%" padding=8
                    text `${s.name}: a paragraph long enough to wrap across the row several times, so measuring and painting both shape lines, zone ${s.zone}`
                    text `${s.name} again: a second paragraph, measured on the owner and painted on main at the same moment`

        """
        try source.write(to: dir.appendingPathComponent("app.contract"), atomically: true, encoding: .utf8)
        let compiler = Process()
        compiler.executableURL = URL(fileURLWithPath: try XCTUnwrap(ProcessInfo.processInfo.environment["EXACT_CONTRACT"], "build.mjs --test builds the Contract compiler and names it"))
        compiler.arguments = ["build", dir.appendingPathComponent("app.contract").path, "-o", dir.appendingPathComponent("app.plan").path]
        try compiler.run(); compiler.waitUntilExit()
        XCTAssertEqual(compiler.terminationStatus, 0)
        let bytes = try Data(contentsOf: dir.appendingPathComponent("app.plan"))

        ExactSession.asyncFills = true
        let session = ExactApp.shared.makeSession(label: "owner-scroll")
        defer { session.destroy() }
        let view = ExactView(session: session)
        let window = NSWindow(contentRect: NSRect(x: 0, y: 0, width: 320, height: 240), styleMask: [.titled], backing: .buffered, defer: false)
        window.isReleasedWhenClosed = false
        window.contentView = view
        self.window = window
        view.layoutSubtreeIfNeeded()
        XCTAssertNil(session.boot(plan: bytes, size: CGSize(width: 320, height: 240)).error)
        func spin(_ seconds: Double) { RunLoop.main.run(until: Date().addingTimeInterval(seconds)) }
        let deadline = Date().addingTimeInterval(5)
        while session.presenter.views.values.first(where: { $0.kind == "list" }).map({ $0.subviewsDeep.count < 3 }) ?? true, Date() < deadline { RunLoop.main.run(mode: .default, before: Date().addingTimeInterval(0.02)) }
        let list = try XCTUnwrap(session.presenter.views.values.first { $0.kind == "list" })
        let clip = try XCTUnwrap(list.scroll?.contentView)
        let extent = (list.scroll?.documentView?.frame.height ?? 0) - clip.bounds.height
        XCTAssertGreaterThan(extent, 500, "the list is taller than its port")
        // Down and back, twice, a little each frame, as a scroll arrives.
        for _ in 0..<2 {
            for y in stride(from: 0.0, through: extent, by: 45) { clip.scroll(to: NSPoint(x: 0, y: y)); spin(0.004) }
            for y in stride(from: extent, through: 0.0, by: -45) { clip.scroll(to: NSPoint(x: 0, y: y)); spin(0.004) }
        }
        session.drainFill()
        spin(0.1)
        XCTAssertFalse(session.presenter.collections.fillLatency.isEmpty, "slices were built on the owner")
        XCTAssertGreaterThan(list.subviewsDeep.count, 3, "the list still shows rows")
    }
}

private extension NSView {
    var subviewsDeep: [NSView] { subviews + subviews.flatMap(\.subviewsDeep) }
}
#endif
