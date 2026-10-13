#if os(iOS)
import UIKit
import XCTest
@testable import ExactKit
@testable import ExactSurfaces

/// A sheet holds its background route's geometry until it closes (LLP
/// 1075.003 §9 D4): a content box layout sent (`fieldContent`) is geometry
/// too, held with the frames it goes with and replayed with them, so the
/// background never shows a new inset in an old frame. What is placed in
/// the content box waits with it: a 2D canvas's bitmap, an `svg`'s whole
/// scene, which layout sized, and a native button's face, whose padding the
/// kernel resolved with it, and which sides its author wrote. A box that had
/// none (no padding) waits too, and a view whose geometry is held stays a
/// view until the hold is over.
final class SheetBackgroundGeometryIOSTests: XCTestCase {
    override class func setUp() { super.setUp(); ExactSurfaces.install() } // LLP 1047.001 D4
    private var window: UIWindow?
    private var host: UIViewController?
    private var sessions: [ExactSession] = []

    override func tearDown() {
        for session in sessions { session.destroy() }
        sessions = []
        window?.isHidden = true
        window = nil
        host = nil
        super.tearDown()
    }

    private func until(_ what: String, _ seconds: Double = 5, _ done: () -> Bool) {
        let deadline = Date().addingTimeInterval(seconds)
        while !done(), Date() < deadline { RunLoop.main.run(mode: .default, before: Date().addingTimeInterval(0.02)) }
        XCTAssertTrue(done(), what)
    }

    private func button(_ id: Int, padding: Double?) -> [String: Any] {
        ["op": "create", "id": id, "kind": "control", "handlers": ["press"], "props": ["type": "button", "accessibilityRole": "button"],
         "style": padding.map { ["appearance": "auto", "padding_left": $0] } ?? ["appearance": "auto"]]
    }

