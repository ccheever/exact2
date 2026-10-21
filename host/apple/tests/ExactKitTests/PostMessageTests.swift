import XCTest
@testable import ExactKit
@testable import ExactSurfaces

// postMessage parity (final review item 3): a post with no live canvas of its
// surface name waits, at most CanvasesHost.postBound per name, as on web and Linux.
final class PostMessageTests: XCTestCase {
    override class func setUp() { super.setUp(); ExactSurfaces.install() } // LLP 1047.001 D4
    func testPostsWaitForTheirSurfaceUpToTheBound() {
        let canvases = CanvasesHost()
        for _ in 0..<CanvasesHost.postBound { canvases.post("world", "buy") }
        canvases.post("world", "dropped")
        canvases.post("other", "kept")
        XCTAssertEqual(canvases.pendingPosts.filter { $0.name == "world" }.count, CanvasesHost.postBound)
        XCTAssertFalse(canvases.pendingPosts.contains { $0.text == "dropped" })
        canvases.deliverPosts()
        XCTAssertEqual(canvases.pendingPosts.count, CanvasesHost.postBound + 1, "no live canvas: still held, in order")
        XCTAssertEqual(canvases.pendingPosts.last?.text, "kept")
    }
}
