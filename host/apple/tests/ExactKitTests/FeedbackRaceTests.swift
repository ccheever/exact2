#if os(iOS) || os(macOS)
#if os(iOS)
import UIKit
#else
import AppKit
#endif
import XCTest
import ExactCore
@testable import ExactKit

/// A list's report that reaches the runner after the runner has moved its snapshot on (a commit from elsewhere: a
/// reply, a timer, an action, another report) is turned away by the runner's own revision check, silently (runner
/// `prepare_feedback`: a revision not its current one); the clone's feed sees one about every 2 s. The host must not
/// lose what that report carried: the rows it measured are measured in the runner once the newer snapshot lands.
/// Why the race is safe: every report sends again what the runner's snapshot hasn't acknowledged
/// (`CollectionHost.flush`), so a report accepted after a turned-away one carries its measurements; and a newer
/// snapshot that reissues the rows (new epochs, as here) is measured afresh. The one window left, a newer snapshot
/// that keeps the rows at the same epochs, their sizes unacknowledged, with nothing else moved (two host reports at
/// one revision, the first accepted and the second turned away), is closed by `CollectionHost.Entry.owed`
/// (CollectionMacTests' unit test builds it directly).
///   bun host/apple/build.mjs --test --ios   (and --test for the Mac)
final class FeedbackRaceTests: XCTestCase {
    private var session: ExactSession?
    #if os(iOS)
    private var window: UIWindow?
    #else
    private var window: NSWindow?
    #endif

    override func tearDown() {
        session?.destroy()
        session = nil
        #if os(iOS)
        window?.isHidden = true
        #else
        window?.close()
        #endif
        window = nil
        super.tearDown()
    }

    private func until(_ what: String, _ seconds: Double = 10, _ done: () -> Bool) {
        let deadline = Date().addingTimeInterval(seconds)
        while !done(), Date() < deadline { RunLoop.main.run(mode: .default, before: Date().addingTimeInterval(0.02)) }
        XCTAssertTrue(done(), what)
    }

    func testARowsMeasurementInAReportTheRunnerTurnsAwayReachesItOnceTheNewerSnapshotLands() throws {
        let plan = try fixturePlan("FEEDBACK_RACE")
        let session = ExactApp.shared.makeSession(label: "feedback-race")
        self.session = session
        let size = CGSize(width: 400, height: 500)
        #if os(iOS)
        let window = UIWindow(frame: CGRect(origin: .zero, size: size))
        let host = UIViewController()
        window.rootViewController = host
        window.makeKeyAndVisible()
        let view = ExactView(session: session)
        view.frame = host.view.bounds
        host.view.addSubview(view)
        #else
        _ = NSApplication.shared
        let window = NSWindow(contentRect: NSRect(origin: .zero, size: size), styleMask: [.titled], backing: .buffered, defer: false)
        window.isReleasedWhenClosed = false
        window.contentView = ExactView(session: session)
        #endif
        self.window = window
        let p = session.presenter
        // Every report the host sends, held: the test delivers each to the real runtime itself, in the race's order.
        let send = try XCTUnwrap(p.collections.onFeedback)
        var held: [Data] = []
        p.collections.onFeedback = { held.append($0) }
        XCTAssertNil(session.boot(plan: plan, size: size).error)
        let feed = try XCTUnwrap(p.views.values.first { $0.props["testId"] == "feed" }).id
        until("the host builds the rows it shows and reports them") { !held.isEmpty && !(p.collections.entries[feed]?.snapshot.rows.isEmpty ?? true) }
        let entry = try XCTUnwrap(p.collections.entries[feed])
        let revision = entry.snapshot.revision
        // The rows the host has a view for, which its report measures (`CollectionHost.flush`).
        let measured = Set(entry.snapshot.rows.filter { p.views[$0.view] != nil }.map(\.view))
        XCTAssertFalse(measured.isEmpty)
        XCTAssertTrue(entry.snapshot.rows.filter { measured.contains($0.view) }.allSatisfy { !$0.measured }, "the runner has measured none of them yet")
        // The race: the runner commits from elsewhere (a row appended past the port), its new snapshot not yet at the
        // host, and only then the host's report reaches it, at the old revision: turned away.
        let more = try XCTUnwrap(p.views.values.first { $0.props["testId"] == "more" }).id
        let elsewhere = session.runtime.press(more, now: session.now())
        XCTAssertNil(elsewhere.error)
        for report in held {
            let answer = session.runtime.collectionFeedback(report, now: session.now())
            XCTAssertNil(answer.error)
            XCTAssertFalse(answer.ops.contains { $0.op == .collections }, "a report at revision \(revision) is turned away once the runner has moved on")
        }
        held = []
        // The newer snapshot lands; from here the host's reports go to the runtime as they do in an app.
        p.collections.onFeedback = send
        session.apply(elsewhere)
        XCTAssertGreaterThan(p.collections.entries[feed]?.snapshot.revision ?? 0, revision)
        until("each row the turned-away report measured, still in the snapshot, is measured in the runner") {
            let rows = p.collections.entries[feed]?.snapshot.rows ?? []
            let kept = rows.filter { measured.contains($0.view) }
            return !kept.isEmpty && kept.allSatisfy(\.measured)
        }
    }
}
#endif
