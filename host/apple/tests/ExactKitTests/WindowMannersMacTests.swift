#if os(macOS)
import AppKit
import XCTest
@testable import ExactKit

/// LLP 1115.000 D7.9: a window the adapter makes opens with AppKit's own
/// animation (it was told `.none`), and window tabs are for an app whose
/// documents have windows of their own: a one-window app offers none, where
/// every window used to carry a tabbing identifier and AppKit's menus
/// offered Show Tab Bar.
final class WindowMannersMacTests: XCTestCase {
    private var windows: [NSWindow] = []
    override func tearDown() {
        windows.forEach { $0.close() }
        windows = []
        NSWindow.allowsAutomaticWindowTabbing = true // AppKit's own, for the tests after this
    }
    private func window() -> NSWindow {
        _ = NSApplication.shared
        let made = NSWindow(contentRect: NSRect(x: 0, y: 0, width: 300, height: 200), styleMask: [.titled, .closable, .resizable], backing: .buffered, defer: false)
        made.isReleasedWhenClosed = false
        windows.append(made)
        return made
    }

    func testAWindowOpensAsAppKitOpensOneAndAOneWindowAppOffersNoTabs() {
        let appKit = window(), one = window(), documents = window()
        ExactDocuments.furnish(one, tabs: false)
        XCTAssertEqual(one.animationBehavior, appKit.animationBehavior, "AppKit's own opening animation")
        XCTAssertNotEqual(one.animationBehavior, .none)
        XCTAssertFalse(NSWindow.allowsAutomaticWindowTabbing, "no Show Tab Bar in a one-window app's menus")
        XCTAssertEqual(one.tabbingMode, .disallowed)
        XCTAssertEqual(one.tabbingIdentifier, appKit.tabbingIdentifier, "and no identifier of the app's to gather windows under")
        // An app whose documents have their own windows keeps AppKit's tabs.
        ExactDocuments.furnish(documents, tabs: true)
        XCTAssertTrue(NSWindow.allowsAutomaticWindowTabbing)
        XCTAssertEqual(documents.tabbingMode, .automatic)
        XCTAssertEqual(documents.tabbingIdentifier, ExactEnv.appMetadata["CFBundleIdentifier"] as? String ?? ExactEnv.appName)
        XCTAssertEqual(documents.animationBehavior, appKit.animationBehavior)
        // With nothing said, the manifest's launch mode decides; a test has none, so one window.
        XCTAssertNotEqual(ExactDocuments.launchMode, "navigate-new")
        ExactDocuments.furnish(documents)
        XCTAssertFalse(NSWindow.allowsAutomaticWindowTabbing)
        XCTAssertEqual(documents.tabbingMode, .disallowed)
    }

    /// A counter: its state is what a hidden window must keep. It shows
    /// its page's `visibilityState`, `persisted` and `restores` too.
    private static let counter: Result<Data, Error> = Result {
        let dir = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
        try FileManager.default.createDirectory(at: dir, withIntermediateDirectories: true)
        defer { try? FileManager.default.removeItem(at: dir) }
        let source = "shape Page\n  visibilityState: string\n  persisted: bool\n  restores: number\ncomponent App\n  resource page = exactPage() as shape Page\n  state count = 0\n  action add\n    count = count + 1\n  view\n    column\n      text `${count}`\n      text `${page.visibilityState}` testId=\"visibility\"\n      text `${page.persisted} ${page.restores}` testId=\"transition\"\n      button \"Add\" testId=\"add\" press=add\n"
        try source.write(to: dir.appendingPathComponent("app.contract"), atomically: true, encoding: .utf8)
        let compiler = Process()
        compiler.executableURL = URL(fileURLWithPath: try XCTUnwrap(ProcessInfo.processInfo.environment["EXACT_CONTRACT"], "build.mjs --test builds the Contract compiler and names it"))
        compiler.arguments = ["build", dir.appendingPathComponent("app.contract").path, "-o", dir.appendingPathComponent("app.plan").path]
        try compiler.run(); compiler.waitUntilExit()
        XCTAssertEqual(compiler.terminationStatus, 0)
        return try Data(contentsOf: dir.appendingPathComponent("app.plan"))
    }

    /// What the adapter's window delegate answers for a close (`ExactMac`'s `windowShouldClose`).
    private final class Closer: NSObject, NSWindowDelegate {
        let keepsRunning: Bool, only: Bool
        var closed = false
        init(keepsRunning: Bool, only: Bool) { self.keepsRunning = keepsRunning; self.only = only }
        func windowShouldClose(_ sender: NSWindow) -> Bool { !ExactDocuments.hidesOnClose(sender, only: only, keepsRunning: keepsRunning) }
        func windowWillClose(_ notification: Notification) { closed = true }
    }

