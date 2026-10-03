import XCTest
@testable import ExactKit

// postMessage parity (final review item 3): a post with no live canvas of its
// surface name waits, at most Canvases.postBound per name, as on web and Linux.
final class PostMessageTests: XCTestCase {
    func testPostsWaitForTheirSurfaceUpToTheBound() {
        let canvases = Canvases()
        for _ in 0..<Canvases.postBound { canvases.post("world", "buy") }
        canvases.post("world", "dropped")
        canvases.post("other", "kept")
        XCTAssertEqual(canvases.pendingPosts.filter { $0.name == "world" }.count, Canvases.postBound)
        XCTAssertFalse(canvases.pendingPosts.contains { $0.text == "dropped" })
        canvases.deliverPosts()
        XCTAssertEqual(canvases.pendingPosts.count, Canvases.postBound + 1, "no live canvas: still held, in order")
        XCTAssertEqual(canvases.pendingPosts.last?.text, "kept")
    }
}
