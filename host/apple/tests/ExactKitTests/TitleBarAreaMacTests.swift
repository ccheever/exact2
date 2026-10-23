#if os(macOS)
import AppKit
import XCTest
@testable import ExactKit

/// `env(titlebar-area-*)` on the Mac: AppKit's title bar beside the traffic
/// lights under `viewport-fit=cover`, zero in full screen and under a
/// toolbar, undefined (the fallback) where the page does not underlap the
/// title bar.
final class TitleBarAreaMacTests: XCTestCase {
    private final class Window: NSWindow {
        var fullscreen = false
        override var styleMask: NSWindow.StyleMask {
            get { fullscreen ? super.styleMask.union(.fullScreen) : super.styleMask }
            set { super.styleMask = newValue }
        }
    }

    private static let plan: Result<Data, Error> = Result {
        let dir = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
        try FileManager.default.createDirectory(at: dir, withIntermediateDirectories: true)
        defer { try? FileManager.default.removeItem(at: dir) }
        let source = """
        component App
          view
            main viewport-fit="cover" width="100%" height="100%"
              row testId="bar" margin-left="env(titlebar-area-x, 5px)" width="env(titlebar-area-width, 50px)" height="env(titlebar-area-height, 9px)"

        """
        try source.write(to: dir.appendingPathComponent("app.contract"), atomically: true, encoding: .utf8)
        let compiler = Process()
        compiler.executableURL = URL(fileURLWithPath: try XCTUnwrap(ProcessInfo.processInfo.environment["EXACT_CONTRACT"]))
        compiler.arguments = ["build", dir.appendingPathComponent("app.contract").path, "-o", dir.appendingPathComponent("app.plan").path]
        try compiler.run(); compiler.waitUntilExit()
        XCTAssertEqual(compiler.terminationStatus, 0)
        return try Data(contentsOf: dir.appendingPathComponent("app.plan"))
    }

    private func bar(_ session: ExactSession, file: StaticString = #filePath, line: UInt = #line) throws -> NSRect {
        let node = try XCTUnwrap(session.presenter.views.values.first { $0.props["testId"] == "bar" }, file: file, line: line)
        return node.frame
    }

    private func awaitEnvironment(_ view: ExactView, file: StaticString = #filePath, line: UInt = #line) {
        let p = view.session.presenter
        func current() -> Bool {
            p.titlebarArea == TitleBarAreaMac.rect(of: view, cover: true)
                && p.regions == LayoutRegionsMac.insets(of: view)
                && p.insets.top == view.safeAreaInsets.top
        }
        let end = Date().addingTimeInterval(2)
        while !current(), Date() < end { RunLoop.main.run(mode: .default, before: Date().addingTimeInterval(0.05)) }
        XCTAssertTrue(current(), "environment follows AppKit without a manual sync", file: file, line: line)
    }

    func testToolbarVisibilityAndFixedSizeViewMovementRefreshTheEnvironment() throws {
        _ = NSApplication.shared
        let session = ExactApp.shared.makeSession(label: "moving-titlebar-area")
        let window = Window(contentRect: NSRect(x: 100, y: 100, width: 900, height: 600),
                            styleMask: [.titled, .resizable, .fullSizeContentView], backing: .buffered, defer: false)
        window.isReleasedWhenClosed = false
        let container = NSView(frame: NSRect(x: 0, y: 0, width: 900, height: 600))
        let parent = NSView(frame: container.bounds)
        container.addSubview(parent)
        window.contentView = container
        XCTAssertNil(session.boot(plan: try Self.plan.get(), size: CGSize(width: 900, height: 500)).error)
        let view = ExactView(session: session)
        view.frame = NSRect(x: 0, y: 100, width: 900, height: 500)
        parent.addSubview(view)
        defer { window.close(); session.destroy() }
        view.layoutSubtreeIfNeeded()
        awaitEnvironment(view)
        XCTAssertNotNil(session.presenter.titlebarArea)
        let size = view.bounds.size

        let toolbar = NSToolbar(identifier: "visibility-environment")
        window.toolbar = toolbar
        awaitEnvironment(view)
        XCTAssertEqual(session.presenter.titlebarArea, [0, 0, 0, 0])
        toolbar.isVisible = false
        awaitEnvironment(view)
        XCTAssertNotEqual(session.presenter.titlebarArea, [0, 0, 0, 0])
        toolbar.isVisible = true
        awaitEnvironment(view)
        XCTAssertEqual(session.presenter.titlebarArea, [0, 0, 0, 0])
        window.toolbar = nil
        awaitEnvironment(view)

        view.setFrameOrigin(.zero)
        awaitEnvironment(view)
        XCTAssertNil(session.presenter.titlebarArea, "the view moved below the title bar")
        view.setFrameOrigin(NSPoint(x: 0, y: 100))
        awaitEnvironment(view)
        XCTAssertNotNil(session.presenter.titlebarArea)
        parent.setFrameOrigin(NSPoint(x: 0, y: -100))
        awaitEnvironment(view)
        XCTAssertNil(session.presenter.titlebarArea, "moving an ancestor changes window coordinates too")
        parent.setFrameOrigin(.zero)
        awaitEnvironment(view)
        XCTAssertNotNil(session.presenter.titlebarArea)
        XCTAssertEqual(view.bounds.size, size)
    }