    /// LLP 1069.010 D8: `host.macos.window.keepRunning` hides the one window
    /// of a one-window app on close, its session alive, and the
    /// reopen shows it again as it was. Unsaid, the window closes, which
    /// ends the app (`applicationShouldTerminateAfterLastWindowClosed`).
    func testTheOneWindowOfAnAppThatKeepsRunningHidesAndComesBackWithItsSession() throws {
        XCTAssertFalse(ExactDocuments.keepsRunning(), "a test bundle declares none")
        XCTAssertFalse(ExactDocuments.keepsRunning(["ExactWindow": ["width": 900, "height": 640]]), "unsaid, the app ends with its window")
        XCTAssertFalse(ExactDocuments.keepsRunning(["ExactWindow": ["keepRunning": false]]))
        XCTAssertTrue(ExactDocuments.keepsRunning(["ExactWindow": ["keepRunning": true]]))
        XCTAssertTrue(ExactDocuments.keepsRunning(["ExactWindow": ["keepRunning": true], "ExactLaunchMode": "focus-existing"]))
        XCTAssertFalse(ExactDocuments.keepsRunning(["ExactWindow": ["keepRunning": true], "ExactLaunchMode": "navigate-new"]),
                       "a document app's windows close as documents do")
        // AppKit asks whether to end the app when its last window is ordered out, hidden or closed.
        let keeping: [String: Any] = ["ExactWindow": ["keepRunning": true]]
        XCTAssertTrue(ExactDocuments.endsWithoutWindows(hiding: false, [:]), "unsaid, the app ends with its window")
        XCTAssertTrue(ExactDocuments.endsWithoutWindows(hiding: true, [:]))
        XCTAssertFalse(ExactDocuments.endsWithoutWindows(hiding: true, keeping), "a hidden window keeps its app running")
        XCTAssertTrue(ExactDocuments.endsWithoutWindows(hiding: false, keeping), "the app's own close() ends it")
        XCTAssertFalse(ExactDocuments.endsWithoutWindows(hiding: false, ["ExactLaunchMode": "navigate-new"]))

        let session = ExactApp.shared.makeSession(label: "keep-running")
        defer { session.destroy() }
        let shown = window()
        let view = ExactView(session: session)
        shown.contentView = view
        XCTAssertNil(session.boot(plan: try Self.counter.get(), size: CGSize(width: 300, height: 200)).error)
        shown.makeKeyAndOrderFront(nil)
        func count() -> String? { session.presenter.views.values.compactMap { $0.props["text"] }.first { Int($0) != nil } }
        func add() {
            let button = session.presenter.views.values.first { $0.props["testId"] == "add" }
            XCTAssertNil(session.agentInstance.tap(["op": "tap", "id": Int(button?.id ?? 0)])["error"])
        }
        add()
        XCTAssertEqual(count(), "1")
        func visibility() -> String? { session.presenter.views.values.first { $0.props["testId"] == "visibility" }?.props["text"] }
        /// What the page reads once the main queue has told it (`PageFacts.observe`).
        func settled(_ expected: String) -> String? {
            let end = Date().addingTimeInterval(5)
            while visibility() != expected, Date() < end { RunLoop.main.run(mode: .default, before: Date().addingTimeInterval(0.05)) }
            return visibility()
        }
        // On screen the page reads as the window's occlusion says (a test
        // process's window, or any under a locked screen, may read occluded).
        session.tellPage()
        let onScreen = PageFacts.hidden ? "hidden" : "visible"
        XCTAssertEqual(visibility(), onScreen)
        func transition() -> String? { session.presenter.views.values.first { $0.props["testId"] == "transition" }?.props["text"] }
        XCTAssertEqual(transition(), "false 0", "a page launched is not put away and has not come back")

        let keeps = Closer(keepsRunning: true, only: true)
        shown.delegate = keeps
        shown.performClose(nil)
        XCTAssertFalse(shown.isVisible, "closing hides the window")
        XCTAssertFalse(keeps.closed, "and does not close it")
        XCTAssertEqual(session.state, .ready, "its session stays")
        XCTAssertTrue(PageFacts.hidden, "a window its close hid is not seen")
        XCTAssertEqual(settled("hidden"), "hidden", "and its page is told so")
        // Visibility may already read hidden (an occluded window): wait for the close's persisted fact.
        let hid = Date().addingTimeInterval(5)
        while transition() != "true 0", Date() < hid { RunLoop.main.run(mode: .default, before: Date().addingTimeInterval(0.05)) }
        XCTAssertEqual(transition(), "true 0", "put away: pagehide with persisted")
        shown.makeKeyAndOrderFront(nil) // the reopen's `front()`
        XCTAssertTrue(shown.isVisible, "the reopen shows it")
        let shownAgain = shown.occlusionState.contains(.visible) ? "visible" : "hidden"
        XCTAssertEqual(settled(shownAgain), shownAgain, "and its page reads it as its occlusion says again")
        let end = Date().addingTimeInterval(5)
        while transition() != "false 1", Date() < end { RunLoop.main.run(mode: .default, before: Date().addingTimeInterval(0.05)) }
        XCTAssertEqual(transition(), "false 1", "back: pageshow with persisted, counted once")
        XCTAssertTrue(shown.contentView === view, "the same view")
        XCTAssertEqual(count(), "1", "and the same state")
        add()
        XCTAssertEqual(count(), "2", "a live session")

        // With another window open, or with nothing said, the window closes.
        let beside = Closer(keepsRunning: true, only: false)
        shown.delegate = beside
        shown.performClose(nil)
        XCTAssertTrue(beside.closed)
        shown.makeKeyAndOrderFront(nil)
        let plain = Closer(keepsRunning: false, only: true)
        shown.delegate = plain
        shown.performClose(nil)
        XCTAssertTrue(plain.closed, "unsaid, the last window closes and the app ends with it")
        XCTAssertFalse(shown.isVisible)
        shown.delegate = nil
    }
}
#endif
