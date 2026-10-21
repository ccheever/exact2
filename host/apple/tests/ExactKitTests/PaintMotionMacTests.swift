#if os(macOS)
import AppKit
import XCTest
@testable import ExactKit

/// Paint motion on AppKit (LLP 1062): an inline run paints the colour its
/// paragraph is told it inherits; a view whose own appearance differs from
/// the session's is noted for the host, once, until it agrees again.
final class PaintMotionMacTests: XCTestCase {
    func testAnInlineRunPaintsTheColourItsParagraphIsTold() throws {
        _ = NSApplication.shared
        let session = ExactApp.shared.makeSession(label: "paint-run-mac")
        defer { session.destroy() }
        let p = session.presenter
        p.apply(wireBatch([
            ["op": "create", "id": 1, "kind": "text", "style": ["font_size": 16.0, "text_color": [0.0, 0.0, 0.0, 255.0]]],
            ["op": "paragraph", "id": 1, "runs": [
                ["id": 2, "parent": 1, "paint": true, "props": ["text": "plain "], "style": ["text_color": [0.0, 0.0, 0.0, 255.0]]],
                ["id": 3, "parent": 1, "paint": true, "props": ["text": "red"], "style": ["text_color": [255.0, 0.0, 0.0, 255.0]]],
            ]],
            ["op": "roots", "ids": [1]],
            ["op": "frame", "id": 1, "x": 0.0, "y": 0.0, "w": 300.0, "h": 40.0],
        ]))
        let node = try XCTUnwrap(p.views[1])
        let grey = [128.0, 128.0, 128.0, 255.0]
        // The host re-sends the paragraph with the run's presented colour.
        let paragraph = { (c: [Double]) in wireBatch([["op": "paragraph", "id": 1, "runs": [
            ["id": 2, "parent": 1, "paint": true, "props": ["text": "plain "], "style": ["text_color": c]],
            ["id": 3, "parent": 1, "paint": true, "props": ["text": "red"], "style": ["text_color": [255.0, 0.0, 0.0, 255.0]]],
        ]]]) }
        p.apply(paragraph(grey))
        XCTAssertEqual(node.paragraphSpec().runs.map(\.color), [grey, [255, 0, 0, 255]])
        p.apply(paragraph([0.0, 0.0, 0.0, 255.0]))
        XCTAssertEqual(node.paragraphSpec().runs.map(\.color), [[0, 0, 0, 255], [255, 0, 0, 255]])
    }

    func testAViewInItsOwnAppearanceIsNotedOnceUntilItAgreesAgain() throws {
        _ = NSApplication.shared
        let session = ExactApp.shared.makeSession(label: "paint-view-scheme-mac")
        defer { session.destroy() }
        XCTAssertNil(session.boot(size: CGSize(width: 390, height: 844)).error)
        let sheet = NSView(frame: NSRect(x: 0, y: 0, width: 100, height: 100))
        sheet.appearance = NSAppearance(named: .aqua)
        let view = NodeView(id: 99_999, kind: "view", presenter: session.presenter)
        sheet.addSubview(view)
        session.scheme(dark: false)
        session.noteAppearance(view)
        XCTAssertNil(session.viewDark[view.id], "it agrees with the session")
        sheet.appearance = NSAppearance(named: .darkAqua)
        XCTAssertTrue(view.drawsDark)
        session.noteAppearance(view)
        XCTAssertEqual(session.viewDark[view.id], true)
        sheet.appearance = NSAppearance(named: .aqua)
        session.noteAppearance(view)
        XCTAssertNil(session.viewDark[view.id], "it agrees again")
    }

    /// An `svg`'s `light-dark()` paint is resolved into its scene's layers,
    /// so an appearance change applies the scene again, as Chrome repaints
    /// it (minesweeper F11, paint F10).
    func testAnSvgPaintFollowsItsViewsAppearance() throws {
        _ = NSApplication.shared
        let session = ExactApp.shared.makeSession(label: "paint-svg-scheme-mac")
        defer { session.destroy() }
        let p = session.presenter
        p.root.appearance = NSAppearance(named: .aqua)
        let pair: [Any] = [[36.0, 86.0, 51.0, 255.0], [207.0, 255.0, 220.0, 255.0]]
        let shape: [String: Any] = ["id": 1, "n": 1, "o": 1, "p": [0.0, 0.0, 0.0, 1.0, 10.0, 0.0, 1.0, 10.0, 10.0, 3.0], "s": pair, "w": 2.0]
        p.apply(wireBatch([
            ["op": "create", "id": 1, "kind": "svg", "style": [:]],
            ["op": "roots", "ids": [1]],
            ["op": "frame", "id": 1, "x": 0.0, "y": 0.0, "w": 24.0, "h": 24.0],
            ["op": "svg", "id": 1, "scene": ["box": [0.0, 0.0, 24.0, 24.0], "t": [1.0, 0.0, 0.0, 1.0, 0.0, 0.0], "els": [shape]]],
        ]))
        let view = try XCTUnwrap(p.views[1])
        func stroke() -> [CGFloat]? {
            (view.layer?.sublayers?.lazy.compactMap { $0.sublayers?.first as? CAShapeLayer }.first?.strokeColor)?.components
        }
        XCTAssertEqual(stroke()?.map { ($0 * 255).rounded() }, [36, 86, 51, 255])
        p.root.appearance = NSAppearance(named: .darkAqua)
        XCTAssertTrue(view.drawsDark)
        XCTAssertEqual(stroke()?.map { ($0 * 255).rounded() }, [207, 255, 220, 255])
    }
}
#endif
