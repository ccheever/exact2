#if os(macOS)
import AppKit
import XCTest
import ObjectiveC
@testable import ExactKit

final class InnerScrollMacTests: XCTestCase {
    private static var preferredStyle: NSScroller.Style = .overlay
    @objc private class func forcedScrollerStyle() -> NSScroller.Style { preferredStyle }

    func testInnerContentMatchesTheClipAcrossSystemScrollerStyles() throws {
        _ = NSApplication.shared
        let original = try XCTUnwrap(class_getClassMethod(NSScroller.self, #selector(getter: NSScroller.preferredScrollerStyle)))
        let replacement = try XCTUnwrap(class_getClassMethod(Self.self, #selector(Self.forcedScrollerStyle)))
        method_exchangeImplementations(original, replacement)
        defer { method_exchangeImplementations(original, replacement) }
        Self.preferredStyle = .overlay
        let dir = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
        try FileManager.default.createDirectory(at: dir, withIntermediateDirectories: true)
        defer { try? FileManager.default.removeItem(at: dir) }
        let contract = dir.appendingPathComponent("app.contract"), plan = dir.appendingPathComponent("app.plan")
        try """
        component App
          view
            column
              view width=300 height=200 overflow="auto" testId="vertical"
                view width="100%" height=600 testId="vertical-child"
              view width=300 height=200 overflow="auto" testId="horizontal"
                view width=600 height="100%" testId="horizontal-child"
              view width=300 height=200 overflow="auto" direction="rtl" testId="rtl"
                view width="100%" height=600 testId="rtl-child"
              view width=300 height=200 overflow="auto" scrollbar-width="none" testId="hidden"
                view width="100%" height=600 testId="hidden-child"

        """.write(to: contract, atomically: true, encoding: .utf8)
        let compiler = Process()
        compiler.executableURL = URL(fileURLWithPath: try XCTUnwrap(ProcessInfo.processInfo.environment["EXACT_CONTRACT"]))
        compiler.arguments = ["build", contract.path, "-o", plan.path]
        try compiler.run(); compiler.waitUntilExit()
        XCTAssertEqual(compiler.terminationStatus, 0)
        let session = ExactApp.shared.makeSession(label: "inner-scroll")
        defer { session.destroy() }
        XCTAssertNil(session.boot(plan: try Data(contentsOf: plan), size: NSSize(width: 800, height: 800)).error)
        let p = session.presenter
        func node(_ name: String) throws -> NodeView { try XCTUnwrap(p.views.values.first { $0.props["testId"] == name }) }
        for style in [NSScroller.Style.overlay, .legacy, .overlay, .legacy] {
            Self.preferredStyle = style
            NotificationCenter.default.post(name: NSScroller.preferredScrollerStyleDidChangeNotification, object: nil)
            for name in ["vertical", "horizontal", "hidden", "rtl"] {
                let parent = try node(name), child = try node(name + "-child")
                let scroll = try XCTUnwrap(parent.scroll)
                XCTAssertEqual(scroll.scrollerStyle, ExactEnv.agentMode ? .overlay : style)
                XCTAssertEqual(scroll.userInterfaceLayoutDirection, name == "rtl" ? .rightToLeft : .leftToRight)
                if name == "rtl" {
                    let drawn = child.convert(child.bounds, to: parent)
                    XCTAssertEqual(drawn.minX, scroll.contentView.frame.minX, accuracy: 0.001)
                    XCTAssertEqual(drawn.maxX, scroll.contentView.frame.maxX, accuracy: 0.001)
                }
                if name == "horizontal" {
                    XCTAssertEqual(child.frame.height, scroll.contentView.bounds.height, accuracy: 0.001)
                    XCTAssertTrue(scroll.verticalScroller!.isHidden, "a horizontal gutter must not create vertical overflow")
                } else {
                    XCTAssertEqual(child.frame.width, scroll.contentView.bounds.width, accuracy: 0.001)
                    XCTAssertTrue(scroll.horizontalScroller!.isHidden, "\(name): a vertical gutter must not create horizontal overflow; content \(parent.content), document \(scroll.documentView!.frame), clip \(scroll.contentView.frame), bounds \(scroll.contentView.bounds)")
                    XCTAssertEqual(child.frame.width < 300, style == .legacy && name != "hidden" && !ExactEnv.agentMode)
                }
            }
        }
    }
}
#endif
