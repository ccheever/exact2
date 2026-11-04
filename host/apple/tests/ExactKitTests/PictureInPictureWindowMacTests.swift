// @ref LLP 1042 §9.1 — Exact's own picture-in-picture window: a floating,
// non-activating panel on every Space, locked to the picture's aspect; the
// browser's kind at the bottom right with its controls shown on hover, placed
// as Chromium places it and following the next image that asks, the titled
// kind where a new window opens; its controls and its close are the image's
// leave.
#if os(macOS)
import AppKit
import XCTest
@testable import ExactKit

final class PictureInPictureWindowMacTests: XCTestCase {
    private var window: PictureInPictureWindow?
    private var said: [String] = []
    private var files: [URL] = []

    override func setUp() { PictureInPictureWindow.lastFrame = nil }

    override func tearDown() {
        window?.close(); window = nil
        files.forEach { try? FileManager.default.removeItem(at: $0) }
    }

    /// A PNG of `width` by `height` pixels in one colour.
    private func picture(_ width: Int, _ height: Int, _ color: NSColor = .systemRed) -> String {
        let rep = NSBitmapImageRep(bitmapDataPlanes: nil, pixelsWide: width, pixelsHigh: height, bitsPerSample: 8, samplesPerPixel: 4,
                                   hasAlpha: true, isPlanar: false, colorSpaceName: .deviceRGB, bytesPerRow: 0, bitsPerPixel: 0)!
        NSGraphicsContext.saveGraphicsState()
        NSGraphicsContext.current = NSGraphicsContext(bitmapImageRep: rep)
        color.setFill(); NSRect(x: 0, y: 0, width: width, height: height).fill()
        NSGraphicsContext.restoreGraphicsState()
        let url = FileManager.default.temporaryDirectory.appendingPathComponent("pip-\(UUID().uuidString).png")
        try! rep.representation(using: .png, properties: [:])!.write(to: url)
        files.append(url)
        return url.path
    }

    private func open(panel: Bool, width: Double? = nil, height: Double? = nil, picture path: String) -> PictureInPictureWindow {
        _ = NSApplication.shared
        let window = PictureInPictureWindow(panel: panel, title: "Preview · Page", width: width, height: height) { [weak self] in self?.said.append($0) }
        self.window = window
        window.frame(path)
        spin { self.said.contains("enter") }
        return window
    }

    private func spin(_ done: () -> Bool) {
        let deadline = Date(timeIntervalSinceNow: 5)
        while !done() && Date() < deadline { RunLoop.main.run(mode: .default, before: Date(timeIntervalSinceNow: 0.01)) }
    }

    func testTheBrowsersWindowFloatsAtTheBottomRightOfTheScreen() throws {
        let window = open(panel: false, picture: picture(922, 1280))
        XCTAssertEqual(said, ["enter"])
        let panel = window.panel
        XCTAssertTrue(panel.isVisible)
        XCTAssertEqual(panel.level, .floating)
        XCTAssertTrue(panel.collectionBehavior.contains(.canJoinAllSpaces))
        XCTAssertTrue(panel.collectionBehavior.contains(.fullScreenAuxiliary))
        XCTAssertTrue(panel.styleMask.contains(.nonactivatingPanel))
        XCTAssertTrue(panel.styleMask.contains(.resizable))
        XCTAssertFalse(panel.styleMask.contains(.titled))
        XCTAssertEqual(panel.contentAspectRatio.width / panel.contentAspectRatio.height, 922 / 1280, accuracy: 0.001)
        let screen = try XCTUnwrap(panel.screen?.visibleFrame)
        let size = PictureInPictureWindow.chromiumSize(NSSize(width: 922, height: 1280), screen: screen.size)
        XCTAssertEqual(panel.frame.size, size)
        // Measured beside T3 Code v0.1.6 on a 1352 × 849-point visible frame.
        XCTAssertEqual(PictureInPictureWindow.chromiumSize(NSSize(width: 1600, height: 1000), screen: NSSize(width: 1352, height: 849)), NSSize(width: 284, height: 178))
        XCTAssertEqual(PictureInPictureWindow.chromiumSize(NSSize(width: 922, height: 1224), screen: NSSize(width: 1352, height: 849)), NSSize(width: 284, height: 377))
        XCTAssertEqual(panel.frame, PictureInPictureWindow.chromiumFrame(size, topLeft: nil, screen: screen))
        XCTAssertEqual(window.report["controls"] as? [String], ["minimize", "back", "close"])
        XCTAssertEqual(window.report["controlsShown"] as? Bool, false)
        XCTAssertNil(window.press("hover"))
        XCTAssertEqual(window.report["controlsShown"] as? Bool, true)
        let labels = Set(allViews(panel.contentView!).compactMap { ($0 as? NSButton)?.accessibilityLabel() })
        XCTAssertEqual(labels, ["Minimize", "Back to tab", "Close"])
        XCTAssertEqual(window.press("rewind"), "the window has no control rewind: minimize, back, close or hover")
        XCTAssertEqual(panel.minSize, panel.frame.size, "no smaller than it opened")
    }

