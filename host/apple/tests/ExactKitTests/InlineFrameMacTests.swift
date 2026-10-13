import XCTest
@testable import ExactKit
#if os(macOS)
import AppKit

final class InlineFrameMacTests: XCTestCase {
    func testEmptyRunHasAnEncodableFrameAtItsInsertionPoint() throws { try checkEdgeCase("empty") }
    func testSmallRunUsesItsOwnFontMetrics() throws { try checkEdgeCase("small") }
    func testHangingSpaceDoesNotAddAClientRect() throws { try checkEdgeCase("wrap") }

    private func checkEdgeCase(_ name: String) throws {
        _ = NSApplication.shared
        let session = ExactApp.shared.makeSession(label: "inline-edge")
        defer { session.destroy() }
        XCTAssertNil(session.boot(plan: InlineFrameFixture.plan, size: CGSize(width: 600, height: 400)).error)
        let window = NSWindow(contentRect: NSRect(x: 0, y: 0, width: 600, height: 400), styleMask: [.titled], backing: .buffered, defer: false)
        window.isReleasedWhenClosed = false
        defer { window.close() }
        window.contentView = ExactView(session: session)
        window.contentView?.layoutSubtreeIfNeeded()
        try InlineFrameFixture.check(name, session: session)
    }

    func testWrappedRunFrameAndLayoutFollowTheParagraphThroughScroll() throws {
        _ = NSApplication.shared
        let dir = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
        try FileManager.default.createDirectory(at: dir, withIntermediateDirectories: true)
        defer { try? FileManager.default.removeItem(at: dir) }
        let source = """
        component App
          state result = ""
          action measure
            let r = frame("link")
            result = `${r.x},${r.y},${r.width},${r.height},${r.unavailable}`
          view
            column padding=24 gap=12
              column height=100 width=300 overflow="scroll" testId="scroller"
                text width=260 font-size=16 font-family="Arial" line-height="20px" testId="paragraph"
                  text "Before this, please read "
                  text "see the earlier fix in pull request twelve" id="link" testId="link" href="https://example.com/"
                  text " after it."
                view height=300 flex-shrink=0
              button press=measure testId="measure"
                text "Measure"
              text result testId="result"

        """
        try source.write(to: dir.appendingPathComponent("app.contract"), atomically: true, encoding: .utf8)
        let compiler = Process()
        compiler.executableURL = URL(fileURLWithPath: try XCTUnwrap(ProcessInfo.processInfo.environment["EXACT_CONTRACT"]))
        compiler.arguments = ["build", dir.appendingPathComponent("app.contract").path, "-o", dir.appendingPathComponent("app.plan").path]
        try compiler.run(); compiler.waitUntilExit()
        XCTAssertEqual(compiler.terminationStatus, 0)
        let session = ExactApp.shared.makeSession(label: "inline-frame")
        defer { session.destroy() }
        XCTAssertNil(session.boot(plan: try Data(contentsOf: dir.appendingPathComponent("app.plan")), size: CGSize(width: 600, height: 400)).error)
        let window = NSWindow(contentRect: NSRect(x: 0, y: 0, width: 600, height: 400), styleMask: [.titled], backing: .buffered, defer: false)
        window.isReleasedWhenClosed = false
        defer { window.close() }
        window.contentView = ExactView(session: session)
        window.contentView?.layoutSubtreeIfNeeded()
        let p = session.presenter, agent = Agent(session: session)
        func node(_ name: String) throws -> NodeView { try XCTUnwrap(p.views.values.first { $0.props["testId"] == name }) }
        let paragraph = try node("paragraph"), button = try node("measure")
        let run = try XCTUnwrap(paragraph.inlineText.first { $0.props["id"] == "link" })
        let scroll = try XCTUnwrap(try node("scroller").scroll)
        var original: CGRect?
        for top in [CGFloat(0), 25] {
            scroll.contentView.scroll(to: CGPoint(x: 0, y: top))
            scroll.reflectScrolledClipView(scroll.contentView)
            let fragments = paragraph.inlineRects(run)
            XCTAssertEqual(fragments.count, 2)
            let expected = fragments.map { agent.box(paragraph, region: $0) }.reduce(CGRect.null) { $0.union($1) }
            if let original { XCTAssertEqual(expected.minY, original.minY - top, accuracy: 0.01) }
            else {
                original = expected
                // Chrome, Arial 16px/20px at 260px: the span's two client rects.
                let box = agent.box(paragraph)
                XCTAssertEqual(expected.minX - box.minX, 0, accuracy: 1)
                XCTAssertEqual(expected.minY - box.minY, 1, accuracy: 1)
                XCTAssertEqual(expected.width, 225.921875, accuracy: 1)
                XCTAssertEqual(expected.height, 37, accuracy: 1)
            }
            p.onPress?(button.id)
            let result = try XCTUnwrap(try node("result").props["text"]).split(separator: ",")
            XCTAssertEqual(result.count, 5)
            XCTAssertEqual(result.last, "false", "an inline run has a frame")
            for (value, want) in zip(result.prefix(4), [expected.minX, expected.minY, expected.width, expected.height]) {
                XCTAssertEqual(try XCTUnwrap(Double(value)), Double(want), accuracy: 0.01)
            }
            let rows = try XCTUnwrap(agent.layout()["nodes"] as? [[String: Any]])
            let row = rows.first { ($0["id"] as? Int) == Int(run.id) }
            XCTAssertNotNil(row, "layout lists inline runs")
            if let row {
                for (key, want) in zip(["x", "y", "w", "h"], [expected.minX, expected.minY, expected.width, expected.height]) {
                    XCTAssertEqual(try XCTUnwrap(row[key] as? Double), Double(want), accuracy: 0.01)
                }
            }
            let detail = try XCTUnwrap(agent.layout(["id": Int(run.id)])["node"] as? [String: Any])
            let space = try XCTUnwrap(detail["space"] as? [String: Any])
            let viewport = try XCTUnwrap(space["viewport"] as? [String: Any])
            XCTAssertEqual(try XCTUnwrap(viewport["w"] as? Double), Double(expected.width), accuracy: 0.01)
            XCTAssertEqual(try XCTUnwrap(viewport["y"] as? Double), Double(expected.minY), accuracy: 0.01)
        }
    }
}
#endif

