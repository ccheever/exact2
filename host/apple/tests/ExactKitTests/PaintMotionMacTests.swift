#if os(macOS)
import AppKit
import XCTest
@testable import ExactKit

/// Paint motion on AppKit (LLP 1062): an inline run paints the colour its
/// paragraph is told it inherits; a view whose own appearance differs from
/// the session's is noted for the host, once, until it agrees again.
final class PaintMotionMacTests: XCTestCase {
    private static let appearancePlan: Result<Data, Error> = Result {
        let directory = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
        try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
        defer { try? FileManager.default.removeItem(at: directory) }
        let rows = (0..<8).map { "          text \"Row \($0)\" testId=\"appearance-row-\($0)\" height=20 color=\"light-dark(#000000, #ffffff)\"" }.joined(separator: "\n")
        let source = """
        component App
          state shown = false
          action reveal
            shown = true
          view
            column
              button "Show" testId="appearance-reveal" press=reveal
              column testId="appearance-changing" height=10 background-color="light-dark(#ffffff, #000000)" transition="background-color 1s linear"
              if shown
                column color-scheme="dark" testId="appearance-panel"
        \(rows)

        """
        let contract = directory.appendingPathComponent("app.contract"), plan = directory.appendingPathComponent("app.plan")
        try source.write(to: contract, atomically: true, encoding: .utf8)
        let compiler = Process()
        compiler.executableURL = URL(fileURLWithPath: try XCTUnwrap(ProcessInfo.processInfo.environment["EXACT_CONTRACT"]))
        compiler.arguments = ["build", contract.path, "-o", plan.path]
        try compiler.run(); compiler.waitUntilExit()
        XCTAssertEqual(compiler.terminationStatus, 0)
        return try Data(contentsOf: plan)
    }

    private func appearanceSession(_ label: String) throws -> ExactSession {
        _ = NSApplication.shared
        let session = ExactApp.shared.makeSession(label: label)
        session.presenter.root.appearance = NSAppearance(named: .aqua)
        XCTAssertNil(session.boot(plan: try Self.appearancePlan.get(), size: CGSize(width: 390, height: 400)).error)
        session.clock = session.now()
        session.apply(session.runtime.tick(now: session.now()))
        session.scheme(dark: false)
        return session
    }

    func testMountingAnAuthoredSchemeSkipsUnchangedAppearancePasses() throws {
        let session = try appearanceSession("appearance-mount")
        defer { session.destroy() }
        let reveal = try XCTUnwrap(session.presenter.views.values.first { $0.props["testId"] == "appearance-reveal" })
        let before = session.appliedBatches
        session.apply(session.runtime.press(reveal.id, now: session.now()))
        let rows = session.presenter.views.values.filter { $0.props["testId"]?.hasPrefix("appearance-row-") == true }
        XCTAssertEqual(rows.count, 8, "the runner mounted the authored rows")
        XCTAssertTrue(rows.allSatisfy { $0.drawsDark && session.viewDark[$0.id] == true })
        XCTAssertTrue(rows.allSatisfy { $0.channels("text_color") == [255, 255, 255, 255] })
        XCTAssertEqual(session.appliedBatches, before + 1, "only the reveal applies; its unchanged appearance reports skip their passes")
    }

    func testAnAppearanceReportThatStartsMotionStillApplies() throws {
        let session = try appearanceSession("appearance-motion")
        defer { session.destroy() }
        let changing = try XCTUnwrap(session.presenter.views.values.first { $0.props["testId"] == "appearance-changing" })
        changing.appearance = NSAppearance(named: .darkAqua)
        session.noteAppearance(changing)
        session.apply(wireBatch([]))
        XCTAssertEqual(session.viewDark[changing.id], true)
        changing.appearance = NSAppearance(named: .aqua)
        session.noteAppearance(changing)
        let before = session.appliedBatches, start = session.now()
        session.apply(wireBatch([]))
        XCTAssertEqual(session.appliedBatches, before + 2, "the ordinary batch and the report that starts the transition both apply")
        XCTAssertTrue(session.frames.motion)
        session.clock = start + 500
        session.apply(session.runtime.tick(now: session.now()))
        XCTAssertEqual(try XCTUnwrap(changing.channels("background_color")?.first), 128, accuracy: 1)
        session.clock = start + 1000
        session.apply(session.runtime.tick(now: session.now()))
        XCTAssertEqual(changing.channels("background_color"), [255, 255, 255, 255])
    }

    func testAPublishedReplyLandsBeforeItsUnchangedAppearanceReports() throws {
        let session = try appearanceSession("appearance-behind-reply")
        defer { session.runtime.observeBatch = nil; session.destroy() }
        let reveal = try XCTUnwrap(session.presenter.views.values.first { $0.props["testId"] == "appearance-reveal" })
        let earlier = session.runtime.press(reveal.id, now: session.now())
        session.repliesInFlight = 1
        session.replySubmissions = [0]
        session.publish(.reply, earlier, session.generation)
        let later = session.runtime.tick(now: session.now())
        XCTAssertTrue(session.changesNothing(later))
        var reportsAfterReply: [Bool] = []
        session.runtime.observeBatch = { _, batch in
            Owner.shared.callMain {
                let rows = session.presenter.views.values.filter { $0.props["testId"]?.hasPrefix("appearance-row-") == true }
                reportsAfterReply.append(session.repliesInFlight == 0 && rows.count == 8 && batch.ops.isEmpty && batch.error == nil)
            }
        }
        let before = session.appliedBatches
        session.applyUnlessEmpty(later)
        XCTAssertEqual(session.repliesInFlight, 0)
        XCTAssertFalse(reportsAfterReply.isEmpty, "the real mounted subtree reported its appearance")
        XCTAssertTrue(reportsAfterReply.allSatisfy { $0 }, "each report follows the earlier reply's presentation")
        XCTAssertEqual(session.appliedBatches, before + 1, "only the published reply applies")
        XCTAssertEqual(session.timerDue, later.timerDueMs)
    }

