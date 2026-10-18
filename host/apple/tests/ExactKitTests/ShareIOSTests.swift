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
final class ShareIOSTests: XCTestCase {
    func testAnchor() throws { try ShareAnchorCase.run(self) }
    func testPicture() throws { try SharePictureCase.run() }
}
#else
final class ShareMacTests: XCTestCase {
    func testAnchor() throws { try ShareAnchorCase.run(self) }
    func testPicture() throws { try SharePictureCase.run() }
}
#endif

/// `share(files=["#id"])` (LLP 1125 D12): the node's view drawn to a PNG at
/// the display's scale, its pixels the view's.
enum SharePictureCase {
    static func run() throws {
        #if canImport(UIKit)
        let window = UIWindow(frame: CGRect(x: 0, y: 0, width: 400, height: 400))
        window.rootViewController = UIViewController()
        window.makeKeyAndVisible()
        let host = window.rootViewController!.view!
        #else
        _ = NSApplication.shared
        let window = NSWindow(contentRect: NSRect(x: 0, y: 0, width: 400, height: 400), styleMask: [.titled], backing: .buffered, defer: false)
        let host = window.contentView!
        #endif
        let p = Presenter()
        p.viewport.frame = host.bounds
        host.addSubview(p.viewport)
        p.apply(wireBatch([
            ["op": "create", "id": 1, "kind": "view"],
            ["op": "create", "id": 2, "kind": "view", "props": ["id": "card"], "style": ["background_color": [255, 0, 0, 255]]],
            ["op": "children", "id": 1, "ids": [2]],
            ["op": "roots", "ids": [1]],
            ["op": "frame", "id": 1, "x": 0.0, "y": 0.0, "w": 400.0, "h": 400.0],
            ["op": "frame", "id": 2, "x": 10.0, "y": 20.0, "w": 120.0, "h": 50.0],
        ]))
        let card = try XCTUnwrap(p.views[2])
        let picture = try XCTUnwrap(ShareSheet.picture(of: card))
        #if canImport(UIKit)
        let scale = card.traitCollection.displayScale
        let image = try XCTUnwrap(UIImage(data: picture.png)?.cgImage)
        #else
        let scale = window.backingScaleFactor
        let image = try XCTUnwrap(NSBitmapImageRep(data: picture.png)?.cgImage)
        #endif
        XCTAssertGreaterThan(scale, 0)
        XCTAssertEqual(picture.width, Int((120 * scale).rounded()))
        XCTAssertEqual(picture.height, Int((50 * scale).rounded()))
        XCTAssertEqual(image.width, picture.width)
        XCTAssertEqual(image.height, picture.height)
        // The view's own fill is in the picture, not a blank page.
        let pixel = try XCTUnwrap(rgba(image, x: image.width / 2, y: image.height / 2))
        XCTAssertGreaterThan(pixel.0, 200, "red: \(pixel)")
        XCTAssertLessThan(pixel.1, 60, "green: \(pixel)")
        XCTAssertGreaterThan(pixel.3, 200, "alpha: \(pixel)")
        // Two ids whose names come out alike stay two files, each its own picture.
        XCTAssertEqual(ShareSheet.pictureName("card.one"), ShareSheet.pictureName("card-one"))
        let dir = FileManager.default.temporaryDirectory.appendingPathComponent("exact-share-test-\(UUID().uuidString)")
        defer { try? FileManager.default.removeItem(at: dir) }
        let first = try ShareSheet.scratchFile(dir, 0, ShareSheet.pictureName("card.one"))
        let second = try ShareSheet.scratchFile(dir, 1, ShareSheet.pictureName("card-one"))
        try Data("one".utf8).write(to: first)
        try picture.png.write(to: second)
        XCTAssertNotEqual(first, second)
        XCTAssertEqual(first.lastPathComponent, "card-one.png")
        XCTAssertEqual(try Data(contentsOf: first), Data("one".utf8))
        XCTAssertEqual(try Data(contentsOf: second), picture.png)
        // With a title, the picture is named by it, as the sheet's header shows it.
        XCTAssertEqual(ShareSheet.pictureName("share-card", title: "Four on the Floor"), "Four on the Floor.png")
        XCTAssertEqual(ShareSheet.pictureName("share-card", title: " A/B: 2 "), "A-B- 2.png")
        XCTAssertEqual(ShareSheet.pictureName("share-card", title: "  "), "share-card.png")
        // A long title is cut at a character to fit APFS's 255 bytes, `.png` included.
        for title in [String(repeating: "鼓", count: 100), String(repeating: "🥁", count: 100), String(repeating: "👩🏽‍🎤", count: 40)] {
            let name = ShareSheet.pictureName("share-card", title: title)
            XCTAssertLessThanOrEqual(name.utf8.count, 255, name)
            XCTAssertTrue(name.hasSuffix(".png"))
            XCTAssertTrue(title.hasPrefix(name.dropLast(4)), "whole characters: \(name)")
            let url = try ShareSheet.scratchFile(dir, 9, name)
            XCTAssertNoThrow(try Data("x".utf8).write(to: url), name)
        }
        // A view with no area has no picture.
        p.apply(wireBatch([["op": "frame", "id": 2, "x": 0.0, "y": 0.0, "w": 0.0, "h": 50.0]]))
        XCTAssertNil(ShareSheet.picture(of: card))
    }

    static func rgba(_ image: CGImage, x: Int, y: Int) -> (UInt8, UInt8, UInt8, UInt8)? {
        var bytes = [UInt8](repeating: 0, count: 4)
        guard let context = CGContext(data: &bytes, width: 1, height: 1, bitsPerComponent: 8, bytesPerRow: 4,
                                      space: CGColorSpace(name: CGColorSpace.sRGB)!,
                                      bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue) else { return nil }
        context.draw(image, in: CGRect(x: -x, y: y - image.height + 1, width: image.width, height: image.height))
        return (bytes[0], bytes[1], bytes[2], bytes[3])
    }
}

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
        // So does one that invokes it by command alone.
        p.views[5]!.props = ["commandfor": "more", "command": "toggle-popover"]
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
