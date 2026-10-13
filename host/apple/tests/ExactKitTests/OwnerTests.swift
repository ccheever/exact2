import Foundation
import XCTest
#if os(macOS)
import AppKit
#endif
@testable import ExactKit

// @ref LLP 1072 T1/T5 — the owner thread's contract.
final class OwnerTests: XCTestCase {
    func testDocumentOwnerIsTheRuntimeThatResolvesStorage() {
        let session = ExactApp.shared.makeSession(label: "document-owner")
        defer { session.destroy() }
        XCTAssertEqual(session.documentOwner, UInt64(session.runtime.rt))
    }

    func testSyncRunsOnTheOwnerInOrder() {
        var seen: [Int] = []
        for i in 0..<100 {
            let onOwner = Owner.shared.sync { () -> Bool in seen.append(i); return Owner.shared.isOwner }
            XCTAssertTrue(onOwner)
        }
        XCTAssertEqual(seen, Array(0..<100))
        XCTAssertFalse(Owner.shared.isOwner)
    }

    func testNestedSyncOnTheOwnerRunsInline() {
        let value = Owner.shared.sync { Owner.shared.sync { 7 } + 1 }
        XCTAssertEqual(value, 8)
    }

    func testCallMainFromTheOwnerIsServedWhileMainWaits() {
        let result = Owner.shared.sync { () -> (Bool, Int) in
            let answer = Owner.shared.callMain { () -> (Bool, Int) in (Thread.isMainThread, 41) }
            return (answer.0, answer.1 + 1)
        }
        XCTAssertTrue(result.0)
        XCTAssertEqual(result.1, 42)
    }

    func testARuntimeCallFromAServedCallbackIsRefusedNotDeadlocked() {
        let refused = Owner.shared.sync { () -> Int in
            Owner.shared.callMain { Owner.shared.sync({ 1 }, busy: -1) }
        }
        XCTAssertEqual(refused, -1)
    }

    func testANotificationFromAServedCallbackRunsAfterTheCurrentJob() {
        var order: [String] = []
        Owner.shared.sync {
            Owner.shared.callMain {
                Owner.shared.syncOrLater { order.append("later") }
                order.append("served")
            }
            order.append("job")
        }
        // The deferred job runs after the one that asked; a later sync is
        // queued behind it.
        Owner.shared.sync { order.append("next") }
        XCTAssertEqual(order, ["served", "job", "later", "next"])
    }

    func testCallMainFromAnAsynchronousOwnerJobIsServedByTheMainQueue() {
        let done = expectation(description: "served")
        Owner.shared.sync {
            Owner.shared.callMain {
                Owner.shared.syncOrLater {
                    let onMain = Owner.shared.callMain { Thread.isMainThread }
                    XCTAssertTrue(onMain)
                    DispatchQueue.main.async { done.fulfill() }
                }
            }
        }
        wait(for: [done], timeout: 5)
    }

    #if os(macOS)
    func testViewResizeFromAServedCallbackReachesTheRootAfterTheCallback() throws {
        let dir = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
        try FileManager.default.createDirectory(at: dir, withIntermediateDirectories: true)
        defer { try? FileManager.default.removeItem(at: dir) }
        let source = dir.appendingPathComponent("app.contract"), plan = dir.appendingPathComponent("app.plan")
        try "component App\n  view\n    main testId=\"resize-root\" width=\"100%\" height=\"100%\"\n".write(to: source, atomically: true, encoding: .utf8)
        let compiler = Process()
        compiler.executableURL = URL(fileURLWithPath: try XCTUnwrap(ProcessInfo.processInfo.environment["EXACT_CONTRACT"]))
        compiler.arguments = ["build", source.path, "-o", plan.path]
        try compiler.run(); compiler.waitUntilExit()
        XCTAssertEqual(compiler.terminationStatus, 0)
        let session = ExactApp.shared.makeSession(label: "callback-resize")
        defer { session.destroy() }
        XCTAssertNil(session.boot(plan: try Data(contentsOf: plan), size: CGSize(width: 1024, height: 999)).error)
        let view = ExactView(session: session)
        view.setFrameSize(CGSize(width: 1024, height: 999))
        view.layout()
        let root = try XCTUnwrap(session.presenter.views.values.first { $0.props["testId"] == "resize-root" })
        XCTAssertEqual(root.frame.height, 999)
        Owner.shared.sync {
            Owner.shared.callMain {
                view.setFrameSize(CGSize(width: 1024, height: 1200))
                view.layout()
                view.setFrameSize(CGSize(width: 1024, height: 1366))
                view.layout()
            }
        }
        let restored = expectation(description: "latest resize applied after native callback")
        DispatchQueue.main.async {
            XCTAssertEqual(session.presenter.viewportSize.height, 1366)
            XCTAssertEqual(root.frame.height, 1366)
            // A later same-size pass must also keep the restored frame.
            view.layout()
            XCTAssertEqual(root.frame.height, 1366)
            Owner.shared.sync {
                Owner.shared.callMain {
                    view.setFrameSize(CGSize(width: 1024, height: 1400))
                    view.layout()
                }
            }
            session.destroy()
            let retiredBatches = session.appliedBatches
            DispatchQueue.main.async {
                XCTAssertEqual(session.appliedBatches, retiredBatches, "queued layout must not call a destroyed runtime")
                restored.fulfill()
            }
        }
        wait(for: [restored], timeout: 5)
    }
    #endif
}
