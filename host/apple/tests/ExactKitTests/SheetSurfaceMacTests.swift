#if os(macOS)
import AppKit
import SwiftUI
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

    private func commonColours(_ rep: NSBitmapImageRep) -> [[Int]] {
        var counts: [[Int]: Int] = [:]
        for y in stride(from: 16, to: rep.pixelsHigh - 16, by: 2) {
            for x in stride(from: 16, to: rep.pixelsWide - 16, by: 2) {
                guard let c = rep.colorAt(x: x, y: y)?.usingColorSpace(.sRGB) else { continue }
                let rgb = [c.redComponent, c.greenComponent, c.blueComponent].map { Int(($0 * 255).rounded()) }
                counts[rgb, default: 0] += 1
            }
        }
        return counts.sorted { $0.value > $1.value }.prefix(4).map(\.key)
    }

    private func measuredColours(_ rep: NSBitmapImageRep) throws -> [[Int]] {
        let colours = commonColours(rep)
        guard colours.count >= 2, let ground = colours.first, ground.contains(where: { $0 > 8 }), ground.contains(where: { $0 < 247 }) else {
            throw XCTSkip("window-server capture is blank; sheet surfaces cannot be measured")
        }
        return colours
    }

    func testCaptureValidationRejectsBlankImages() throws {
        let space = try XCTUnwrap(CGColorSpace(name: CGColorSpace.sRGB))
        let context = try XCTUnwrap(CGContext(data: nil, width: 64, height: 64, bitsPerComponent: 8, bytesPerRow: 256,
                                              space: space, bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue))
        for blank: CGFloat in [0, 1] {
            context.setFillColor(CGColor(colorSpace: space, components: [blank, blank, blank, 1])!)
            context.fill(CGRect(x: 0, y: 0, width: 64, height: 64))
            let rep = NSBitmapImageRep(cgImage: try XCTUnwrap(context.makeImage()))
            XCTAssertThrowsError(try measuredColours(rep)) { XCTAssertTrue($0 is XCTSkip) }
        }
        for (x, shade): (Int, CGFloat) in [(0, 0.15), (32, 0.25)] {
            context.setFillColor(CGColor(colorSpace: space, components: [shade, shade, shade, 1])!)
            context.fill(CGRect(x: x, y: 0, width: 32, height: 64))
        }
        XCTAssertEqual(try measuredColours(NSBitmapImageRep(cgImage: XCTUnwrap(context.makeImage()))).count, 2)
    }

    func testAuthoredGroundAndGroupedCardMatchASwiftUISheetInDarkMode() throws {
        _ = NSApplication.shared
        let reference = NSWindow(contentRect: NSRect(x: 100, y: 100, width: 600, height: 500), styleMask: [.titled], backing: .buffered, defer: false)
        reference.isReleasedWhenClosed = false
        reference.appearance = NSAppearance(named: .darkAqua)
        reference.contentView = NSHostingView(rootView: Color.clear.frame(width: 600, height: 500).sheet(isPresented: .constant(true)) {
            Form { Section { Text("A row"); Text("Another row") } }
                .formStyle(.grouped).frame(width: 400, height: 300)
        })
        reference.orderFront(nil)
        defer { if let sheet = reference.attachedSheet { reference.endSheet(sheet) }; reference.close() }
        let limit = Date().addingTimeInterval(5)
        while reference.attachedSheet == nil && Date() < limit { RunLoop.main.run(mode: .default, before: Date().addingTimeInterval(0.05)) }
        let swiftSheet = try XCTUnwrap(reference.attachedSheet)
        let expected = try measuredColours(try capture(swiftSheet))

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
                  box height=60 background-color="-exact-background" testId="ground"
                  list appearance="auto" listStyle="inset-grouped" flex=1 min-height=0
                    section
                      text "A row"
                      text "Another row"
        """)
        defer { f.close() }
        f.window.appearance = NSAppearance(named: .darkAqua)
        f.window.orderFront(nil)
        try f.press("open")
        let actualSheet = try XCTUnwrap(f.window.attachedSheet)
        let ground = try f.node("ground")
        let material = try XCTUnwrap(ground.materialView as? NSVisualEffectView)
        XCTAssertEqual(material.material, .sheet)
        XCTAssertEqual(material.blendingMode, .behindWindow)
        XCTAssertNil(try f.node("modal").materialView)
        let picture = try capture(actualSheet)
        let actual = try measuredColours(picture)
        let top = try XCTUnwrap(picture.colorAt(x: picture.pixelsWide / 2, y: 30)?.usingColorSpace(.sRGB))
        let topRGB = [top.redComponent, top.greenComponent, top.blueComponent].map { Int(($0 * 255).rounded()) }
        print("SwiftUI sheet colours: \(expected); Exact: \(actual); authored ground: \(topRGB)")
        XCTAssertTrue(zip(topRGB, expected[0]).allSatisfy { abs($0 - $1) <= 3 }, "authored ground \(topRGB) differs from sheet \(expected[0])")
        for colour in expected.prefix(2) {
            XCTAssertTrue(actual.contains { zip($0, colour).allSatisfy { abs($0 - $1) <= 3 } }, "SwiftUI's surface \(colour) absent from \(actual)")
        }
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
