// LLP 1115.000 D10.3: the display's scale is a layout input the presenter
// reads from the platform and tells the kernel, as it tells the root font
// size: a hairline (`-exact-hairline`) is one device pixel of it.
#if os(macOS)
import AppKit
#else
import UIKit
#endif
import XCTest
@testable import ExactKit

final class DisplayScaleTests: XCTestCase {
    override func tearDown() {
        PageFacts.agentDisplayScale = nil
        super.tearDown()
    }

    /// The scale told is the display's the view is on: AppKit's
    /// `backingScaleFactor` (a point is a pixel for a view in no window yet),
    /// UIKit's `displayScale`. A drive's `prefer display-scale` stands in for
    /// it and gives it back.
    func testTheScaleIsTheDisplaysTheViewIsOn() {
        #if os(macOS)
        let window = NSWindow(contentRect: NSRect(x: 0, y: 0, width: 200, height: 200), styleMask: [.titled], backing: .buffered, defer: false)
        window.isReleasedWhenClosed = false
        let view = NSView(frame: NSRect(x: 0, y: 0, width: 200, height: 200))
        window.contentView = view
        XCTAssertEqual(PageFacts.displayScale(view), Double(window.backingScaleFactor))
        XCTAssertEqual(PageFacts.displayScale(NSView()), 1, "no window yet")
        #else
        let view = UIView()
        XCTAssertEqual(PageFacts.displayScale(view), Double(view.traitCollection.displayScale))
        #endif
        XCTAssertGreaterThanOrEqual(PageFacts.displayScale(view), 1)
        XCTAssertEqual(PageFacts.displayScale(nil), 1, "no view: a point is a pixel")
        PageFacts.agentDisplayScale = 3
        XCTAssertEqual(PageFacts.displayScale(view), 3, "the drive's stands in")
        PageFacts.agentDisplayScale = nil
        XCTAssertGreaterThanOrEqual(PageFacts.displayScale(view), 1, "and is given back")
    }
}