// Compiled from the fixture below with `contract build`; shared by AppKit and UIKit.
// component App
//   view
//     column padding=24 gap=12
//       text width=260 font-size=16 font-family="Arial" line-height="20px" testId="empty-paragraph"
//         text "Hi" testId="before"
//         text "" id="empty" testId="empty"
//         text " there"
//       text width=260 font-size=16 font-family="Arial" line-height="60px" testId="mixed-paragraph"
//         text "link" font-size=12 id="small" testId="small"
//         text "BIG" font-size=48
//       text width=40 font-size=16 font-family="Arial" line-height="20px" testId="wrap-paragraph"
//         text "Hi"
//         text " there" id="wrap" testId="wrap"
enum InlineFrameFixture {
    static let plan = Data(base64Encoded: "RVhQTAUAAABQj6VlcW+hgTSjEgztc+xGGA1E2ggCJPIAAAAA//////////8RAAAABAAAAGZsZXgGAAAAY29sdW1uBQAAAEFyaWFsBAAAADIwcHgPAAAAZW1wdHktcGFyYWdyYXBoAgAAAEhpBgAAAGJlZm9yZQAAAAAFAAAAZW1wdHkGAAAAIHRoZXJlBAAAADYwcHgPAAAAbWl4ZWQtcGFyYWdyYXBoBAAAAGxpbmsFAAAAc21hbGwDAAAAQklHDgAAAHdyYXAtcGFyYWdyYXBoBAAAAHdyYXCoAAAABCgCAAAAACgCAQAAACgAAAAAAAAAOEAoAAAAAAAAAChAKAAAAAAAAEBwQCgAAAAAAAAAMEAoAAAAAAAAACBAKAIDAAAAKAIEAAAAKAIFAAAAKAIGAAAAKAIHAAAAKAIIAAAAKAIJAAAAKAIKAAAAKAILAAAAKAIMAAAAKAINAAAAKAIOAAAAKAAAAAAAAABIQCgAAAAAAAAAREAoAg8AAAAoAhAAAAAoAAAAAAAAAAAAAAAAAAAAAAEAAAACAAAAAAAAAAAAAAAJAAAAAAAAAAEAAAABAAAAAQAAAAIAAAABAAAAAwAAAAEAAAAEAAAAAQAAAAUAAAABAAAABgAAAAEAAAAHAAAAAQAAAAgAAAABAAAACQAAAAH/////Av////8D/////wT/////Bf////8G/////wf/////CP////8AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAALAAAAAP//////////AAAAAAAAAAAIAAAAAAAAAAAAAAD/////AQAAAAD/////AAAAAAgAAAAFAAAAAAAAAAAAAAD/////AQEAAAD/////AAAAAA0AAAACAAAAAAAAAAAAAAD/////AQEAAAD/////AQAAAA8AAAADAAAAAAAAAAAAAAD/////AQEAAAD/////AgAAABIAAAABAAAAAAAAAAAAAAD/////AQAAAAD/////AQAAABMAAAAFAAAAAAAAAAAAAAD/////AQUAAAD/////AAAAABgAAAAEAAAAAAAAAAAAAAD/////AQUAAAD/////AQAAABwAAAACAAAAAAAAAAAAAAD/////AQAAAAD/////AgAAAB4AAAAFAAAAAAAAAAAAAAD/////AQgAAAD/////AAAAACMAAAABAAAAAAAAAAAAAAD/////AQgAAAD/////AQAAACQAAAADAAAAAAAAAAAAAAD/////JwAAAAEfAAIAAAAGAAAAAAEOAAgAAAAGAAAAAAEGAA4AAAAKAAAAAQEHAA4AAAAKAAAAAQEIAA4AAAAKAAAAAQEJAA4AAAAKAAAAAQEcABgAAAAKAAAAAQEdABgAAAAKAAAAAQEAACIAAAAKAAAAAQFAACwAAAAKAAAAAQFDADYAAAAKAAAAAQFFAEAAAAAGAAAAAQALAEYAAAAGAAAAAQAAAEwAAAAGAAAAAQALAFIAAAAGAAAAAQAAAFgAAAAGAAAAAQAsAF4AAAAGAAAAAQALAF4AAAAGAAAAAQAAAGQAAAAGAAAAAQEAACIAAAAKAAAAAQFAACwAAAAKAAAAAQFDADYAAAAKAAAAAQFFAGoAAAAGAAAAAQALAHAAAAAGAAAAAQAAAHYAAAAGAAAAAQFAABgAAAAKAAAAAQAsAHwAAAAGAAAAAQALAHwAAAAGAAAAAQAAAIIAAAAGAAAAAQFAAIgAAAAKAAAAAQEAAJIAAAAKAAAAAQFAACwAAAAKAAAAAQFDADYAAAAKAAAAAQFFAEAAAAAGAAAAAQALAJwAAAAGAAAAAQAAAEwAAAAGAAAAAQAAAGQAAAAGAAAAAQAsAKIAAAAGAAAAAQALAKIAAAAGAAAAAQAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA")!

