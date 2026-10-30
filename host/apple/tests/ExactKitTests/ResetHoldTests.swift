#if os(iOS) || os(macOS)
#if os(iOS)
import UIKit
#else
import AppKit
#endif
import XCTest
import ExactCore
@testable import ExactKit

/// A restart's reset holds its runtime only through the synchronous teardown (ResetGate), even with a sheet open (on
/// the Mac, `endSheet`, which takes the sheet's animation) or two windows restarted together (`applyTogether` holds
/// both while it presents each). Work queued before the restart on the main queue, as a common-modes timer and as a
/// common-modes run-loop perform runs only after it, so no main-queue arrival (a store change, a share, a URL:
/// ResetGate.knownLoss) reaches a held runtime. Were the teardown to turn the run loop in the default, modal-panel or
/// event-tracking mode, or drain the main queue, this fails, and knownLoss's losses become real. Each probe must run
/// after the restart, so none passes by never running.
final class ResetHoldTests: XCTestCase {
    private static let probes = ["commonPerform", "commonTimer", "mainQueue"]

    /// Queues the three probes; each records its name and whether a runtime was held when it ran.
    private func probe(_ held: @escaping () -> [Bool]) -> () -> [(String, [Bool])] {
        var ran: [(String, [Bool])] = []
        DispatchQueue.main.async { ran.append(("mainQueue", held())) }
        RunLoop.main.add(Timer(timeInterval: 0.001, repeats: false) { _ in ran.append(("commonTimer", held())) }, forMode: .common)
        RunLoop.main.perform(inModes: [.common]) { ran.append(("commonPerform", held())) }
        return { ran }
    }

    private func spin(_ seconds: Double, until done: () -> Bool) {
        let end = Date().addingTimeInterval(seconds)
        while !done(), Date() < end { RunLoop.main.run(mode: .default, before: Date().addingTimeInterval(0.02)) }
    }

    func testASheetOpenAtARestartTurnsNoRunLoopWhileHeld() throws {
        let plan = try fixturePlan("RESET_HOLD")
        let session = ExactApp.shared.makeSession(label: "reset-hold")
        defer { session.destroy() }
        let size = CGSize(width: 400, height: 600)
        #if os(iOS)
        guard let scene = hostedWindowScene() else { throw missing("scene", "a sheet's presentation completes only in a scene's window") }
        let window = UIWindow(windowScene: scene)
        window.frame = CGRect(origin: .zero, size: size)
        let host = UIViewController()
        window.rootViewController = host
        window.makeKeyAndVisible()
        let view = ExactView(session: session)
        view.frame = host.view.bounds
        host.view.addSubview(view)
        defer { window.isHidden = true }
        #else
        _ = NSApplication.shared
        let window = TimedWindow(contentRect: NSRect(origin: .zero, size: size), styleMask: [.titled], backing: .buffered, defer: false)
        window.isReleasedWhenClosed = false
        window.contentView = ExactView(session: session)
        window.makeKeyAndOrderFront(nil)
        defer { window.close() }
        #endif
        XCTAssertNil(session.boot(plan: plan, size: size).error)
        let open = try XCTUnwrap(session.presenter.views.values.first { $0.props["testId"] as? String == "open" }?.id)
        session.presenter.onPress?(open)
        #if os(iOS)
        let shown = { host.presentedViewController.map { $0.view.window != nil && !$0.isBeingPresented && $0.transitionCoordinator == nil } ?? false }
        spin(5, until: shown)
        XCTAssertTrue(shown(), "the sheet's presentation completed before the restart")
        #else
        spin(5) { window.attachedSheet != nil }
        spin(1) { false } // its presentation settles
        XCTAssertTrue(window.attachedSheet != nil, "the sheet was open at the restart")
        #endif
        let ran = probe { [session.runtime.held] }
        let started = Date()
        XCTAssertTrue(session.apply(plan, label: "generation"))
        let took = Date().timeIntervalSince(started)
        let inside = ran().map(\.0)
        #if os(macOS)
        // Not vacuous: the window was on screen and the teardown ended its sheet, `endSheet` itself taking the animation's time.
        XCTAssertTrue(window.isVisible, "the window is on screen")
        XCTAssertNil(window.attachedSheet, "the restart ended the sheet")
        XCTAssertEqual(window.ends.count, 1, "the restart ended the sheet once")
        XCTAssertGreaterThan(window.ends.first ?? 0, 0.15, "endSheet took the sheet's animation's time")
        #endif
        spin(2) { ran().count == Self.probes.count }
        #if os(iOS)
        spin(3) { host.presentedViewController == nil }
        XCTAssertNil(host.presentedViewController, "the restart dismissed the sheet")
        #endif
        XCTAssertEqual(inside, [], "ran inside the restart's hold (the restart took \(String(format: "%.3f", took)) s)")
        XCTAssertEqual(ran().map(\.0).sorted(), Self.probes, "each probe ran after the restart")
        XCTAssertEqual(ran().filter { $0.1.contains(true) }.map(\.0), [], "a probe saw a held runtime")
    }

