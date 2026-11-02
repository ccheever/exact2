import XCTest
import CoreGraphics
@testable import ExactKit
#if os(macOS)
import AppKit
#else
import UIKit
#endif

/// `background-clip: text` paints the background through glyph outlines.
/// A hidden element paints none of its own, and a hidden run punches no hole.
final class BackgroundClipTextIOSTests: XCTestCase {
    private var session: ExactSession?
    override func tearDown() { session?.destroy(); session = nil; super.tearDown() }

    private func redCount(_ node: NodeView, _ paint: (CGContext) -> Void) -> Int {
        let w = max(1, Int(node.bounds.width)), h = max(1, Int(node.bounds.height))
        guard let ctx = CGContext(data: nil, width: w, height: h, bitsPerComponent: 8, bytesPerRow: w * 4,
                                  space: CGColorSpaceCreateDeviceRGB(),
                                  bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue) else { return 0 }
        paint(ctx)
        guard let raw = ctx.data else { return 0 }
        let bytes = raw.bindMemory(to: UInt8.self, capacity: w * h * 4)
        var count = 0
        for i in stride(from: 0, to: w * h * 4, by: 4) where bytes[i] > 180 && bytes[i + 1] < 60 && bytes[i + 2] < 60 && bytes[i + 3] > 180 {
            count += 1
        }
        return count
    }

    /// `op` is first, as the host writes it, so a run's `visibility` has to
    /// survive the streaming decoder.
    private func wire(_ json: String) -> Batch {
        let batch = Batch.decode(Data(json.utf8))
        precondition(batch.error == nil, batch.error ?? json)
        return batch
    }

    private func clipNode(_ runs: String, style: [String: Any]) throws -> NodeView {
        let session = ExactApp.shared.makeSession(label: "clip-text")
        self.session = session
        let p = session.presenter
        p.apply(wireBatch([
            ["op": "create", "id": 1, "kind": "text", "style": style],
            ["op": "roots", "ids": [1]],
            ["op": "frame", "id": 1, "x": 0.0, "y": 0.0, "w": 460.0, "h": 80.0],
        ]))
        p.apply(wire(#"{"ops":[{"op":"paragraph","id":1,"runs":[\#(runs)]}]}"#))
        return try XCTUnwrap(p.views[1])
    }

    private let red: [String: Any] = [
        "font_size": 36, "background_color": [255.0, 0.0, 0.0, 255.0], "background_clip": "text",
    ]

    func testAHiddenElementDoesNotPaintBackgroundThroughAVisibleRun() throws {
        var style = red
        style["visibility"] = "hidden"
        let node = try clipNode(
            #"{"id":2,"parent":1,"paint":true,"props":{"text":"Shown"},"style":{"font_size":36,"visibility":"visible"}}"#,
            style: style)
        let paragraph = try XCTUnwrap(node.paragraphLayout())
        let reds = redCount(node) { node.paintBackgroundThroughText($0, paragraph: paragraph, spec: node.paragraphSpec(), in: node.contentBox()) }
        XCTAssertEqual(reds, 0, "a hidden element's background does not paint through its glyphs")
    }

    func testAHiddenRunDoesNotPunchTheTextClip() throws {
        let node = try clipNode(
            #"{"id":2,"parent":1,"paint":true,"props":{"text":"Secret"},"style":{"font_size":36,"visibility":"hidden"}},{"id":3,"parent":1,"paint":true,"props":{"text":"        "},"style":{"font_size":36,"visibility":"visible"}},{"id":4,"parent":1,"paint":true,"props":{"text":"Shown"},"style":{"font_size":36,"visibility":"visible"}}"#,
            style: red)
        XCTAssertEqual(node.paragraphSpec().runs.map(\.hidden), [true, false, false])
        let secret = try XCTUnwrap(node.inlineText.first { $0.text == "Secret" })
        let shown = try XCTUnwrap(node.inlineText.first { $0.text == "Shown" })
        let hidden = node.inlineRects(secret).reduce(CGRect.null) { $0.union($1) }
        let visible = node.inlineRects(shown).reduce(CGRect.null) { $0.union($1) }
        let paragraph = try XCTUnwrap(node.paragraphLayout())
        let path = TextEngine.glyphPath(paragraph, spec: node.paragraphSpec(), in: node.contentBox())
        XCTAssertFalse(hidden.insetBy(dx: 2, dy: 2).isEmpty)
        XCTAssertFalse(path.boundingBox.intersects(hidden.insetBy(dx: 2, dy: 2)), "a hidden run punches no glyph hole")
        XCTAssertTrue(path.boundingBox.intersects(visible.insetBy(dx: 2, dy: 2)), "the visible run still clips")
    }
}
