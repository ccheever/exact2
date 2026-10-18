import XCTest
import ExactCore

final class WidgetContainerTests: XCTestCase {
    func testPublicationSkipsUnchangedAssetsAndKeepsPinnedGenerations() throws {
        let root = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
        defer { try? FileManager.default.removeItem(at: root) }
        let names = ["plans/card.plan"]
        var reads = 0
        func assets(_ generation: String) -> AssetResolver {
            AssetResolver(root: root, names: names, generation: generation) { _ in
                reads += 1
                return Data(generation.utf8)
            }
        }
        func publish(_ assets: AssetResolver) throws -> Bool {
            try WidgetContainer.publish(assets, root: root, kinds: ["card"], compatibility: "cohort")
        }
        XCTAssertTrue(try publish(assets("first")))
        let base = root.appendingPathComponent("widget-generations")
        let first = try String(contentsOf: base.appendingPathComponent("current"), encoding: .utf8)
        XCTAssertFalse(try publish(assets("first")))
        XCTAssertEqual(reads, 1, "a new resolver for the same generation reads no assets")
        XCTAssertTrue(try publish(assets("second")))
        XCTAssertEqual(reads, 2)
        XCTAssertEqual(try String(contentsOf: base.appendingPathComponent(first + "/plans/card.plan"), encoding: .utf8), "first")
        XCTAssertNotEqual(try String(contentsOf: base.appendingPathComponent("current"), encoding: .utf8), first)
    }
    func testTypeScriptPublicationRefusesEitherMissingHalf() throws {
        let root = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
        defer { try? FileManager.default.removeItem(at: root) }
        for half in ["app.hbc", "app.module.json"] {
            let assets = AssetResolver(root: root, names: ["plans/card.plan", "typescript/" + half], generation: half) { _ in Data("bytes".utf8) }
            XCTAssertThrowsError(try WidgetContainer.publish(assets, root: root, kinds: ["card"], compatibility: "cohort", typescript: true))
        }
        XCTAssertFalse(FileManager.default.fileExists(atPath: root.appendingPathComponent("widget-generations/current").path))
    }

    private enum RefreshFailure: Error { case offline }