    func testABackgroundsContentBoxAndCanvasPlacementWaitForTheSheetToClose() throws {
        let session = ExactApp.shared.makeSession(label: "sheet-background-geometry")
        sessions.append(session)
        XCTAssertNotNil(session.boot(plan: Data(), size: CGSize(width: 402, height: 874)).error, "no plan in no bytes")
        let view = ExactView(session: session)
        let host = UIViewController()
        let scene = UIApplication.shared.connectedScenes.first as? UIWindowScene
        let window = scene.map { UIWindow(windowScene: $0) } ?? UIWindow(frame: CGRect(x: 0, y: 0, width: 402, height: 874))
        window.frame = CGRect(x: 0, y: 0, width: 402, height: 874)
        window.rootViewController = host
        window.makeKeyAndVisible()
        host.view.addSubview(view)
        view.frame = host.view.bounds
        self.window = window
        self.host = host
        let p = session.presenter
        p.viewport.frame = host.view.bounds
        // A sheet waits for the first drawn frame, which this window never draws.
        session.firstDrawMs = ExactEnv.wall()
        let apply = { (ops: [[String: Any]]) in p.apply(wireBatch(ops)) }
        let frame = { (id: Int) -> [String: Any] in ["op": "frame", "id": id, "x": 0.0, "y": 0.0, "w": 402.0, "h": 874.0] }
        apply([
            ["op": "create", "id": 1, "kind": "view", "props": ["navigationBack": "back", "navigationKey": "0"]], frame(1),
            ["op": "create", "id": 2, "kind": "view", "props": ["navigationKey": "rooms"]], frame(2),
            ["op": "create", "id": 3, "kind": "canvas2d", "handlers": ["pointerdown"]],
            ["op": "style", "id": 3, "style": ["padding_left": ["pct": 10]]],
            ["op": "frame", "id": 3, "x": 0.0, "y": 100.0, "w": 190.0, "h": 100.0],
            contentOp(3, 190, 100, padding: [0, 0, 0, 40]),
            ["op": "create", "id": 6, "kind": "control", "handlers": ["press"],
             "props": ["type": "button", "accessibilityRole": "button"], "style": ["appearance": "auto", "padding_left": ["pct": 10]]],
            ["op": "frame", "id": 6, "x": 0.0, "y": 300.0, "w": 120.0, "h": 34.0],
            contentOp(6, 120, 34, padding: [0, 0, 0, 40], button: true),
            ["op": "create", "id": 7, "kind": "svg"],
            ["op": "style", "id": 7, "style": ["padding_left": ["pct": 10]]],
            ["op": "frame", "id": 7, "x": 0.0, "y": 400.0, "w": 150.0, "h": 40.0],
            contentOp(7, 150, 40, padding: [0, 0, 0, 40]),
            // `viewBox="0 0 100 100"` in a 110 by 40 content box: scaled by 0.4, centred 35 in
            ["op": "svg", "id": 7, "scene": ["box": [40.0, 0.0, 110.0, 40.0], "t": [0.4, 0.0, 0.0, 0.4, 35.0, 0.0], "els": [Any]()]],
            // A canvas padded in points, its content box sent too
            ["op": "create", "id": 8, "kind": "canvas2d"],
            ["op": "style", "id": 8, "style": ["padding_left": 40]],
            ["op": "frame", "id": 8, "x": 0.0, "y": 500.0, "w": 190.0, "h": 100.0],
            contentOp(8, 190, 100, padding: [0, 0, 0, 40]),
            // A button padded in points whose border alone changes, and a canvas with no padding yet
            ["op": "create", "id": 9, "kind": "control", "handlers": ["press"],
             "props": ["type": "button", "accessibilityRole": "button"], "style": ["appearance": "auto", "padding_left": 40]],
            ["op": "frame", "id": 9, "x": 0.0, "y": 620.0, "w": 120.0, "h": 34.0],
            contentOp(9, 120, 34, padding: [0, 0, 0, 40], button: true),
            ["op": "create", "id": 10, "kind": "canvas2d"],
            ["op": "frame", "id": 10, "x": 0.0, "y": 700.0, "w": 200.0, "h": 100.0],
            // A button whose class has a padding and one whose class has none
            button(11, padding: 40), ["op": "frame", "id": 11, "x": 200.0, "y": 300.0, "w": 120.0, "h": 34.0], contentOp(11, 120, 34, padding: [0, 0, 0, 40], button: true),
            button(12, padding: nil), ["op": "frame", "id": 12, "x": 200.0, "y": 400.0, "w": 120.0, "h": 34.0],
            // A box a border keeps from being a flat leaf, its padding a percentage
            ["op": "create", "id": 13, "kind": "view", "style": ["background_color": [200, 200, 200, 255], "padding_left": ["pct": 10], "border_width_left": 10.0,
                                                               "border_style_left": "solid", "border_color_left": [0, 0, 0, 255]]],
            ["op": "frame", "id": 13, "x": 200.0, "y": 500.0, "w": 200.0, "h": 100.0],
            contentOp(13, 200, 100, padding: [0, 0, 0, 40], border: [0, 0, 0, 10]),
            ["op": "children", "id": 2, "ids": [3, 6, 7, 8, 9, 10, 11, 12, 13]],
            ["op": "children", "id": 1, "ids": [2]],
            ["op": "roots", "ids": [1]],
        ])
        // The button's face, as the runner would give it for each block
        var padding = 40.0, switched = false
        p.buttonFace = { id in
            var f = ButtonFace(); f.title = "Go"
            if id == 6 { f.rows.button = ["padding_left": .number(padding)] }
            if id == 9 { f.rows.button = ["padding_left": .number(40)] }
            if id == 11, !switched { f.rows.button = ["padding_left": .number(40)] }
            if id == 12, switched { f.rows.button = ["padding_left": .number(20)] }
            return f
        }
        p.apply(wireBatch([6, 9, 11, 12].map { ["op": "props", "id": $0, "set": ["testId": "go-\($0)"]] }))
        let unclassed = try XCTUnwrap(p.controls.controls[11] as? NativeButtonIOS), classed = try XCTUnwrap(p.controls.controls[12] as? NativeButtonIOS)
        let factory = try XCTUnwrap(classed.configuration?.contentInsets.leading)
        XCTAssertEqual(unclassed.configuration?.contentInsets.leading, 40)
        let go = try XCTUnwrap(p.controls.controls[6] as? NativeButtonIOS)
        let bordered = try XCTUnwrap(p.controls.controls[9] as? NativeButtonIOS)
        XCTAssertEqual(bordered.configuration?.contentInsets.leading, 40)
        let art = try XCTUnwrap(p.views[7])
        art.layoutIfNeeded()
        let svgRoot = try XCTUnwrap(art.layer.sublayers?.first { $0.name == SvgHost.rootName })
        XCTAssertEqual(svgRoot.frame, CGRect(x: 40, y: 0, width: 110, height: 40))
        XCTAssertEqual(go.configuration?.contentInsets.leading, 40)
        let canvas = try XCTUnwrap(p.views[3])
        let draw = { (w: Int, fresh: Bool) in
            p.canvas2d.apply(3, ["lifetime": 1, "generation": fresh ? 1 : 0, "fresh": fresh, "w": w, "h": 100, "scale": 1.0, "stretch": false,
                                 "lists": [Data]()], layer: canvas.layer)
        }
        canvas.layoutIfNeeded()
        draw(150, true)
        let bitmap = try XCTUnwrap(canvas.layer.sublayers?.first { $0.frame.size == CGSize(width: 150, height: 100) })
        let still = try XCTUnwrap(p.views[8])
        still.layoutIfNeeded()
        p.canvas2d.apply(8, ["lifetime": 1, "generation": 1, "fresh": true, "w": 150, "h": 100, "scale": 1.0, "stretch": false,
                             "lists": [Data]()], layer: still.layer)
        let stillBitmap = try XCTUnwrap(still.layer.sublayers?.first { $0.frame.size == CGSize(width: 150, height: 100) })
        let unpadded = try XCTUnwrap(p.views[10])
        unpadded.layoutIfNeeded()
        p.canvas2d.apply(10, ["lifetime": 1, "generation": 1, "fresh": true, "w": 200, "h": 100, "scale": 1.0, "stretch": false,
                              "lists": [Data]()], layer: unpadded.layer)
        let unpaddedBitmap = try XCTUnwrap(unpadded.layer.sublayers?.first { $0.frame == CGRect(x: 0, y: 0, width: 200, height: 100) })
        // Open the sheet over the canvas's route.
        apply([
            ["op": "create", "id": 4, "kind": "view", "props": ["navigationKey": "1", "navigationPresentation": "modal"]], frame(4),
            ["op": "create", "id": 5, "kind": "button", "handlers": ["press"], "props": ["id": "back"]],
            ["op": "frame", "id": 5, "x": 0.0, "y": 0.0, "w": 80.0, "h": 44.0],
            ["op": "children", "id": 4, "ids": [5]],
            ["op": "children", "id": 1, "ids": [2, 4]],
            ["op": "props", "id": 1, "set": ["navigationKey": "1"]],
        ])
        until("the sheet is presented") { host.presentedViewController != nil }
        // Its block narrows beneath the sheet: a new frame and content box,
        // the canvas drawn again at a new size, the button's frame and its
        // face resolved against the new block, and a points padding that
        // moves the other canvas's content box without resizing it.
        padding = 30; switched = true
        apply([
            ["op": "frame", "id": 6, "x": 0.0, "y": 300.0, "w": 110.0, "h": 34.0],
            ["op": "style", "id": 8, "style": ["padding_left": 30, "padding_right": 10]],
            contentOp(8, 190, 100, padding: [0, 10, 0, 30]),
            contentOp(3, 190, 100, padding: [0, 0, 0, 30]),
            ["op": "frame", "id": 3, "x": 0.0, "y": 100.0, "w": 190.0, "h": 100.0],
            contentOp(6, 110, 34, padding: [0, 0, 0, 30], button: true),
            contentOp(7, 150, 40, padding: [0, 0, 0, 30]),
            ["op": "style", "id": 9, "style": ["appearance": "auto", "padding_left": 40, "border_width_left": 5]],
            contentOp(9, 120, 34, padding: [0, 0, 0, 40], border: [0, 0, 0, 5], button: true),
            ["op": "style", "id": 10, "style": ["padding_left": 20]],
            contentOp(10, 200, 100, padding: [0, 0, 0, 20]),
            ["op": "svg", "id": 7, "scene": ["box": [30.0, 0.0, 120.0, 40.0], "t": [0.4, 0.0, 0.0, 0.4, 40.0, 0.0], "els": [Any]()]],
            // A class without the padding, and a first padding written
            ["op": "style", "id": 11, "style": ["appearance": "auto"]], ["op": "fieldContent", "id": 11, "rect": NSNull()],
            ["op": "style", "id": 12, "style": ["appearance": "auto", "padding_left": 20]], contentOp(12, 120, 34, padding: [0, 0, 0, 20], button: true),
            // The border goes: a flat leaf's paint, but its new box is held
            ["op": "style", "id": 13, "style": ["background_color": [200, 200, 200, 255], "padding_left": ["pct": 10]]],
            contentOp(13, 200, 100, padding: [0, 0, 0, 40]),
        ])
        art.layoutIfNeeded(); canvas.layoutIfNeeded(); still.layoutIfNeeded(); unpadded.layoutIfNeeded()
        draw(160, true)
        // A control's contents change, which asks every face again
        let controls = try JSONSerialization.data(withJSONObject: ["ops": [["op": "props", "id": 5, "set": ["testId": "back"]]], "controls": true])
        p.apply(Batch.decode(controls))
        XCTAssertEqual(canvas.contentBox().minX, 40, "the content box waits for the sheet")
        XCTAssertEqual(bitmap.frame, CGRect(x: 40, y: 0, width: 150, height: 100), "and so does the bitmap's place, its new pixels in the old box")
        XCTAssertEqual(go.configuration?.contentInsets.leading, 40, "and the button's insets, whatever face is asked")
        XCTAssertEqual(stillBitmap.frame.minX, 40, "and a bitmap whose points padding moved it")
        XCTAssertEqual(unpaddedBitmap.frame, CGRect(x: 0, y: 0, width: 200, height: 100), "and one given its first padding")
        XCTAssertEqual(bordered.configuration?.contentInsets.leading, 40, "and a button's insets when its border alone changed")
        XCTAssertEqual(unclassed.configuration?.contentInsets.leading, 40, "or its padding is no longer written")
        XCTAssertEqual(classed.configuration?.contentInsets.leading, factory, "or is written for the first time")
        XCTAssertEqual(p.views[13]?.contentBox().minX, 50, "a box whose border went stays a view, its box held")
        XCTAssertEqual(svgRoot.frame, CGRect(x: 40, y: 0, width: 110, height: 40), "and the scene where it was")
        XCTAssertEqual(svgRoot.sublayerTransform.m41, 35, "drawn as it was, its viewBox centred in the old box")
        p.svg.reappear(7, dark: true, clock: nil)
        XCTAssertEqual(svgRoot.frame, CGRect(x: 40, y: 0, width: 110, height: 40), "as it is in a new appearance")
        // Close it: everything held goes on together. This window finishes no
        // presentation (ModalDetentIOSTests), so the close is the modal stack's own.
        p.modals.closeTop(animated: false)
        XCTAssertEqual(canvas.contentBox().minX, 30)
        canvas.layoutIfNeeded()
        XCTAssertEqual(bitmap.frame, CGRect(x: 30, y: 0, width: 160, height: 100))
        still.layoutIfNeeded()
        XCTAssertEqual(stillBitmap.frame.minX, 30)
        unpadded.layoutIfNeeded()
        XCTAssertEqual(unpaddedBitmap.frame, CGRect(x: 20, y: 0, width: 180, height: 100))
        // The face is asked for again at the replay's projection sync, the next turn
        until("the button's face follows") { go.configuration?.contentInsets.leading == 30 }
        art.layoutIfNeeded()
        XCTAssertEqual(svgRoot.frame, CGRect(x: 30, y: 0, width: 120, height: 40))
        XCTAssertEqual(svgRoot.sublayerTransform.m41, 40)
        XCTAssertEqual(bordered.configuration?.contentInsets.leading, 40)
        until("the unwritten padding is UIKit's") { unclassed.configuration?.contentInsets.leading == factory }
        XCTAssertEqual(classed.configuration?.contentInsets.leading, 20)
        p.apply(wireBatch([["op": "props", "id": 5, "set": ["testId": "back-again"]]]))
        XCTAssertNil(p.views[13], "a flat leaf once the hold is over")
        p.apply(wireBatch([["op": "rank", "id": 13, "rank": 1]]))
        XCTAssertEqual(p.views[13]?.contentBox().minX, 40, "with the box replayed into its view")
        p.canvas2d.forget(3); p.canvas2d.forget(8); p.canvas2d.forget(10)
    }
}
#endif
