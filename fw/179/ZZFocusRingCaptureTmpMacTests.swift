#if os(macOS)
import AppKit
import XCTest
@testable import ExactKit

// Evidence capture for #179, never committed: drop into
// host/apple/tests/ExactKitTests/ and run
// `FOCUS179_OUT=<dir> bun host/apple/build.mjs --test`.
// A non-activating panel becomes the key window without activating the app
// (this session can activate no app), so AppKit draws the first responder's
// ring as in any key window; CGWindowListCreateImage reads the window server's pixels.
final class ZZFocusRingCaptureTmpMacTests: XCTestCase {
    func testCaptureFocusRings() throws {
        guard let out = ProcessInfo.processInfo.environment["FOCUS179_OUT"] else { throw XCTSkip("no FOCUS179_OUT") }
        _ = NSApplication.shared
        let p = Presenter()
        p.buttonFace = { _ in ButtonFace() }
        let panel = NSPanel(contentRect: NSRect(x: 200, y: 200, width: 420, height: 240), styleMask: [.titled, .nonactivatingPanel], backing: .buffered, defer: false)
        panel.isReleasedWhenClosed = false
        panel.becomesKeyOnlyIfNeeded = false
        panel.contentView = p.viewport
        func box(_ id: Int, _ y: Double, _ w: Double, _ color: [Double], _ handlers: [String]) -> [[String: Any]] {
            var style: [String: Any] = ["background_color": color, "text_color": [0, 0, 0, 255]]
            for c in ["top_left", "top_right", "bottom_right", "bottom_left"] { style["border_radius_" + c] = 8.0 }
            return [["op": "create", "id": id, "kind": "view", "props": ["testId": "b\(id)"], "handlers": handlers, "style": style],
                    ["op": "frame", "id": id, "x": 24.0, "y": y, "w": w, "h": 38.0]]
        }
        p.apply(wireBatch(box(1, 24, 134, [232, 238, 252, 255], ["press"]) + box(2, 78, 170, [233, 246, 236, 255], ["press"]) + box(3, 132, 194, [241, 241, 241, 255], [])
                          + [["op": "create", "id": 10, "kind": "view", "props": [:], "handlers": [], "style": ["background_color": [255, 255, 255, 255]]],
                             ["op": "frame", "id": 10, "x": 0.0, "y": 0.0, "w": 420.0, "h": 240.0],
                             ["op": "children", "id": 10, "ids": [1, 2, 3]], ["op": "roots", "ids": [10]]]))
        panel.makeKeyAndOrderFront(nil)
        p.flushKeyViewLoop()
        let settle = { RunLoop.main.run(until: Date().addingTimeInterval(0.6)) }
        settle()
        let capture = { (name: String) in
            settle()
            let id = CGWindowID(panel.windowNumber)
            typealias F = @convention(c) (CGRect, UInt32, CGWindowID, UInt32) -> Unmanaged<CGImage>?
            let sym = dlsym(UnsafeMutableRawPointer(bitPattern: -2), "CGWindowListCreateImage")!
            let image = unsafeBitCast(sym, to: F.self)(.null, 1 << 3, id, 1 << 0 | 1 << 3)!.takeRetainedValue()
            let rep = NSBitmapImageRep(cgImage: image)
            try? rep.representation(using: .png, properties: [:])!.write(to: URL(fileURLWithPath: out + "/" + name + ".png"))
            print("FOCUS179 \(name) key=\(panel.isKeyWindow) responder=\((panel.firstResponder as? NodeView)?.id ?? 0) mask1=\(p.views[1]!.focusRingMaskBounds) mask2=\(p.views[2]!.focusRingMaskBounds)")
        }
        let tab = NSEvent.keyEvent(with: .keyDown, location: .zero, modifierFlags: [], timestamp: 0, windowNumber: panel.windowNumber, context: nil, characters: "\t", charactersIgnoringModifiers: "\t", isARepeat: false, keyCode: 48)!
        // 1. A Tab into the page: the first custom button.
        _ = p.routeKey(tab, focused: true, in: panel)
        panel.makeFirstResponder(p.views[1])
        capture("1-tab-first")
        // 2. Tab on: the `box press=`.
        _ = p.routeKey(tab, focused: true, in: panel)
        p.views[1]!.keyDown(with: tab)
        capture("2-tab-second")
        // 3. The box grows while focused: the ring follows.
        p.apply(wireBatch([["op": "frame", "id": 2, "x": 24.0, "y": 78.0, "w": 260.0, "h": 38.0]]))
        capture("3-resized")
        // 4. A click on the first: focused, no ring (Chrome's :focus-visible).
        let at = p.views[1]!.convert(NSPoint(x: 20, y: 20), to: nil)
        let down = NSEvent.mouseEvent(with: .leftMouseDown, location: at, modifierFlags: [], timestamp: 0, windowNumber: panel.windowNumber, context: nil, eventNumber: 0, clickCount: 1, pressure: 1)!
        p.views[1]!.mouseDown(with: down)
        capture("4-click-first")
        panel.close()
    }
}
#endif
