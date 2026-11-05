#if os(macOS)
import AppKit
import XCTest
@testable import ExactKit

final class SheetSurfaceMacTests: XCTestCase {
    private func capture(_ window: NSWindow) throws -> NSBitmapImageRep {
        window.contentView?.layoutSubtreeIfNeeded()
        window.displayIfNeeded()
        CATransaction.flush()
        typealias Create = @convention(c) (CGRect, UInt32, UInt32, UInt32) -> Unmanaged<CGImage>?
        let symbol = try XCTUnwrap(dlsym(UnsafeMutableRawPointer(bitPattern: -2), "CGWindowListCreateImage"))
        let image = try XCTUnwrap(unsafeBitCast(symbol, to: Create.self)(.null, 1 << 3, UInt32(window.windowNumber), 1 << 0)?.takeRetainedValue())
        let rep = NSBitmapImageRep(cgImage: image)
        return rep
    }

    /// The colour of the pixel at row `y` of a capture's middle column
    private func colour(_ rep: NSBitmapImageRep, y: Int) throws -> [Int] {
        let c = try XCTUnwrap(rep.paintedColorAt(x: rep.pixelsWide / 2, y: y)?.usingColorSpace(.sRGB))
        return [c.redComponent, c.greenComponent, c.blueComponent].map { Int(($0 * 255).rounded()) }
    }

    func testAuthoredGroundMatchesAnAppKitSheetInDarkMode() throws {
        let f = try SheetFixture(page: """
        component Proof
          state showing = false
          action open
            showing = true
          view
            column navigationKey=(showing ? "modal" : "home") navigationBack="back" width="100%" height="100%"
              column navigationKey="home"
                button testId="open" press=open
                  text "Open"
              if showing
                column navigationKey="modal" navigationPresentation="modal" width=400 height=300 background-color="-exact-background" testId="modal"
                  box background-color="#ff0000"
                    box height=60 background-color="-exact-background" testId="ground"
        """)
        defer { f.close() }
        f.window.appearance = NSAppearance(named: .darkAqua)
        f.window.orderFront(nil)
        // Open AppKit's own sheet on the same window, at Exact's size and in
        // the window's appearance, since a sheet's ground is a material that
        // blends what lies behind it
        let reference = NSWindow(contentRect: NSRect(x: 0, y: 0, width: 400, height: 300), styleMask: [.titled], backing: .buffered, defer: false)
        reference.isReleasedWhenClosed = false
        reference.appearanceSource = f.window
        f.window.beginSheet(reference)
        XCTAssertTrue(f.window.attachedSheet === reference)
        let picture = try capture(reference)
        f.window.endSheet(reference)
        let expected = try colour(picture, y: 30)
        try XCTSkipUnless(expected.contains { $0 > 8 } && expected.contains { $0 < 247 }, "window-server capture is blank; sheet surfaces cannot be measured")

        try f.press("open")
        let ground = try f.node("ground")
        let actualSheet = try XCTUnwrap(ground.window)
        let material = try XCTUnwrap(ground.materialView as? NSVisualEffectView)
        XCTAssertEqual(material.material, .sheet)
        XCTAssertEqual(material.blendingMode, .behindWindow)
        XCTAssertNil(try f.node("modal").materialView)
        XCTAssertEqual(try colour(try capture(actualSheet), y: 30), expected, "the authored ground, over red, is the colour of AppKit's sheet")
    }

    func testGroupedCardFillUsesItsWindowSurface() throws {
        let f = try SheetFixture(page: """
        component Proof
          view
            column width="100%" height="100%"
              list appearance="auto" listStyle="inset-grouped" width=300 height=120
                section
                  text "Window row" testId="window-row"
              button testId="open" commandfor="sheet" command="show-modal"
                text "Open"
              dialog id="sheet" width=400 height=300
                list appearance="auto" listStyle="inset-grouped" width=300 height=120
                  section
                    text "Sheet row" testId="sheet-row"
        """)
        defer { f.close() }
        f.window.appearance = NSAppearance(named: .darkAqua)
        func card(above id: String) throws -> NodeView {
            var view = try f.node(id).superview
            while let at = view {
                if let node = at as? NodeView, node.style["background_color"] != nil { return node }
                view = at.superview
            }
            throw NSError(domain: "missing grouped card", code: 1)
        }
        let windowCard = try card(above: "window-row")
        XCTAssertEqual(windowCard.channels("background_color", dark: true), [39, 39, 39, 255])
        XCTAssertEqual(windowCard.cgColor("background_color", dark: true)?.alpha, 1)
        try f.press("open")
        let sheetCard = try card(above: "sheet-row")
        XCTAssertTrue(sheetCard.window is SheetWindow)
        let sheet = try XCTUnwrap(sheetCard.channels("background_color", dark: true))
        XCTAssertEqual(Array(sheet.prefix(3)), [255, 255, 255])
        XCTAssertEqual(sheet[3] / 255, 0.03, accuracy: 0.002)
        XCTAssertEqual(windowCard.channels("background_color", dark: true), [39, 39, 39, 255], "opening a sheet must not recolour the window's card")
        XCTAssertEqual(windowCard.channels("background_color", dark: false), [249, 249, 249, 255])
    }

    func testAnAuthoredLiteralReplacesTheSheetMaterial() throws {
        let f = try SheetFixture(page: """
        component Proof
          state showing = false
          state literal = false
          action open
            showing = true
          action recolour
            literal = true
          view
            column navigationKey=(showing ? "modal" : "home") navigationBack="back" width="100%" height="100%"
              column navigationKey="home"
                button testId="open" press=open
                  text "Open"
              if showing
                column navigationKey="modal" navigationPresentation="modal" width=400 height=300
                  box background-color=(literal ? "#123456" : "-exact-background") testId="ground"
                    button testId="recolour" press=recolour
                      text "Use a literal colour"
        """)
        defer { f.close() }
        try f.press("open")
        let ground = try f.node("ground")
        XCTAssertEqual((ground.materialView as? NSVisualEffectView)?.material, .sheet)
        try f.press("recolour")
        XCTAssertNil(ground.materialView)
        XCTAssertEqual(ground.channels("background_color", dark: true), [18, 52, 86, 255])
    }
}
#endif
