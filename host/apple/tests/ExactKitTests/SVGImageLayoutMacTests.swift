#if os(macOS)
import AppKit
import XCTest
@testable import ExactKit

/// The SVG worker's viewport and cache follow applied CSS layout, including
/// a content-box update with no frame change (#239, #515).
final class SVGImageLayoutMacTests: XCTestCase {
    private static let plan: Result<Data, Error> = Result {
        let root = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
        try FileManager.default.createDirectory(at: root, withIntermediateDirectories: true)
        defer { try? FileManager.default.removeItem(at: root) }
        let svg = "<svg xmlns='http://www.w3.org/2000/svg' width='100' height='100' viewBox='0 0 100 100'><circle cx='50' cy='50' r='30' fill='red'/></svg>"
        let image = "data:image/svg+xml;base64," + Data(svg.utf8).base64EncodedString()
        let unsized = "<svg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 100 100'><circle cx='50' cy='50' r='30' fill='red'/></svg>"
        let ratioOnly = "data:image/svg+xml;base64," + Data(unsized.utf8).base64EncodedString()
        let source = """
        component App
          state wide = false
          state padded = false
          state contained = false
          action widen
            wide = true
          action pad
            padded = true
          action contain
            contained = true
          view
            column width=600
              column width=(wide ? 600 : 400)
                image "\(image)" testId="relative" width=200 height=100 box-sizing="border-box" padding-left="10%" padding-right="calc(5% + 10px)"
              image "\(image)" testId="equivalent" width=130 height=100
              image "\(image)" testId="changing" width=200 height=100 box-sizing="border-box" padding-left=(padded ? 20 : 0)
              image "\(image)" testId="fitting" width=200 height=100 object-fit=(contained ? "contain" : "fill")
              row
                image "\(ratioOnly)" testId="none" width=400 height=400 flex-shrink=0 object-fit="none"
                image "\(ratioOnly)" testId="scale-down" width=400 height=400 flex-shrink=0 object-fit="scale-down"
              button "Widen" testId="widen" press=widen
              button "Pad" testId="pad" press=pad
              button "Contain" testId="contain" press=contain

        """
        let contract = root.appendingPathComponent("app.contract"), plan = root.appendingPathComponent("app.plan")
        try source.write(to: contract, atomically: true, encoding: .utf8)
        let compiler = Process()
        compiler.executableURL = URL(fileURLWithPath: try XCTUnwrap(ProcessInfo.processInfo.environment["EXACT_CONTRACT"]))
        compiler.arguments = ["build", contract.path, "-o", plan.path]
        try compiler.run(); compiler.waitUntilExit()
        XCTAssertEqual(compiler.terminationStatus, 0)
        return try Data(contentsOf: plan)
    }

    private func withLayout(_ body: (ExactSession, (String) throws -> NodeView, NSWindow) throws -> Void) throws {
        _ = NSApplication.shared
        let session = ExactApp.shared.makeSession(label: "svg-content-box")
        let window = NSWindow(contentRect: CGRect(x: 0, y: 0, width: 600, height: 400), styleMask: [.borderless], backing: .buffered, defer: false)
        window.isReleasedWhenClosed = false
        window.contentView = session.presenter.viewport
        defer { session.destroy(); window.close() }
        XCTAssertNil(session.boot(plan: try Self.plan.get(), size: CGSize(width: 600, height: 400)).error)
        func node(_ testId: String) throws -> NodeView {
            try XCTUnwrap(session.presenter.views.values.first { $0.props["testId"] == testId })
        }
        try body(session, node, window)
    }

    private func settle(_ window: NSWindow, file: StaticString = #filePath, line: UInt = #line, _ predicate: () -> Bool) {
        let deadline = Date(timeIntervalSinceNow: 300)
        while !predicate(), Date() < deadline {
            window.contentView?.layoutSubtreeIfNeeded()
            RunLoop.main.run(mode: .default, before: Date(timeIntervalSinceNow: 0.01))
        }
        XCTAssertTrue(predicate(), file: file, line: line)
    }

