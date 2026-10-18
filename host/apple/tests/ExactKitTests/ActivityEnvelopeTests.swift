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
        XCTAssertEqual(try attributes.validate(state), try encoder.encode(attributes).count + encoder.encode(state).count)
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

    func testLastDecodableStateIsPerActivityAndGeneration() throws {
        let root = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
        defer { try? FileManager.default.removeItem(at: root) }
        func cache(_ id: String = "first", _ generation: String = "one", _ shape: String = "shape") -> ActivityStateCache {
            ActivityStateCache(root: root, id: id, generation: generation, shape: shape)
        }
        let good = Data(#"{"title":"Next train"}"#.utf8), bad = Data("unknown shape".utf8)
        let accepts = { (bytes: Data) in bytes == good }
        XCTAssertNil(cache().resolve(bad, accepts: accepts).state)
        XCTAssertFalse(cache().resolve(good, accepts: accepts).outdated)
        let kept = cache().resolve(bad, accepts: accepts)
        XCTAssertEqual(kept.state, good)
        XCTAssertTrue(kept.outdated)
        XCTAssertEqual(cache().resolve(nil, accepts: accepts).state, good)
        XCTAssertNil(cache("second").resolve(bad, accepts: accepts).state)
        XCTAssertNil(cache("first", "two").resolve(bad, accepts: accepts).state)
        XCTAssertNil(cache("first", "one", "changed").resolve(bad, accepts: accepts).state)
        XCTAssertFalse(cache().resolve(good, accepts: accepts).outdated)
    }
}
