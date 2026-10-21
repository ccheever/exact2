import XCTest
@testable import ExactKit
#if os(macOS)
import AppKit
#elseif os(iOS)
import UIKit
#endif

#if os(macOS) || os(iOS)
class IntersectionFixture: XCTestCase {
    func plan() throws -> Data {
        #if os(macOS)
        let dir = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
        try FileManager.default.createDirectory(at: dir, withIntermediateDirectories: true)
        defer { try? FileManager.default.removeItem(at: dir) }
        let source = URL(fileURLWithPath: #filePath).deletingLastPathComponent().appendingPathComponent("../../../../scripts/fixtures/intersect.contract").standardizedFileURL
        let out = dir.appendingPathComponent("app.plan")
        let compiler = Process()
        compiler.executableURL = URL(fileURLWithPath: try XCTUnwrap(ProcessInfo.processInfo.environment["EXACT_CONTRACT"]))
        compiler.arguments = ["build", source.path, "-o", out.path]
        try compiler.run(); compiler.waitUntilExit()
        XCTAssertEqual(compiler.terminationStatus, 0)
        return try Data(contentsOf: out)
        #else
        return try Data(contentsOf: URL(fileURLWithPath: try XCTUnwrap(ProcessInfo.processInfo.environment["EXACT_INTERSECT_PLAN"])))
        #endif
    }
    func exercise() throws {
        #if os(macOS)
        _ = NSApplication.shared
        let window = NSWindow(contentRect: NSRect(x: 0, y: 0, width: 400, height: 400), styleMask: [.titled], backing: .buffered, defer: false)
        window.isReleasedWhenClosed = false
        defer { window.close() }
        #else
        let window = UIWindow(frame: CGRect(x: 0, y: 0, width: 400, height: 400))
        defer { window.isHidden = true }
        #endif
        let session = ExactApp.shared.makeSession(label: "intersect")
        defer { session.destroy() }
        #if os(macOS)
        window.contentView = session.presenter.viewport
        #else
        session.presenter.viewport.frame = window.bounds
        window.addSubview(session.presenter.viewport)
        window.makeKeyAndVisible()
        #endif
        XCTAssertNil(session.boot(plan: try plan(), size: CGSize(width: 400, height: 400)).error)
        func facts(_ name: String) -> String { session.presenter.views.values.first { $0.props["testId"] == name }?.props["text"] ?? "" }
        func until(_ value: String, _ name: String = "facts") {
            let end = Date().addingTimeInterval(20)
            while facts(name) != value, Date() < end {
                RunLoop.main.run(mode: .default, before: Date().addingTimeInterval(0.05))
            }
            XCTAssertEqual(facts(name), value)
        }
        func scroll(_ top: CGFloat, _ name: String = "port") throws {
            let root = try XCTUnwrap(session.presenter.views.values.first { $0.props["testId"] == name })
            #if os(macOS)
            let scroll = try XCTUnwrap(root.scroll)
            scroll.contentView.scroll(to: CGPoint(x: 0, y: top))
            scroll.reflectScrolledClipView(scroll.contentView)
            root.clipScrolled()
            #else
            let scroll = try XCTUnwrap(root.scrollView)
            scroll.setContentOffset(CGPoint(x: 0, y: top), animated: false)
            root.scrollViewDidScroll(scroll)
            #endif
        }
        let end = Date().addingTimeInterval(20)
        func initialFacts() -> String { session.presenter.views.values.first { $0.props["testId"] == "initial-facts" }?.props["text"] ?? "" }
        while initialFacts() == "0:false:0", Date() < end {
            RunLoop.main.run(mode: .default, before: Date().addingTimeInterval(0.05))
        }
        XCTAssertEqual(initialFacts(), "1:true:1", "initial observation follows authored scrollTop")
        until("1:false:0")
        try scroll(60)
        until("2:true:0.6")
        try scroll(211)
        until("3:false:0")
        // No scroll handler is declared: geometry reports alone must wake delivery.
        try scroll(60)
        until("4:true:0.6")
        until("1:true:1", "sticky-facts")
        try scroll(201, "sticky-port")
        until("2:false:0", "sticky-facts")
        try scroll(60, "sticky-port")
        until("3:true:1", "sticky-facts")
    }
}
#if os(macOS)
final class IntersectionMacTests: IntersectionFixture {
    func testMountAndNativeScrollWithoutScrollHandler() throws { try exercise() }
}
#else
final class IntersectionIOSTests: IntersectionFixture {
    func testMountAndNativeScrollWithoutScrollHandler() throws { try exercise() }
}
#endif
#endif