    func testEachControlIsTheImagesLeave() {
        for control in ["minimize", "back", "close"] {
            said = []
            let window = open(panel: false, picture: picture(400, 300))
            XCTAssertNil(window.press(control))
            XCTAssertEqual(said, ["enter", "leave"], control)
            window.close()
        }
    }

    func testTheWindowIsPlacedAsChromiumPlacesIt() {
        // Measured beside T3 Code v0.1.6 on a 1352 × 849-point visible frame (878 points high, from the top
        // left: 1051, 683 and 1053, 486), and a window moved to 1051, 683 that takes a portrait picture.
        let screen = NSRect(x: 0, y: 0, width: 1352, height: 849)
        XCTAssertEqual(PictureInPictureWindow.chromiumFrame(NSSize(width: 284, height: 178), topLeft: nil, screen: screen), NSRect(x: 1051, y: 17, width: 284, height: 178))
        XCTAssertEqual(PictureInPictureWindow.chromiumFrame(NSSize(width: 284, height: 377), topLeft: nil, screen: screen), NSRect(x: 1053, y: 15, width: 284, height: 377))
        XCTAssertEqual(PictureInPictureWindow.chromiumFrame(NSSize(width: 284, height: 377), topLeft: NSPoint(x: 1051, y: 195), screen: screen), NSRect(x: 1051, y: 15, width: 284, height: 377))
        XCTAssertEqual(PictureInPictureWindow.chromiumFrame(NSSize(width: 400, height: 531), topLeft: NSPoint(x: 700, y: 728), screen: screen), NSRect(x: 700, y: 197, width: 400, height: 531))
        XCTAssertEqual(PictureInPictureWindow.width(400, aspect: NSSize(width: 922, height: 1224), max: NSSize(width: 1014, height: 637)), NSSize(width: 400, height: 531))
        XCTAssertEqual(PictureInPictureWindow.width(600, aspect: NSSize(width: 922, height: 1224), max: NSSize(width: 1014, height: 637)), NSSize(width: 480, height: 637))
    }

    func testTheNextWindowOpensWhereTheLastClosedAtItsWidth() throws {
        let first = open(panel: false, picture: picture(400, 300))
        first.panel.setFrame(NSRect(x: 100, y: 400, width: 400, height: 300), display: false)
        first.close()
        said = []
        let next = open(panel: false, picture: picture(400, 300))
        XCTAssertEqual(next.panel.frame, NSRect(x: 100, y: 400, width: 400, height: 300))
        next.close()
        said = []
        let portrait = open(panel: false, picture: picture(300, 400))
        let screen = try XCTUnwrap(portrait.panel.screen?.visibleFrame)
        XCTAssertEqual(portrait.panel.frame, PictureInPictureWindow.chromiumFrame(NSSize(width: 400, height: 533), topLeft: NSPoint(x: 100, y: 700), screen: screen))
    }