    func testTheAreaIsAppKitsBesideTheTrafficLightsAndZeroInFullScreenOrUnderAToolbar() throws {
        _ = NSApplication.shared
        let session = ExactApp.shared.makeSession(label: "titlebar-area")
        let window = Window(contentRect: NSRect(x: 100, y: 100, width: 900, height: 600),
                            styleMask: [.titled, .closable, .miniaturizable, .resizable, .fullSizeContentView], backing: .buffered, defer: false)
        window.titlebarAppearsTransparent = true
        window.titleVisibility = .hidden
        window.isReleasedWhenClosed = false
        let view = ExactView(session: session)
        window.contentView = view
        defer { window.close(); session.destroy() }
        XCTAssertNil(session.boot(plan: try Self.plan.get(), size: CGSize(width: 900, height: 600)).error)
        view.layoutSubtreeIfNeeded()
        view.syncInsets()

        // AppKit's geometry: the title bar's band, less the buttons and the
        // margin before the first kept after the last
        let close = try XCTUnwrap(window.standardWindowButton(.closeButton))
        let zoom = try XCTUnwrap(window.standardWindowButton(.zoomButton))
        let band = try XCTUnwrap(close.superview)
        let x = zoom.frame.maxX + close.frame.minX
        XCTAssertEqual(band.frame.height, view.safeAreaInsets.top)
        if #available(macOS 26.0, *) {
            XCTAssertEqual(x, view.rect(for: .safeArea(cornerAdaptation: .horizontal)).minX, "where AppKit's corner-adapted safe area begins")
        }
        // The traffic lights are centred in the band the area's height reports
        XCTAssertEqual(close.frame.midY, band.bounds.midY)
        XCTAssertEqual(session.presenter.titlebarArea, [x, 0, 900 - x, band.frame.height])
        var frame = try bar(session)
        XCTAssertEqual([frame.minX, frame.width, frame.height], [x, 900 - x, band.frame.height])

        // Full screen: the title bar leaves the window, and every variable is zero
        window.fullscreen = true
        NotificationCenter.default.post(name: NSWindow.didEnterFullScreenNotification, object: window)
        XCTAssertEqual(session.presenter.titlebarArea, [0, 0, 0, 0])
        frame = try bar(session)
        XCTAssertEqual([frame.minX, frame.width, frame.height], [0, 0, 0])
        window.fullscreen = false
        NotificationCenter.default.post(name: NSWindow.didExitFullScreenNotification, object: window)
        XCTAssertEqual(session.presenter.titlebarArea, [x, 0, 900 - x, band.frame.height])

        // A toolbar fills the title bar (LLP 1115.002 D2): zero too
        window.toolbar = NSToolbar(identifier: "titlebar-area")
        view.syncInsets()
        XCTAssertEqual(session.presenter.titlebarArea, [0, 0, 0, 0])
        window.toolbar = nil
        view.syncInsets()
        XCTAssertEqual(session.presenter.titlebarArea, [x, 0, 900 - x, band.frame.height])

        // `window-controls-overlay`'s empty unified toolbar (LLP 1039 D8): the
        // area is its taller band beside the traffic lights, and the display
        // mode is the overlay's
        let bare = band.frame.height
        window.toolbar = NSToolbar(identifier: "titlebar-area-overlay")
        window.toolbarStyle = .unified
        view.controlsOverlay = true
        view.layoutSubtreeIfNeeded()
        view.syncInsets()
        let row = try XCTUnwrap(close.superview)
        XCTAssertGreaterThan(row.frame.height, bare, "the unified toolbar's row is taller than the bare title bar")
        let overlayX = zoom.frame.maxX + close.frame.minX
        XCTAssertEqual(session.presenter.titlebarArea, [overlayX, 0, 900 - overlayX, row.frame.height])
        XCTAssertEqual(DisplayPreferences.displayMode(view), 4, "window-controls-overlay")
        view.controlsOverlay = false
        view.syncInsets()
        XCTAssertEqual(session.presenter.titlebarArea, [0, 0, 0, 0], "a route's toolbar: zero")
        XCTAssertEqual(DisplayPreferences.displayMode(view), 0)
        window.toolbar = nil
        window.toolbarStyle = .automatic
        view.layoutSubtreeIfNeeded()
        view.syncInsets()
        XCTAssertEqual(session.presenter.titlebarArea, [x, 0, 900 - x, band.frame.height])

        // The page below the title bar: undefined, so each fallback applies
        window.styleMask.remove(.fullSizeContentView)
        view.syncInsets()
        XCTAssertNil(session.presenter.titlebarArea)
        frame = try bar(session)
        XCTAssertEqual([frame.minX, frame.width, frame.height], [5, 50, 9])

        // A view embedded below the title bar of a full-size window: undefined too
        window.styleMask.insert(.fullSizeContentView)
        let container = NSView(frame: NSRect(x: 0, y: 0, width: 900, height: 600))
        window.contentView = container
        view.frame = NSRect(x: 0, y: 0, width: 900, height: 500)
        container.addSubview(view)
        view.syncInsets()
        XCTAssertNil(session.presenter.titlebarArea)
        // Under a toolbar too: the view still does not underlap the title bar
        window.toolbar = NSToolbar(identifier: "titlebar-area")
        view.syncInsets()
        XCTAssertNil(session.presenter.titlebarArea)
    }
}
#endif
