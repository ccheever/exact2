#if os(macOS)
import AppKit
import SwiftUI
import XCTest
@testable import ExactKit

final class SheetSurfaceMacTests: XCTestCase {
    private func capture(_ window: NSWindow, name: String) throws -> NSBitmapImageRep {
        window.contentView?.layoutSubtreeIfNeeded()
        window.displayIfNeeded()
        CATransaction.flush()
        typealias Create = @convention(c) (CGRect, UInt32, UInt32, UInt32) -> Unmanaged<CGImage>?
        let symbol = try XCTUnwrap(dlsym(UnsafeMutableRawPointer(bitPattern: -2), "CGWindowListCreateImage"))
        let image = try XCTUnwrap(unsafeBitCast(symbol, to: Create.self)(.null, 1 << 3, UInt32(window.windowNumber), 1 << 0)?.takeRetainedValue())
        let rep = NSBitmapImageRep(cgImage: image)
        try rep.representation(using: .png, properties: [:])?.write(to: URL(fileURLWithPath: "/tmp/b2-sheet-\(name).png"))
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
        let expected = commonColours(try capture(swiftSheet, name: "swiftui"))

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
        let picture = try capture(actualSheet, name: "exact")
        let actual = commonColours(picture)
        let top = try XCTUnwrap(picture.colorAt(x: picture.pixelsWide / 2, y: 30)?.usingColorSpace(.sRGB))
        let topRGB = [top.redComponent, top.greenComponent, top.blueComponent].map { Int(($0 * 255).rounded()) }
        print("SwiftUI sheet colours: \(expected); Exact: \(actual); authored ground: \(topRGB)")
        try XCTExpectFailure("QUEUE 1569: authored system backgrounds and grouped cards use the window surface in a sheet") {
            XCTAssertTrue(zip(topRGB, expected[0]).allSatisfy { abs($0 - $1) <= 3 }, "authored ground \(topRGB) differs from sheet \(expected[0])")
            for colour in expected.prefix(2) {
                XCTAssertTrue(actual.contains { zip($0, colour).allSatisfy { abs($0 - $1) <= 3 } }, "SwiftUI's surface \(colour) absent from \(actual)")
            }
            XCTAssertTrue(try f.node("ground").materialView is NSVisualEffectView)
        }
    }
}
#endif