    static func check(_ name: String, session: ExactSession) throws {
        let p = session.presenter, agent = Agent(session: session)
        let run = try XCTUnwrap(p.inlineOwners.keys.compactMap { p.inlineText($0) }.first { $0.props["testId"] == name })
        let host = try XCTUnwrap(p.textHost(run.id))
        // Chrome's getClientRects/getBoundingClientRect, Arial, relative to the paragraph.
        let expected: [String: CGRect] = [
            "empty": CGRect(x: 15.109375, y: 1, width: 0, height: 17),
            "small": CGRect(x: 0, y: 35, width: 18.015625, height: 14),
            "wrap": CGRect(x: 0, y: 21, width: 36.46875, height: 17),
        ]
        let want = try XCTUnwrap(expected[name])
        let fragments = host.inlineRects(run)
        XCTAssertEqual(fragments.count, 1, name)
        let detail = agent.layout(["id": Int(run.id)])
        XCTAssertTrue(JSONSerialization.isValidJSONObject(detail), "every coordinate must be finite: \(name)")
        let node = try XCTUnwrap(detail["node"] as? [String: Any])
        let space = try XCTUnwrap(node["space"] as? [String: Any])
        let viewport = try XCTUnwrap(space["viewport"] as? [String: Any])
        let frame = try XCTUnwrap(p.inlineFrame(run.id), "mounted empty runs also have a frame")
        let origin = agent.box(host)
        let values = [frame.minX, frame.minY, frame.width, frame.height]
        let targets = [want.minX + origin.minX, want.minY + origin.minY, want.width, want.height]
        for (index, key) in ["x", "y", "w", "h"].enumerated() {
            XCTAssertEqual(values[index], targets[index], accuracy: 1, "\(name) \(key)")
            XCTAssertEqual(try XCTUnwrap(viewport[key] as? Double), Double(values[index]), accuracy: 0.01)
        }
        let rows = try XCTUnwrap(agent.layout()["nodes"] as? [[String: Any]])
        XCTAssertNotNil(rows.first { ($0["id"] as? Int) == Int(run.id) })
    }
}
