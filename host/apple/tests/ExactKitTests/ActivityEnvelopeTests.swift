import XCTest
import ExactCore
import ExactSnapshot

final class ActivityEnvelopeTests: XCTestCase {
    func testUniversalEnvelopeRoundTripsAndCountsEscapedUTF8() throws {
        let digest = String(repeating: "a", count: 64)
        let attributes = ExactActivityAttributes(kind: "train", generation: digest, shape: digest)
        let state = ExactActivityAttributes.ContentState(shape: digest, state: .object([
            "title": .string("🚆\"\\"), "progress": .number(0.5), "done": .bool(false),
            "items": .array([.null, .number(101)])
        ]))
        let encoded = try JSONEncoder().encode(state)
        XCTAssertEqual(try JSONDecoder().decode(ExactActivityAttributes.ContentState.self, from: encoded), state)
        let encoder = JSONEncoder()
        encoder.outputFormatting = [.sortedKeys, .withoutEscapingSlashes]
        XCTAssertGreaterThanOrEqual(try attributes.validate(state), try encoder.encode(attributes).count + encoder.encode(state).count)
        let empty = ExactActivityAttributes.ContentState(shape: digest, state: .object(["title": .string("")]))
        let overhead = try attributes.validate(empty)
        let boundary = ExactActivityAttributes.ContentState(shape: digest, state: .object(["title": .string(String(repeating: "x", count: 4096 - overhead))]))
        XCTAssertEqual(try attributes.validate(boundary), 4096)
        var large = boundary
        large.state = .object(["title": .string(String(repeating: "x", count: 4097 - overhead))])
        XCTAssertThrowsError(try attributes.validate(large)) { error in
            XCTAssertTrue(String(describing: error).contains("4097 bytes"))
        }
        var unknown = state; unknown.shape = String(repeating: "b", count: 64)
        XCTAssertThrowsError(try attributes.validate(unknown))
    }

    func testBothEncodersUseTheSharedEnvelopeBudget() throws {
        struct Fixture: Decodable {
            var attributes: ExactActivityAttributes
            var content: ExactActivityAttributes.ContentState
            var budget: Int
        }
        let path = Bundle.module.resourceURL!.appendingPathComponent("SnapshotFixtures/activity-envelope.json")
        let fixture = try JSONDecoder().decode(Fixture.self, from: Data(contentsOf: path))
        XCTAssertEqual(try fixture.attributes.validate(fixture.content), fixture.budget)
        for formatting: JSONEncoder.OutputFormatting in [[], [.sortedKeys, .withoutEscapingSlashes]] {
            let encoder = JSONEncoder(); encoder.outputFormatting = formatting
            let encoded = try encoder.encode(fixture.attributes).count + encoder.encode(fixture.content).count
            XCTAssertLessThanOrEqual(encoded, fixture.budget)
        }
        var boundary = fixture.content
        guard case var .object(state) = boundary.state else { return XCTFail("fixture state must be an object") }
        state["padding"] = .string(String(repeating: "x", count: 4096 - fixture.budget))
        boundary.state = .object(state)
        XCTAssertEqual(try fixture.attributes.validate(boundary), 4096)
        state["padding"] = .string(String(repeating: "x", count: 4097 - fixture.budget))
        boundary.state = .object(state)
        XCTAssertThrowsError(try fixture.attributes.validate(boundary))
    }

    func testLastDecodableStateIsPerActivityAndReportsFailedWrites() throws {
        let root = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
        defer { try? FileManager.default.removeItem(at: root) }
        func cache(_ id: String = "first") -> ActivityStateCache { ActivityStateCache(root: root, id: id) }
        let good = Data(#"{"title":"Next train"}"#.utf8), bad = Data("unknown shape".utf8)
        let accepts = { (bytes: Data) in bytes == good }
        let empty = try cache().resolve(bad, accepts: accepts)
        XCTAssertNil(empty.state)
        XCTAssertFalse(empty.outdated)
        XCTAssertFalse(try cache().resolve(good, accepts: accepts).outdated)
        let kept = try cache().resolve(bad, accepts: accepts)
        XCTAssertEqual(kept.state, good)
        XCTAssertTrue(kept.outdated)
        XCTAssertEqual(try cache().resolve(nil, accepts: accepts).state, good)
        XCTAssertNil(try cache("second").resolve(bad, accepts: accepts).state)
        let incompatible = try cache().resolve(bad, accepts: { _ in false })
        XCTAssertNil(incompatible.state)
        XCTAssertFalse(incompatible.outdated)
        XCTAssertFalse(try cache().resolve(good, accepts: accepts).outdated)
        let blocked = root.appendingPathComponent("blocked")
        try Data().write(to: blocked)
        XCTAssertThrowsError(try ActivityStateCache(root: blocked, id: "first").resolve(good, accepts: accepts))
    }
}
