#if os(iOS)
import UIKit
import XCTest
@testable import ExactKit

/// Paint motion on UIKit (LLP 1062): a text re-styled with a presented colour
/// is painted in the batch that carries it, not by a worker a frame later; an inline run
/// paints the colour its paragraph is told it inherits; a view whose own
/// appearance differs from the session's is noted for the host. UIKit, so a
/// simulator runs it:
///   bun host/apple/build.mjs --test --ios
final class PaintMotionIOSTests: XCTestCase {
    private var window: UIWindow!

    private func fixture(_ label: String) -> ExactSession {
        let session = ExactApp.shared.makeSession(label: label)
        let p = session.presenter
        window = UIWindow(frame: CGRect(x: 0, y: 0, width: 400, height: 400))
        p.viewport.frame = window.bounds
        window.addSubview(p.viewport)
        window.makeKeyAndVisible()
        p.apply(wireBatch([
            ["op": "create", "id": 1, "kind": "view"],
            ["op": "create", "id": 2, "kind": "text",
             "props": ["text": "The colour moves"], "style": ["font_size": 16.0, "text_color": [0.0, 0.0, 0.0, 255.0]]],
            ["op": "create", "id": 3, "kind": "text", "style": ["font_size": 16.0, "text_color": [0.0, 0.0, 0.0, 255.0]]],
            ["op": "paragraph", "id": 3, "runs": [
                ["id": 4, "parent": 3, "paint": true, "props": ["text": "plain "], "style": ["text_color": [0.0, 0.0, 0.0, 255.0]]],
                ["id": 5, "parent": 3, "paint": true, "props": ["text": "red"], "style": ["text_color": [255.0, 0.0, 0.0, 255.0]]],
            ]],
            ["op": "children", "id": 1, "ids": [2, 3]],
            ["op": "roots", "ids": [1]],
            ["op": "frame", "id": 1, "x": 0.0, "y": 0.0, "w": 400.0, "h": 400.0],
            ["op": "frame", "id": 2, "x": 0.0, "y": 0.0, "w": 300.0, "h": 40.0],
            ["op": "frame", "id": 3, "x": 0.0, "y": 50.0, "w": 300.0, "h": 40.0],
        ]))
        window.layoutIfNeeded(); p.viewport.layer.displayIfNeeded()
        p.paintVisibleText()
        return session
    }

    func testAPresentedColourIsPaintedInTheBatchThatCarriesIt() throws {
        let session = fixture("paint-text-now")
        defer { session.destroy(); RegionTextExecutor.queue.isSuspended = false }
        let p = session.presenter
        let node = try XCTUnwrap(p.views[2])
        XCTAssertTrue(node.textRasterReady, "the fixture's paragraph shows")
        // No worker may run: pixels that come a frame later cannot pass.
        RegionTextExecutor.queue.isSuspended = true
        let grey = [128.0, 128.0, 128.0, 255.0]
        // A frame of paint motion re-sends the style with the presented colour.
        p.apply(wireBatch([["op": "style", "id": 2, "style": ["font_size": 16.0, "text_color": grey]]]))
        XCTAssertEqual(p.textRasters.inFlight, 0, "no worker job")
        XCTAssertTrue(node.textRasterReady, "painted as the batch ended")
        XCTAssertEqual(node.textRasterKey?.spec.runs.first?.color, grey)
        p.apply(wireBatch([["op": "style", "id": 2, "style": ["font_size": 16.0, "text_color": [0.0, 0.0, 0.0, 255.0]]]]))
        XCTAssertTrue(node.textRasterReady)
        XCTAssertEqual(node.textRasterKey?.spec.runs.first?.color, [0, 0, 0, 255])
    }

    func testAnInlineRunPaintsTheColourItsParagraphIsTold() throws {
        let session = fixture("paint-run")
        defer { session.destroy() }
        let p = session.presenter
        let node = try XCTUnwrap(p.views[3])
        let grey = [128.0, 128.0, 128.0, 255.0]
        // The host re-sends the paragraph with the run's presented colour.
        let paragraph = { (c: [Double]) in wireBatch([["op": "paragraph", "id": 3, "runs": [
            ["id": 4, "parent": 3, "paint": true, "props": ["text": "plain "], "style": ["text_color": c]],
            ["id": 5, "parent": 3, "paint": true, "props": ["text": "red"], "style": ["text_color": [255.0, 0.0, 0.0, 255.0]]],
        ]]]) }
        p.apply(paragraph(grey))
        XCTAssertEqual(node.paragraphSpec().runs.map(\.color), [grey, [255, 0, 0, 255]])
        XCTAssertTrue(node.textRasterReady, "painted as the batch ended")
        p.apply(paragraph([0.0, 0.0, 0.0, 255.0]))
        XCTAssertEqual(node.paragraphSpec().runs.map(\.color), [[0, 0, 0, 255], [255, 0, 0, 255]])
    }

    func testAViewInItsOwnAppearanceIsNotedOnceUntilItAgreesAgain() throws {
        let session = ExactApp.shared.makeSession(label: "paint-view-scheme")
        defer { session.destroy() }
        XCTAssertNil(session.boot(size: CGSize(width: 390, height: 844)).error)
        session.scheme(dark: false)
        let window = UIWindow(frame: CGRect(x: 0, y: 0, width: 100, height: 100))
        let sheet = UIView(frame: window.bounds)
        window.addSubview(sheet)
        window.makeKeyAndVisible()
        let view = NodeView(id: 99_999, kind: "view", presenter: session.presenter)
        sheet.addSubview(view)
        session.noteAppearance(view)
        XCTAssertNil(session.viewDark[view.id], "it agrees with the session")
        sheet.overrideUserInterfaceStyle = .dark
        sheet.updateTraitsIfNeeded()
        XCTAssertTrue(view.drawsDark)
        session.noteAppearance(view)
        XCTAssertEqual(session.viewDark[view.id], true)
        sheet.overrideUserInterfaceStyle = .light
        sheet.updateTraitsIfNeeded()
        session.noteAppearance(view)
        XCTAssertNil(session.viewDark[view.id], "it agrees again")
    }
}
#endif
