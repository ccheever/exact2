#if os(macOS)
import AppKit
import XCTest
@testable import ExactKit

// Existing ExactKitTests target. Real AppKit post/nextEvent calls, no windows.
@MainActor
final class AgentMouseQueueTests: XCTestCase {
    private var app: NSApplication { NSApplication.shared }
    private func event(number: Int, window: Int = 0, time: TimeInterval = 1234.567891234567,
                       type: NSEvent.EventType = .leftMouseUp) throws -> NSEvent {
        try XCTUnwrap(NSEvent.mouseEvent(with: type, location: NSPoint(x: 216, y: 59.5),
            modifierFlags: [], timestamp: time, windowNumber: window, context: nil,
            eventNumber: number, clickCount: 1, pressure: type == .leftMouseUp ? 0 : 1))
    }
    private func owned() throws -> (NSEvent, AgentMouseRelease) {
        let up = try event(number: AgentMouseRelease.nextEventNumber())
        return (up, try XCTUnwrap(AgentMouseRelease(up)))
    }
    private func peek() -> NSEvent? {
        app.nextEvent(matching: .leftMouseUp, until: .distantPast,
                      inMode: .default, dequeue: false)
    }

    func testRealQueueWrappersStillIdentifyAndRemoveExactlyOneOwnedRelease() throws {
        let (up, owner) = try owned()
        app.postEvent(up, atStart: true)
        defer { _ = owner.takeQueued(from: app) }
        let pending = try XCTUnwrap(peek())
        XCTAssertTrue(owner.matches(pending))
        XCTAssertEqual(pending.eventNumber, up.eventNumber)
        XCTAssertEqual(pending.windowNumber, up.windowNumber)
        XCTAssertEqual(pending.cgEvent?.timestamp, up.cgEvent?.timestamp)
        // Object identity and Double timestamp equality are deliberately not prerequisites.
        let delivered = try XCTUnwrap(owner.takeQueued(from: app))
        XCTAssertTrue(owner.matches(delivered))
        XCTAssertNil(owner.takeQueued(from: app), "a second UP must not be fabricated")
    }

    func testTrackingModeConsumptionDoesNotProduceASecondRelease() throws {
        let (up, owner) = try owned()
        app.postEvent(up, atStart: true)
        defer { _ = owner.takeQueued(from: app) }
        // The real queue is consumed as a synchronous tracking loop consumes it.
        // This is not a claim that a real NSTextView/AVKit tracking loop was entered.
        let consumed = try XCTUnwrap(app.nextEvent(matching: .leftMouseUp,
            until: .distantPast, inMode: .eventTracking, dequeue: true))
        XCTAssertTrue(owner.matches(consumed))
        XCTAssertNil(owner.takeQueued(from: app))
    }

    func testForeignFrontReleaseIsNeitherTakenNorDispatched() throws {
        let (up, owner) = try owned()
        let (foreign, foreignOwner) = try owned()
        app.postEvent(up, atStart: true)
        app.postEvent(foreign, atStart: true)
        defer { _ = foreignOwner.takeQueued(from: app); _ = owner.takeQueued(from: app) }
        XCTAssertNil(owner.takeQueued(from: app))
        XCTAssertTrue(foreignOwner.matches(try XCTUnwrap(peek())))
        XCTAssertNotNil(foreignOwner.takeQueued(from: app))
        XCTAssertNotNil(owner.takeQueued(from: app))
        XCTAssertNil(owner.takeQueued(from: app))
    }

    func testTrackingConsumedOwnedUpLeavesFollowingForeignUpUntouched() throws {
        let (up, owner) = try owned()
        let (foreign, foreignOwner) = try owned()
        app.postEvent(foreign, atStart: true)
        app.postEvent(up, atStart: true)
        defer { _ = owner.takeQueued(from: app); _ = foreignOwner.takeQueued(from: app) }
        let consumed = try XCTUnwrap(app.nextEvent(matching: .leftMouseUp,
            until: .distantPast, inMode: .eventTracking, dequeue: true))
        XCTAssertTrue(owner.matches(consumed))
        XCTAssertNil(owner.takeQueued(from: app))
        XCTAssertTrue(foreignOwner.matches(try XCTUnwrap(peek())))
        XCTAssertNotNil(foreignOwner.takeQueued(from: app))
    }

    func testOwnershipRequiresTypeNumberWindowAndCGTimestamp() throws {
        let (up, owner) = try owned()
        XCTAssertTrue(owner.matches(try event(number: up.eventNumber)))
        XCTAssertFalse(owner.matches(try event(number: 0)))
        XCTAssertFalse(owner.matches(try event(number: AgentMouseRelease.nextEventNumber())))
        XCTAssertFalse(owner.matches(try event(number: up.eventNumber, window: 1)))
        XCTAssertFalse(owner.matches(try event(number: up.eventNumber, time: 1235.567891234567)))
        XCTAssertFalse(owner.matches(try event(number: up.eventNumber, type: .leftMouseDown)))
        XCTAssertNil(AgentMouseRelease(try event(number: 0)))
    }

    func testSequentialOwnedClicksCannotTakeEachOthersRelease() throws {
        let (first, firstOwner) = try owned()
        let (second, secondOwner) = try owned()
        XCTAssertLessThan(first.eventNumber, 0)
        XCTAssertLessThan(second.eventNumber, 0)
        XCTAssertNotEqual(first.eventNumber, second.eventNumber)
        app.postEvent(first, atStart: true)
        app.postEvent(second, atStart: true)
        defer { _ = secondOwner.takeQueued(from: app); _ = firstOwner.takeQueued(from: app) }
        XCTAssertNil(firstOwner.takeQueued(from: app))
        XCTAssertNotNil(secondOwner.takeQueued(from: app))
        XCTAssertNotNil(firstOwner.takeQueued(from: app))
        XCTAssertNil(secondOwner.takeQueued(from: app))
        XCTAssertNil(firstOwner.takeQueued(from: app))
    }
}
#endif
