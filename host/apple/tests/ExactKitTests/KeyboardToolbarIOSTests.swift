#if os(iOS)
import UIKit
import XCTest
@testable import ExactKit

/// LLP 1008 §9.1: under `interactive-widget="overlays-content"` a keyboard
/// toolbar rides the keyboard by a transform, and the scroller that ends at
/// its top keeps its content clear by a content inset, staying at its end;
/// nothing is laid out again, and both come back when the keyboard goes.
final class KeyboardToolbarIOSTests: XCTestCase {
    func testAKeyboardToolbarRidesTheKeyboardAndItsScrollerInsetsInsteadOfARelayout() throws {
        let session = ExactApp.shared.makeSession(label: "keyboard-toolbar")
        defer { session.destroy() }
        let p = session.presenter
        p.viewport.frame = CGRect(x: 0, y: 0, width: 400, height: 800)
        let window = UIWindow(frame: p.viewport.frame)
        window.addSubview(p.viewport)
        window.isHidden = false
        defer { window.isHidden = true }
        p.apply(wireBatch([
            ["op": "create", "id": 1, "kind": "view"],
            ["op": "props", "id": 1, "set": ["interactiveWidget": "overlays-content"]],
            ["op": "create", "id": 2, "kind": "view", "style": ["overflow_y": "scroll"]],
            ["op": "create", "id": 3, "kind": "view", "style": ["padding_bottom": 34.0]],
            ["op": "props", "id": 3, "set": ["accessibilityRole": "toolbar", "toolbarPlacement": "keyboard"]],
            // Children of their own: an inert leaf box would be a flat layer.
            ["op": "create", "id": 4, "kind": "view"],
            ["op": "create", "id": 5, "kind": "view"],
            ["op": "children", "id": 2, "ids": [4]],
            ["op": "children", "id": 3, "ids": [5]],
            ["op": "children", "id": 1, "ids": [2, 3]],
            ["op": "roots", "ids": [1]],
            ["op": "frame", "id": 1, "x": 0.0, "y": 0.0, "w": 400.0, "h": 800.0],
            ["op": "frame", "id": 2, "x": 0.0, "y": 0.0, "w": 400.0, "h": 710.0],
            ["op": "content", "id": 2, "w": 400.0, "h": 2000.0],
            ["op": "frame", "id": 3, "x": 0.0, "y": 710.0, "w": 400.0, "h": 90.0],
            ["op": "frame", "id": 4, "x": 0.0, "y": 0.0, "w": 400.0, "h": 2000.0],
            ["op": "frame", "id": 5, "x": 16.0, "y": 8.0, "w": 368.0, "h": 40.0]
        ]))
        XCTAssertEqual(p.interactiveWidget, "overlays-content")
        let transcript = try XCTUnwrap(p.views[2]?.scroll), bar = try XCTUnwrap(p.views[3])
        let end = transcript.contentSize.height - transcript.bounds.height
        transcript.contentOffset.y = end
        let frames = (p.views[2]!.frame, bar.frame)
        // A keyboard whose top is 340 points above the window's bottom.
        p.applyKeyboard(top: 460, duration: 0, curve: 0)
        XCTAssertEqual(p.keyboardInset, 340)
        XCTAssertEqual(bar.keyboardLift, 306, "the keyboard's overlap less the bar's own bottom padding")
        XCTAssertEqual(bar.transform.ty, -306, accuracy: 0.01, "a transform, not a frame")
        XCTAssertEqual(transcript.contentInset.bottom, 306)
        XCTAssertEqual(transcript.verticalScrollIndicatorInsets.bottom, 306)
        XCTAssertEqual(transcript.contentOffset.y, end + 306, accuracy: 0.5, "at its end, it stays at its end")
        XCTAssertEqual(p.views[2]!.frame, frames.0, "nothing laid out again")
        XCTAssertEqual(bar.bounds.size, frames.1.size)
        p.applyKeyboard(top: nil, duration: 0, curve: 0)
        XCTAssertEqual(bar.keyboardLift, 0)
        XCTAssertEqual(bar.transform, .identity)
        XCTAssertEqual(transcript.contentInset.bottom, 0)
        XCTAssertEqual(transcript.contentOffset.y, end, accuracy: 0.5)
    }
}
#endif
