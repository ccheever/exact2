#if os(iOS)
import UIKit
import XCTest
@testable import ExactKit

/// A paragraph that cannot raster (`line-clamp`) has only `draw(_:)`'s
/// bitmap. A list builds its lead rows below the scrollport, where that draw
/// runs first, and nothing redisplays the row when it scrolls in: the draw
/// must paint the text visible or not. UIKit, so a simulator runs it:
///   bun host/apple/build.mjs --test --ios
final class TextPaintIOSTests: XCTestCase {
    private var window: UIWindow!

    private func collections(_ rows: [Int]) -> [String: Any] {
        ["op": "collections", "items": [["view": 1, "revision": 1, "scrollSequence": 0, "count": 50, "totalExtent": 2000,
            "rows": rows.map { ["view": $0, "root": $0, "epoch": 1] }, "correction": NSNull()]]]
    }
    /// A row (`base`) holding one clamped paragraph (`base + 1`), at `y`.
    private func rowOps(_ base: Int, y: Double, text: String) -> [[String: Any]] {
        [
            ["op": "create", "id": base, "kind": "view", "props": ["testId": "row-\(base)"]],
            ["op": "create", "id": base + 1, "kind": "text", "props": ["text": text],
             "style": ["font_size": 16.0, "line_clamp": 2.0, "text_overflow": "ellipsis"]],
            ["op": "children", "id": base, "ids": [base + 1]],
            ["op": "frame", "id": base, "x": 0.0, "y": y, "w": 300.0, "h": 60.0],
            ["op": "frame", "id": base + 1, "x": 10.0, "y": 10.0, "w": 280.0, "h": 40.0],
        ]
    }

    private func inkPixels(_ node: NodeView) -> Int {
        let format = UIGraphicsImageRendererFormat(); format.scale = 1; format.opaque = false
        let image = UIGraphicsImageRenderer(bounds: node.bounds, format: format).image { _ in node.draw(node.bounds) }
        guard let cg = image.cgImage, let data = cg.dataProvider?.data, let bytes = CFDataGetBytePtr(data) else { return 0 }
        var count = 0
        for y in 0..<cg.height { for x in 0..<cg.width where bytes[y * cg.bytesPerRow + x * 4 + 3] > 0 { count += 1 } }
        return count
    }

    func testAClampedParagraphInAReusedRowBuiltBelowTheScrollportPaints() throws {
        let session = ExactApp.shared.makeSession(label: "clamp-reuse")
        defer { session.destroy() }
        let p = session.presenter
        window = UIWindow(frame: CGRect(x: 0, y: 0, width: 400, height: 400))
        p.viewport.frame = window.bounds
        window.addSubview(p.viewport)
        window.makeKeyAndVisible()
        p.apply(wireBatch([collections([10]), ["op": "create", "id": 1, "kind": "list", "style": ["overflow_y": "scroll"]]]
            + rowOps(10, y: 0, text: "Maybe family sounds draft later scroll deadline picnic thanks soon a meeting")
            + [["op": "children", "id": 1, "ids": [10]], ["op": "roots", "ids": [1]],
               ["op": "frame", "id": 1, "x": 0.0, "y": 0.0, "w": 300.0, "h": 300.0],
               ["op": "content", "id": 1, "w": 300.0, "h": 2000.0]]))
        let first = try XCTUnwrap(p.views[11])

        // The row retires; the next of its shape is built in the lead, below the scrollport.
        p.apply(wireBatch([collections([20])] + [10, 11].map { ["op": "destroy", "id": $0] }
            + rowOps(20, y: 1000, text: "A sure family meeting schedule text layout definitely thanks")
            + [["op": "children", "id": 1, "ids": [20]]]))
        let text = try XCTUnwrap(p.views[21])
        XCTAssertTrue(text === first, "the pool lent the retired paragraph's view")
        XCTAssertNotNil(text.window)
        XCTAssertFalse(text.canRasterText, "a clamped paragraph draws")
        XCTAssertTrue(text.drawsPaint)
        XCTAssertFalse(p.textIsVisible(text), "drawn while below the scrollport")
        XCTAssertGreaterThan(inkPixels(text), 50, "the bitmap it keeps when it scrolls in has the text")
    }
}
#endif
