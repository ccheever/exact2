import XCTest
@testable import ExactKit

/// LLP 1072 T1/T2: every runtime lives in the owner thread's registry, so a
/// call made from main must be an owner job. The paired transform packet
/// was not: from main it found no runtime at all, and every transform drag
/// (a photo viewer's pan and pinch) was refused at its first geometry
/// report. Whatever the runtime answers a packet now, it is that runtime's
/// answer, not the registry's refusal.
final class TransformMotionOwnerTests: XCTestCase {
    func testAPacketReachesItsRuntimeFromMain() throws {
        let runtime = Runtime()
        defer { runtime.destroy() }
        let packet = TransformDragPacket(op: 10, runtime: 1, handleKey: 1, targetKey: 2, clipKey: 3, sequence: 1,
                                         values: [100, 100, 100, 100, 0, 0], now: 0)
        let reply = try XCTUnwrap(runtime.transformMotion(packet), "a reply")
        XCTAssertFalse(reply.batch.error?.contains("no such runtime") ?? false, reply.batch.error ?? "")
    }
}