    #if os(macOS)
    /// Two windows, each with its sheet open, restarted together: `applyTogether` holds both, then presents each.
    func testTwoWindowsRestartedTogetherTurnNoRunLoopWhileHeld() throws {
        let plan = try fixturePlan("RESET_HOLD")
        let size = CGSize(width: 400, height: 600)
        var sessions: [ExactSession] = [], windows: [TimedWindow] = []
        defer { sessions.forEach { $0.destroy() }; windows.forEach { $0.close() } }
        _ = NSApplication.shared
        for i in 0..<2 {
            let session = ExactApp.shared.makeSession(label: "reset-hold-\(i)")
            let window = TimedWindow(contentRect: NSRect(origin: CGPoint(x: 40 * i, y: 0), size: size), styleMask: [.titled], backing: .buffered, defer: false)
            window.isReleasedWhenClosed = false
            window.contentView = ExactView(session: session)
            window.makeKeyAndOrderFront(nil)
            XCTAssertNil(session.boot(plan: plan, size: size).error)
            let open = try XCTUnwrap(session.presenter.views.values.first { $0.props["testId"] as? String == "open" }?.id)
            session.presenter.onPress?(open)
            spin(5) { window.attachedSheet != nil }
            sessions.append(session); windows.append(window)
        }
        spin(1) { false }
        XCTAssertEqual(windows.map { $0.attachedSheet != nil }, [true, true], "both sheets open at the restart")
        let ran = probe { sessions.map { $0.runtime.held } }
        XCTAssertTrue(ExactApp.shared.apply(plan, label: "generation"))
        let inside = ran().map(\.0)
        XCTAssertEqual(windows.map(\.isVisible), [true, true], "both windows on screen")
        XCTAssertEqual(windows.map { $0.attachedSheet == nil }, [true, true], "the restart ended both sheets")
        XCTAssertEqual(windows.map(\.ends.count), [1, 1], "the restart ended each sheet once")
        XCTAssertEqual(windows.map { ($0.ends.first ?? 0) > 0.15 }, [true, true], "each endSheet took the sheet's animation's time: \(windows.map(\.ends))")
        spin(2) { ran().count == Self.probes.count }
        XCTAssertEqual(inside, [], "ran inside the restart's hold")
        XCTAssertEqual(ran().map(\.0).sorted(), Self.probes, "each probe ran after the restart")
        XCTAssertEqual(ran().filter { $0.1.contains(true) }.map(\.0), [], "a probe saw a held runtime")
    }
    #endif
}

#if os(macOS)
/// Times each `endSheet` it's asked for, so the test knows the sheet's end really animated.
private final class TimedWindow: NSWindow {
    var ends: [TimeInterval] = []

    override func endSheet(_ sheet: NSWindow) {
        let started = Date()
        super.endSheet(sheet)
        ends.append(Date().timeIntervalSince(started))
    }
}
#endif
#endif
