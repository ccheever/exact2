#if canImport(UIKit)
import UIKit
#else
import AppKit
#endif
import XCTest
@testable import ExactKit

/// `share`'s anchor (LLP 1069.003 D3): the pressed node's view; a menu
/// row's popover invoker when the row is not on screen; the content's
/// centre when no input ran the action. On iOS the sheet's popover source is
/// that anchor (what an iPad shows and an iPhone run can't catch).
///   bun host/apple/build.mjs --test [--ios]
#if canImport(UIKit)
final class ShareIOSTests: XCTestCase { func testAnchor() throws { try ShareAnchorCase.run(self) } }
#else
final class ShareMacTests: XCTestCase { func testAnchor() throws { try ShareAnchorCase.run(self) } }
#endif

enum ShareAnchorCase {
    static func run(_ test: XCTestCase) throws {
        #if canImport(UIKit)
        let window = UIWindow(frame: CGRect(x: 0, y: 0, width: 400, height: 400))
        let root = UIViewController()
        window.rootViewController = root
        window.makeKeyAndVisible()
        let p = Presenter()
        p.viewport.frame = window.bounds
        root.view.addSubview(p.viewport)
        #else
        _ = NSApplication.shared
        let window = NSWindow(contentRect: NSRect(x: 0, y: 0, width: 400, height: 400), styleMask: [.titled], backing: .buffered, defer: false)
        let p = Presenter()
        p.viewport.frame = window.contentView!.bounds
        window.contentView!.addSubview(p.viewport)
        #endif
        p.apply(wireBatch([
            ["op": "create", "id": 1, "kind": "view"],
            ["op": "create", "id": 2, "kind": "button", "handlers": ["press"]],
            ["op": "create", "id": 3, "kind": "view", "props": ["popover": "auto", "id": "more"]],
            ["op": "create", "id": 4, "kind": "button", "handlers": ["press"]],
            ["op": "create", "id": 5, "kind": "button", "props": ["popovertarget": "more"]],
            ["op": "children", "id": 1, "ids": [2, 5, 3]],
            ["op": "children", "id": 3, "ids": [4]],
            ["op": "roots", "ids": [1]],
            ["op": "frame", "id": 1, "x": 0.0, "y": 0.0, "w": 400.0, "h": 400.0],
            ["op": "frame", "id": 2, "x": 0.0, "y": 0.0, "w": 100.0, "h": 40.0],
            ["op": "frame", "id": 5, "x": 0.0, "y": 50.0, "w": 100.0, "h": 40.0],
            ["op": "frame", "id": 3, "x": 0.0, "y": 100.0, "w": 200.0, "h": 40.0],
            ["op": "frame", "id": 4, "x": 0.0, "y": 0.0, "w": 200.0, "h": 40.0],
        ]))
        // The pressed button anchors itself.
        let (pressed, rect) = p.shareAnchor(2)
        XCTAssertTrue(pressed === p.views[2])
        XCTAssertEqual(rect, p.views[2]!.bounds)
        // A menu row off screen anchors to the invoker of its popover.
        p.views[3]!.isHidden = true
        XCTAssertTrue(p.shareAnchor(4).0 === p.views[5])
        // No input: the content, centred.
        let (content, centre) = p.shareAnchor(nil)
        #if canImport(UIKit)
        XCTAssertTrue(content === root.view)
        // On an iPad the sheet's popover is the anchor's (an iPhone's sheet
        // has no popover; a test window has no scene to present in).
        let sheet = try XCTUnwrap(ShareSheet.present([URL(string: "https://example.com")!], title: "T", from: p.views[2]!, at: rect, outcome: { _ in }) as? UIActivityViewController)
        if UIDevice.current.userInterfaceIdiom == .pad {
            XCTAssertTrue(sheet.popoverPresentationController?.sourceView === p.views[2])
            XCTAssertEqual(sheet.popoverPresentationController?.sourceRect, rect)
        }
        sheet.dismiss(animated: false)
        #else
        XCTAssertTrue(content === window.contentView)
        #endif
        XCTAssertEqual(centre.midX, 200)
        _ = test
    }
}
