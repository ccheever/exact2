#if os(macOS)
import AppKit
import XCTest
@testable import ExactKit

final class VideoVisibilityTests: XCTestCase {
    func testNestedClippingUsesAreaAndActualScrollOffsets() {
        _ = NSApplication.shared
        let window = NSWindow(contentRect: NSRect(x: 0, y: 0, width: 300, height: 300),
                              styleMask: [.borderless], backing: .buffered, defer: false)
        window.isReleasedWhenClosed = false
        defer { window.close() }
        let root = NSView(frame: NSRect(x: 0, y: 0, width: 300, height: 300))
        window.contentView = root
        let clip = NSClipView(frame: NSRect(x: 0, y: 0, width: 200, height: 200))
        let document = NSView(frame: NSRect(x: 0, y: 0, width: 600, height: 600))
        let target = NSView(frame: NSRect(x: 0, y: 0, width: 200, height: 200))
        root.addSubview(clip); clip.documentView = document; document.addSubview(target)
        clip.scroll(to: .zero)
        XCTAssertEqual(VideoVisibilityHost.fraction(target), 1, accuracy: 0.001)
        clip.scroll(to: NSPoint(x: 100, y: 100))
        XCTAssertEqual(clip.bounds.origin, NSPoint(x: 100, y: 100))
        XCTAssertEqual(VideoVisibilityHost.fraction(target), 0.25, accuracy: 0.001)
        clip.scroll(to: NSPoint(x: 200, y: 100))
        XCTAssertEqual(VideoVisibilityHost.fraction(target), 0, accuracy: 0.001)
        clip.scroll(to: .zero)
        target.isHidden = true
        XCTAssertEqual(VideoVisibilityHost.fraction(target), 0)
        target.isHidden = false
        XCTAssertEqual(VideoVisibilityHost.fraction(target), 1, accuracy: 0.001)
        target.removeFromSuperview()
        XCTAssertEqual(VideoVisibilityHost.fraction(target), 0)
    }
}
#endif