    func testAnInlineRunPaintsTheColourItsParagraphIsTold() throws {
        _ = NSApplication.shared
        let session = ExactApp.shared.makeSession(label: "paint-run-mac")
        defer { session.destroy() }
        let p = session.presenter
        p.apply(wireBatch([
            ["op": "create", "id": 1, "kind": "text", "style": ["font_size": 16.0, "text_color": [0.0, 0.0, 0.0, 255.0]]],
            ["op": "paragraph", "id": 1, "runs": [
                ["id": 2, "parent": 1, "paint": true, "props": ["text": "plain "], "style": ["text_color": [0.0, 0.0, 0.0, 255.0]]],
                ["id": 3, "parent": 1, "paint": true, "props": ["text": "red"], "style": ["text_color": [255.0, 0.0, 0.0, 255.0]]],
            ]],
            ["op": "roots", "ids": [1]],
            ["op": "frame", "id": 1, "x": 0.0, "y": 0.0, "w": 300.0, "h": 40.0],
        ]))
        let node = try XCTUnwrap(p.views[1])
        let grey = [128.0, 128.0, 128.0, 255.0]
        // The host re-sends the paragraph with the run's presented colour.
        let paragraph = { (c: [Double]) in wireBatch([["op": "paragraph", "id": 1, "runs": [
            ["id": 2, "parent": 1, "paint": true, "props": ["text": "plain "], "style": ["text_color": c]],
            ["id": 3, "parent": 1, "paint": true, "props": ["text": "red"], "style": ["text_color": [255.0, 0.0, 0.0, 255.0]]],
        ]]]) }
        p.apply(paragraph(grey))
        XCTAssertEqual(node.paragraphSpec().runs.map(\.color), [grey, [255, 0, 0, 255]])
        p.apply(paragraph([0.0, 0.0, 0.0, 255.0]))
        XCTAssertEqual(node.paragraphSpec().runs.map(\.color), [[0, 0, 0, 255], [255, 0, 0, 255]])
    }

    func testAViewInItsOwnAppearanceIsNotedOnceUntilItAgreesAgain() throws {
        _ = NSApplication.shared
        let session = ExactApp.shared.makeSession(label: "paint-view-scheme-mac")
        defer { session.destroy() }
        XCTAssertNil(session.boot(size: CGSize(width: 390, height: 844)).error)
        let sheet = NSView(frame: NSRect(x: 0, y: 0, width: 100, height: 100))
        sheet.appearance = NSAppearance(named: .aqua)
        let view = NodeView(id: 99_999, kind: "view", presenter: session.presenter)
        sheet.addSubview(view)
        session.scheme(dark: false)
        session.noteAppearance(view)
        XCTAssertNil(session.viewDark[view.id], "it agrees with the session")
        sheet.appearance = NSAppearance(named: .darkAqua)
        XCTAssertTrue(view.drawsDark)
        session.noteAppearance(view)
        XCTAssertEqual(session.viewDark[view.id], true)
        sheet.appearance = NSAppearance(named: .aqua)
        session.noteAppearance(view)
        XCTAssertNil(session.viewDark[view.id], "it agrees again")
    }

    /// An `svg`'s `light-dark()` paint is resolved into its scene's layers,
    /// so an appearance change applies the scene again, as Chrome repaints
    /// it (minesweeper F11, paint F10).
    func testAnSvgPaintFollowsItsViewsAppearance() throws {
        _ = NSApplication.shared
        let session = ExactApp.shared.makeSession(label: "paint-svg-scheme-mac")
        defer { session.destroy() }
        let p = session.presenter
        p.root.appearance = NSAppearance(named: .aqua)
        let pair: [Any] = [[36.0, 86.0, 51.0, 255.0], [207.0, 255.0, 220.0, 255.0]]
        let shape: [String: Any] = ["id": 1, "n": 1, "o": 1, "p": [0.0, 0.0, 0.0, 1.0, 10.0, 0.0, 1.0, 10.0, 10.0, 3.0], "s": pair, "w": 2.0]
        p.apply(wireBatch([
            ["op": "create", "id": 1, "kind": "svg", "style": [:]],
            ["op": "roots", "ids": [1]],
            ["op": "frame", "id": 1, "x": 0.0, "y": 0.0, "w": 24.0, "h": 24.0],
            ["op": "svg", "id": 1, "scene": ["box": [0.0, 0.0, 24.0, 24.0], "t": [1.0, 0.0, 0.0, 1.0, 0.0, 0.0], "els": [shape]]],
        ]))
        let view = try XCTUnwrap(p.views[1])
        func stroke() -> [CGFloat]? {
            (view.layer?.sublayers?.lazy.compactMap { $0.sublayers?.first as? CAShapeLayer }.first?.strokeColor)?.components
        }
        XCTAssertEqual(stroke()?.map { ($0 * 255).rounded() }, [36, 86, 51, 255])
        p.root.appearance = NSAppearance(named: .darkAqua)
        XCTAssertTrue(view.drawsDark)
        XCTAssertEqual(stroke()?.map { ($0 * 255).rounded() }, [207, 255, 220, 255])
    }
}
#endif
