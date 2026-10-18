#if canImport(WidgetKit)
import XCTest
import WidgetKit
import ExactCore
import ExactSnapshot

final class WidgetTimelineWaitersTests: XCTestCase {
    struct Entry: TimelineEntry {
        let date: Date
        var size: CGSize
    }
    func testInlineAndRectangularCoalesceWithoutSharingTheirSize() throws {
        let root = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
        defer { try? FileManager.default.removeItem(at: root) }
        let cache = try WidgetTimelineCache(root: root, kind: "trains", generation: "one")
        let waiters = WidgetTimelineWaiters<Entry> { entry, size in
            var entry = entry; entry.size = size; return entry
        }
        let inline = CGSize(width: 13.5, height: 13.5), rectangular = CGSize(width: 184, height: 69.5)
        for sizes in [[inline, rectangular], [rectangular, inline]] {
            var answers: [CGSize] = []
            for (index, size) in sizes.enumerated() {
                XCTAssertEqual(waiters.begin(cache, size: size) { answers.append($0.entries[0].size) }, index == 0)
            }
            let answer = Timeline(entries: [Entry(date: .now, size: sizes[0])], policy: .never)
            waiters.finish(cache, answer, fallback: answer)
            XCTAssertEqual(answers, sizes)
        }
    }
}
#endif
