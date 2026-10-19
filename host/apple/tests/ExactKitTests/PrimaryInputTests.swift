import XCTest
import ExactCore
@testable import ExactKit

/// LLP 1132 D10: `exactViewport().pointer` and `.hover` by device class, an
/// iPad with a trackpad or mouse answering as the pointing device, and the
/// agent's `prefer pointer` and `prefer hover` reaching the runtime.
final class PrimaryInputTests: XCTestCase {
    func testATouchScreenWithAPointingDeviceIsFineAndHovers() {
        XCTAssertEqual(Preferences.primaryInput(touchScreen: true), [.coarsePointer, .noHover], "a finger")
        XCTAssertEqual(Preferences.primaryInput(touchScreen: true, pointingDevice: true), [], "an iPad with a trackpad: fine, hover")
        XCTAssertEqual(Preferences.primaryInput(touchScreen: false), [], "a Mac")
        XCTAssertEqual(Preferences.primaryInput(touchScreen: false, focusEngine: true), [.noPointer, .noHover], "a remote")
        // The iPad's own reading: `GCMouse.current != nil` is the pointing device.
        XCTAssertEqual(DisplayPreferences.touchInput(pointingDevice: true), [], "a trackpad attached: fine, hover")
        XCTAssertEqual(DisplayPreferences.touchInput(pointingDevice: false), [.coarsePointer, .noHover])
    }

    #if os(macOS)
    private static let plan: Result<Data, Error> = Result {
        let dir = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
        try FileManager.default.createDirectory(at: dir, withIntermediateDirectories: true)
        defer { try? FileManager.default.removeItem(at: dir) }
        let source = "shape Viewport\n  pointer: string\n  hover: string\ncomponent App\n  resource viewport = exactViewport() as shape Viewport\n  view\n    text `${viewport.pointer} ${viewport.hover}`\n"
        try source.write(to: dir.appendingPathComponent("app.contract"), atomically: true, encoding: .utf8)
        let compiler = Process()
        compiler.executableURL = URL(fileURLWithPath: try XCTUnwrap(ProcessInfo.processInfo.environment["EXACT_CONTRACT"], "build.mjs --test builds the Contract compiler and names it"))
        compiler.arguments = ["build", dir.appendingPathComponent("app.contract").path, "-o", dir.appendingPathComponent("app.plan").path]
        try compiler.run(); compiler.waitUntilExit()
        XCTAssertEqual(compiler.terminationStatus, 0)
        return try Data(contentsOf: dir.appendingPathComponent("app.plan"))
    }

    func testPreferPointerAndHoverReachExactViewport() throws {
        let saved = DisplayPreferences.agentInput
        defer { DisplayPreferences.agentInput = saved }
        let session = ExactApp.shared.makeSession(label: "primary-input")
        defer { session.destroy() }
        XCTAssertNil(session.boot(plan: try Self.plan.get(), size: CGSize(width: 390, height: 844)).error)
        let said = { session.presenter.views.values.compactMap { $0.props["text"] }.first { $0.contains(" ") } }
        XCTAssertEqual(said(), "fine hover", "a Mac's")
        let reply = session.agentInstance.prefer(["media": ["pointer": "coarse", "hover": "none"]])
        XCTAssertEqual((reply["media"] as? [String: Any])?["pointer"] as? String, "coarse")
        XCTAssertEqual(said(), "coarse none")
        let refused = session.agentInstance.prefer(["media": ["pointer": "none", "hover": "sometimes"]])
        XCTAssertEqual(refused["malformed"] as? Bool, true)
        XCTAssertEqual(said(), "coarse none", "nothing applied")
        _ = session.agentInstance.prefer(["media": ["pointer": "none"]])
        XCTAssertEqual(said(), "none none")
    }
    #endif
}