    func testTheWindowFollowsTheNextImageWhereItStands() throws {
        let window = open(panel: false, picture: picture(400, 300))
        window.panel.setFrame(NSRect(x: 120, y: 300, width: 400, height: 300), display: false)
        var next: [String] = []
        window.follow(title: "t3code://next") { next.append($0) }
        window.frame(picture(300, 400, .systemBlue))
        spin { next.contains("enter") }
        XCTAssertEqual(said, ["enter"], "the image before hears nothing more from the window")
        XCTAssertEqual(next, ["enter"])
        XCTAssertTrue(window.isOpen)
        XCTAssertEqual(window.report["title"] as? String, "t3code://next")
        let screen = try XCTUnwrap(window.panel.screen?.visibleFrame)
        XCTAssertEqual(window.panel.frame, PictureInPictureWindow.chromiumFrame(NSSize(width: 400, height: 533), topLeft: NSPoint(x: 120, y: 600), screen: screen))
        XCTAssertEqual(window.panel.contentAspectRatio.width / window.panel.contentAspectRatio.height, 0.75, accuracy: 0.001)
        XCTAssertNil(window.press("close"))
        XCTAssertEqual(next, ["enter", "leave"])
    }

    func testTheAskedSizeKeepsItsAreaAtThePicturesAspect() {
        XCTAssertEqual(PictureInPictureWindow.fit(NSSize(width: 480, height: 320), aspect: NSSize(width: 300, height: 400)), NSSize(width: 339, height: 453))
        XCTAssertEqual(PictureInPictureWindow.fit(NSSize(width: 480, height: 320), aspect: NSSize(width: 480, height: 320)), NSSize(width: 480, height: 320))
    }

    func testThePanelIsTitledAndOpensWhereANewWindowOpens() throws {
        let window = open(panel: true, width: 480, height: 320, picture: picture(1600, 900))
        let panel = window.panel
        XCTAssertTrue(panel.styleMask.contains(.titled))
        XCTAssertTrue(panel.styleMask.contains(.nonactivatingPanel))
        XCTAssertEqual(panel.title, "Preview · Page")
        // Electron's 480 × 320 is the window's: its content takes the picture's aspect at the same area.
        let content = panel.contentRect(forFrameRect: NSRect(x: 0, y: 0, width: 480, height: 320)).size
        XCTAssertEqual(panel.contentRect(forFrameRect: panel.frame).size, PictureInPictureWindow.fit(content, aspect: NSSize(width: 1600, height: 900)))
        XCTAssertEqual(panel.minSize, NSSize(width: 240, height: 160))
        let screen = try XCTUnwrap(panel.screen?.visibleFrame)
        XCTAssertEqual(panel.frame.minX, (screen.midX - 240).rounded(), accuracy: 1)
        XCTAssertEqual(window.report["controls"] as? [String], ["close"])
        panel.performClose(nil)
        XCTAssertEqual(said, ["enter", "leave"])
        XCTAssertTrue(panel.isVisible, "the app closes it, as the image hears it leave")
    }

    func testAnUnreadablePictureFailsTheRequest() {
        _ = NSApplication.shared
        let bad = FileManager.default.temporaryDirectory.appendingPathComponent("pip-\(UUID().uuidString).png")
        try! Data("not a picture".utf8).write(to: bad)
        files.append(bad)
        let window = PictureInPictureWindow(panel: false, title: "T", width: nil, height: nil) { [weak self] in self?.said.append($0) }
        self.window = window
        window.frame(bad.path)
        spin { !self.said.isEmpty }
        XCTAssertEqual(said, ["error the picture could not be read"])
        XCTAssertFalse(window.panel.isVisible)
    }

    private func allViews(_ view: NSView) -> [NSView] { [view] + view.subviews.flatMap(allViews) }
}
#endif