    func testAccountSwitchRefusesLastGoodTimelineUntilTheNewAccountRefreshes() throws {
        let root = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
        defer { try? FileManager.default.removeItem(at: root) }
        func cache(_ kind: String = "question", _ generation: String = "one") throws -> WidgetTimelineCache {
            try WidgetTimelineCache(root: root, kind: kind, generation: generation)
        }
        func offline() throws -> Data { throw RefreshFailure.offline }
        let a = Data(#"[{"date":1,"props":{"question":"Account A"}}]"#.utf8)
        let b = Data(#"[{"date":2,"props":{"question":"Account B"}}]"#.utf8)
        let before = try cache()
        XCTAssertEqual(try before.resolve { a }, a)
        XCTAssertEqual(try before.resolve(offline), a, "a failure without a store write keeps the last good answer")
        XCTAssertEqual(try cache().resolve(offline), a, "a new process restores the same account's answer")
        XCTAssertThrowsError(try cache("another-kind").resolve(offline))
        XCTAssertThrowsError(try cache("question", "another-generation").resolve(offline))

        // The app publishes this counter before the Darwin ping for its store write.
        try Data("1".utf8).write(to: root.appendingPathComponent("widget-store-generation"), options: .atomic)
        let after = try cache()
        XCTAssertNotEqual(before, after, "a new account must not join the old account's in-flight refresh")
        XCTAssertFalse(before.isCurrent, "an in-memory answer from A must not reach a completion after the write")
        XCTAssertThrowsError(try before.resolve(offline))
        XCTAssertThrowsError(try after.resolve(offline), "the provider must take its Open-app path, never return A's entries")
        XCTAssertThrowsError(try cache().resolve(offline), "restarting the extension must not restore A's persisted entries")

        XCTAssertEqual(try after.resolve { b }, b)
        XCTAssertEqual(try after.resolve(offline), b, "the counter does not invalidate a newly written good answer")
        XCTAssertEqual(try cache().resolve(offline), b)
    }

    func testStoreWriteDuringRefreshDiscardsItsAnswer() throws {
        let root = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
        defer { try? FileManager.default.removeItem(at: root) }
        let cache = try WidgetTimelineCache(root: root, kind: "question", generation: "one")
        let a = Data("account A".utf8)
        XCTAssertEqual(try cache.resolve { a }, a)
        XCTAssertThrowsError(try cache.resolve {
            try Data("1".utf8).write(to: root.appendingPathComponent("widget-store-generation"), options: .atomic)
            return a
        }, "a refresh that straddles a store write cannot save or return its old account's answer")
        let next = try WidgetTimelineCache(root: root, kind: "question", generation: "one")
        XCTAssertThrowsError(try next.resolve { throw RefreshFailure.offline })
    }

    func testUnknownStoreCounterAndUnversionedCacheCannotAuthorizeAccountData() throws {
        let root = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
        defer { try? FileManager.default.removeItem(at: root) }
        let file = root.appendingPathComponent("Library/Caches/widget-timelines/one/question.json")
        try FileManager.default.createDirectory(at: file.deletingLastPathComponent(), withIntermediateDirectories: true)
        try Data(#"[{"date":1,"props":{"question":"Account A"}}]"#.utf8).write(to: file)
        let cache = try WidgetTimelineCache(root: root, kind: "question", generation: "one")
        XCTAssertThrowsError(try cache.resolve { throw RefreshFailure.offline }, "an old cache has no proof of its store revision")
        _ = try cache.resolve { Data("account A".utf8) }
        try Data("corrupt".utf8).write(to: root.appendingPathComponent("widget-store-generation"), options: .atomic)
        XCTAssertFalse(cache.isCurrent)
        XCTAssertThrowsError(try cache.resolve { throw RefreshFailure.offline })
        // An unreadable counter still lets a refresh answer from the live
        // store; it keeps nothing and reuses nothing.
        let unknown = try WidgetTimelineCache(root: root, kind: "question", generation: "one")
        XCTAssertThrowsError(try unknown.resolve { throw RefreshFailure.offline })
        try FileManager.default.removeItem(at: file)
        XCTAssertEqual(try unknown.resolve { Data("account B".utf8) }, Data("account B".utf8))
        XCTAssertFalse(FileManager.default.fileExists(atPath: file.path))
    }

    func testAFailedCacheWriteKeepsTheGoodRefreshAndAnOlderEntryIsRemoved() throws {
        let root = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
        defer { try? FileManager.default.removeItem(at: root) }
        let caches = root.appendingPathComponent("Library/Caches/widget-timelines")
        try FileManager.default.createDirectory(at: caches, withIntermediateDirectories: true)
        // A file where the generation's directory goes: the cache cannot be written.
        try Data().write(to: caches.appendingPathComponent("one"))
        let blocked = try WidgetTimelineCache(root: root, kind: "question", generation: "one")
        XCTAssertEqual(try blocked.resolve { Data("account A".utf8) }, Data("account A".utf8))
        try FileManager.default.removeItem(at: caches.appendingPathComponent("one"))
        let file = caches.appendingPathComponent("two/question.json")
        _ = try WidgetTimelineCache(root: root, kind: "question", generation: "two").resolve { Data("account A".utf8) }
        XCTAssertTrue(FileManager.default.fileExists(atPath: file.path))
        try Data("1".utf8).write(to: root.appendingPathComponent("widget-store-generation"), options: .atomic)
        let switched = try WidgetTimelineCache(root: root, kind: "question", generation: "two")
        XCTAssertThrowsError(try switched.resolve { throw RefreshFailure.offline })
        XCTAssertFalse(FileManager.default.fileExists(atPath: file.path), "account A's answer is not kept past the switch")
    }

}