    func testPercentageAndCalcPaddingUseResolvedViewportAndShareOnlyEquivalentDecode() throws {
        try withLayout { session, node, window in
            let relative = try node("relative"), equivalent = try node("equivalent"), plain = try node("changing")
            settle(window) { relative.raster != nil && equivalent.raster != nil && plain.raster != nil }
            XCTAssertEqual(relative.contentBox(), CGRect(x: 40, y: 0, width: 130, height: 100))
            let image = try XCTUnwrap(relative.raster?.image)
            XCTAssertEqual(Double(image.image.width) / Double(image.image.height), 1.3, accuracy: 0.01)
            XCTAssertTrue(image === equivalent.raster?.image, "equal concrete viewports share the decoded SVG")
            XCTAssertFalse(image === plain.raster?.image, "the border box alone cannot identify an SVG decode")
            let frame = relative.frame
            let batch = session.runtime.press(try node("widen").id, now: session.now())
            XCTAssertTrue(batch.ops.contains { $0.op == .fieldContent && $0.id == relative.id })
            XCTAssertFalse(batch.ops.contains { $0.op == .frame && $0.id == relative.id })
            session.apply(batch)
            settle(window) { relative.raster != nil && relative.raster?.image !== image }
            XCTAssertEqual(relative.frame, frame)
            XCTAssertEqual(relative.contentBox(), CGRect(x: 60, y: 0, width: 100, height: 100))
            XCTAssertEqual(relative.raster?.image.image.width, relative.raster?.image.image.height)
            XCTAssertTrue(equivalent.raster?.image === image, "another subscriber retains its unchanged viewport")
        }
    }

    func testPaddingOnlyUpdateAtFixedBoundsReplacesDecodedSVG() throws {
        try withLayout { session, node, window in
            let changing = try node("changing")
            settle(window) { changing.raster != nil }
            let frame = changing.frame, image = try XCTUnwrap(changing.raster?.image)
            XCTAssertEqual(Double(image.image.width) / Double(image.image.height), 2, accuracy: 0.01)
            let batch = session.runtime.press(try node("pad").id, now: session.now())
            XCTAssertTrue(batch.ops.contains { $0.op == .fieldContent && $0.id == changing.id })
            XCTAssertFalse(batch.ops.contains { $0.op == .frame && $0.id == changing.id })
            session.apply(batch)
            settle(window) { changing.raster != nil && changing.raster?.image !== image }
            XCTAssertEqual(changing.frame, frame)
            XCTAssertEqual(changing.contentBox(), CGRect(x: 20, y: 0, width: 180, height: 100))
            let updated = try XCTUnwrap(changing.raster?.image.image)
            XCTAssertEqual(Double(updated.width) / Double(updated.height), 1.8, accuracy: 0.01)
        }
    }

    /// Only `object-fit` changes: the SVG is drawn again at its new viewport,
    /// a circle staying round in the box, as a browser draws it.
    func testObjectFitChangeAloneReplacesDecodedSVG() throws {
        try withLayout { session, node, window in
            let fitting = try node("fitting")
            settle(window) { fitting.raster != nil }
            let frame = fitting.frame, image = try XCTUnwrap(fitting.raster?.image)
            XCTAssertEqual(Double(image.image.width) / Double(image.image.height), 2, accuracy: 0.01)
            let batch = session.runtime.press(try node("contain").id, now: session.now())
            XCTAssertFalse(batch.ops.contains { $0.op == .frame && $0.id == fitting.id })
            session.apply(batch)
            settle(window) { fitting.raster != nil && fitting.raster?.image !== image }
            XCTAssertEqual(fitting.frame, frame)
            let updated = try XCTUnwrap(fitting.raster?.image.image)
            XCTAssertEqual(updated.width, updated.height)
        }
    }

    /// A root with only a viewBox has a ratio and no natural size: `none`
    /// and `scale-down` contain it in the box, as Chrome shows it (a square
    /// in a 400×400 box is 400×400, not the 150×150 fallback).
    func testViewBoxOnlySVGWithNoneOrScaleDownFillsTheBoxAsChromeDoes() throws {
        try withLayout { _, node, window in
            for testId in ["none", "scale-down"] {
                let view = try node(testId)
                settle(window) { view.raster != nil }
                let image = try XCTUnwrap(view.raster?.image)
                XCTAssertEqual(image.naturalSize, CGSize(width: 150, height: 150))
                XCTAssertFalse(image.hasNaturalSize)
                let content = view.contentBox()
                XCTAssertEqual(content.size, CGSize(width: 400, height: 400))
                XCTAssertEqual(image.objectRect(content: content, fit: testId), content, testId)
                XCTAssertGreaterThanOrEqual(image.image.width, 400, "decoded for the 400-point viewport")
                XCTAssertEqual(image.image.width, image.image.height)
            }
        }
    }
}
#endif
