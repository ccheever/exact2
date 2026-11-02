import XCTest
@testable import ExactKit

final class RelaunchTests: XCTestCase {
    #if os(macOS)
    func testRelaunchFromBeforeUnloadJoinsTheQuitHold() {
        var pending = true, terminations = 0
        var quitting: QuitHold?
        let drained = expectation(description: "the original quit completes after storage drains")
        let hold = QuitHold(pending: { pending }, end: {
            XCTAssertFalse(pending)
            terminations += 1
            drained.fulfill()
        })
        // beforeunload requests relaunch before the original quit takes its hold.
        ProcessRelaunch.requestQuit(holding: { quitting?.holding == true }) {
            terminations += 1
            XCTFail("a second terminate can bypass AppKit's outstanding storage hold")
        }
        XCTAssertTrue(hold.hold())
        quitting = hold
        RunLoop.main.perform(inModes: [.common]) {
            XCTAssertEqual(terminations, 0, "the queued relaunch must leave the original quit in charge")
            pending = false
            hold.check()
        }
        wait(for: [drained], timeout: 2)
        XCTAssertEqual(terminations, 1)
    }

    func testRelaunchRequestsQuitWithoutAQuitHold() {
        let terminated = expectation(description: "relaunch requests the normal quit")
        ProcessRelaunch.requestQuit(holding: { false }) { terminated.fulfill() }
        wait(for: [terminated], timeout: 2)
    }
    #endif

    final class Delegate: ExactSessionDelegate {
        var commands: [String] = []
        func exactSession(_ session: ExactSession, command name: String, args: [Any]) {
            commands.append(name)
        }
    }

    func testRelaunchDispatchOrRefusal() {
        let session = ExactApp.shared.makeSession(label: "relaunch")
        defer { session.destroy() }
        XCTAssertNil(session.boot(size: CGSize(width: 390, height: 844)).error)
        let delegate = Delegate()
        session.delegate = delegate
        session.relaunch()
        #if os(macOS)
        if ExactEnv.environment["EXACT_AGENT"] == nil {
            XCTAssertEqual(delegate.commands, ["relaunch"])
        } else {
            XCTAssertTrue(delegate.commands.isEmpty)
            XCTAssertTrue(session.agent("{\"op\":\"logs\"}").contains("the agent owns this process"))
        }
        #else
        XCTAssertTrue(delegate.commands.isEmpty)
        XCTAssertTrue(session.agent("{\"op\":\"logs\"}").contains("does not allow an app to restart its process"))
        #endif
    }
}
