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
            struct Case: Decodable {
                var name: String
                var state: ActivityJSON
                var budget: Int
            }
            var attributes: ExactActivityAttributes
            var cases: [Case]
        }
        let path = Bundle.module.resourceURL!.appendingPathComponent("SnapshotFixtures/activity-envelope.json")
        let fixture = try JSONDecoder().decode(Fixture.self, from: Data(contentsOf: path))
        for sample in fixture.cases {
            let content = ExactActivityAttributes.ContentState(shape: fixture.attributes.shape, state: sample.state)
            if sample.budget <= 4096 {
                XCTAssertEqual(try fixture.attributes.validate(content), sample.budget, sample.name)
            } else {
                XCTAssertEqual(sample.budget, 4097)
                XCTAssertThrowsError(try fixture.attributes.validate(content)) { error in
                    XCTAssertTrue(String(describing: error).contains("4097 bytes"))
                }
            }
            for formatting: JSONEncoder.OutputFormatting in [[], [.sortedKeys, .withoutEscapingSlashes]] {
                let encoder = JSONEncoder(); encoder.outputFormatting = formatting
                let encoded = try encoder.encode(fixture.attributes).count + encoder.encode(content).count
                XCTAssertLessThanOrEqual(encoded, sample.budget, sample.name)
                if sample.budget == 4097 { XCTAssertEqual(encoded, 4097) }
            }
        }
    }

    func testSharedNumberSpellingsCoverFormatBoundariesAndDoubleExtremes() throws {
        struct Fixture: Decodable {
            var bits: String
            var foundation: String
            var serde: String
            var budget: Int
        }
        let path = Bundle.module.resourceURL!.appendingPathComponent("SnapshotFixtures/activity-numbers.json")
        let fixtures = try JSONDecoder().decode([Fixture].self, from: Data(contentsOf: path))
        let digest = String(repeating: "a", count: 64)
        let attributes = ExactActivityAttributes(kind: "train", generation: digest, shape: digest)
        let overhead = try attributes.validate(.init(shape: digest, state: .null)) - 4
        for sample in fixtures {
            let value = Double(bitPattern: try XCTUnwrap(UInt64(sample.bits, radix: 16)))
            XCTAssertEqual(String(data: try JSONEncoder().encode(value), encoding: .utf8), sample.foundation)
            XCTAssertEqual(try attributes.validate(.init(shape: digest, state: .number(value))), overhead + sample.budget, sample.foundation)
            XCTAssertEqual(sample.budget, max(sample.serde.utf8.count, sample.foundation.utf8.count))
            XCTAssertLessThanOrEqual(sample.budget, 24)
        }
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
