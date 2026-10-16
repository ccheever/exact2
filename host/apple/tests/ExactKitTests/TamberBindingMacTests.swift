#if os(macOS)
import AppKit
import XCTest
@testable import ExactKit

/// LLP 1121 D12 (stage 5) on AppKit: `ExactTamber` in the process resolves a
/// token for a place (a view under a pin answers its own tuple), as a
/// dynamic colour, and `observe` hears `setMode`, which moves the window's.
final class TamberBindingMacTests: XCTestCase {
    private static let plan: Result<Data, Error> = Result {
        let dir = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
        try FileManager.default.createDirectory(at: dir, withIntermediateDirectories: true)
        defer { try? FileManager.default.removeItem(at: dir) }
        let design = "tamber t\n\naxes\n  tone = warm | cool\n\ntheme\n  ink = #c8102e\n    when tone=cool: #0a7aff\n"
        let source = "component App\n  action flip\n    setMode(\"tone\", \"cool\")\n  view\n    column color=\"ink\"\n      text \"a\" testId=\"window\"\n      column -exact-mode=\"tone=cool\" color=\"ink\"\n        text \"b\" testId=\"pinned\"\n"
        try design.write(to: dir.appendingPathComponent("app.tamber"), atomically: true, encoding: .utf8)
        try source.write(to: dir.appendingPathComponent("app.contract"), atomically: true, encoding: .utf8)
        let compiler = Process()
        compiler.executableURL = URL(fileURLWithPath: try XCTUnwrap(ProcessInfo.processInfo.environment["EXACT_CONTRACT"], "build.mjs --test builds the Contract compiler and names it"))
        compiler.arguments = ["build", dir.appendingPathComponent("app.contract").path, "-o", dir.appendingPathComponent("app.plan").path]
        try compiler.run(); compiler.waitUntilExit()
        XCTAssertEqual(compiler.terminationStatus, 0)
        return try Data(contentsOf: dir.appendingPathComponent("app.plan"))
    }

    private func rgb(_ color: NSColor) -> [Int] {
        var out: [Int] = []
        NSAppearance(named: .aqua)!.performAsCurrentDrawingAppearance {
            guard let c = color.usingColorSpace(.sRGB) else { return }
            out = [c.redComponent, c.greenComponent, c.blueComponent].map { Int(($0 * 255).rounded()) }
        }
        return out
    }

    func testAPlaceResolvesItsTupleAndObserveHearsSetMode() throws {
        _ = NSApplication.shared
        let session = ExactApp.shared.makeSession(label: "tamber-binding")
        defer { session.destroy() }
        XCTAssertNil(session.boot(plan: try Self.plan.get(), size: CGSize(width: 390, height: 844)).error)
        let view = { (id: String) in try XCTUnwrap(session.presenter.views.values.first { $0.props["testId"] == id }) }
        let ink = ExactTamberToken("ink")
        let root = ExactTamber(session: session)
        XCTAssertEqual(rgb(root.color(ink)), [0xc8, 0x10, 0x2e], "the window's tuple")
        XCTAssertEqual(rgb(ExactTamber(try view("pinned")).color(ink)), [0x0a, 0x7a, 0xff], "a view under a pin, its own")
        XCTAssertEqual(rgb(root.color(ExactTamberToken("nonesuch"), fallback: .green)), [0, 255, 0], "a name the plan lacks, its fallback")
        var heard = 0
        let observation = ExactTamber(try view("window")).observe { heard += 1 }
        session.apply(session.runtime.setModes(["tone": "cool"]))
        XCTAssertEqual(heard, 1, "setMode moved what the window's place reads")
        XCTAssertEqual(rgb(ExactTamber(try view("window")).color(ink)), [0x0a, 0x7a, 0xff])
        observation.stop()
        session.apply(session.runtime.setModes(["tone": "warm"]))
        XCTAssertEqual(heard, 1, "a stopped observation hears nothing")
    }

    /// Review round 2: a colour kept across a batch reads the new answer, a
    /// moved place's view is redrawn, and a view observed before it is
    /// mounted hears the node it is mounted in.
    func testAKeptColourFollowsAndAnUnmountedViewHearsOnceMounted() throws {
        _ = NSApplication.shared
        let session = ExactApp.shared.makeSession(label: "tamber-binding-kept")
        defer { session.destroy() }
        XCTAssertNil(session.boot(plan: try Self.plan.get(), size: CGSize(width: 390, height: 844)).error)
        let window = try XCTUnwrap(session.presenter.views.values.first { $0.props["testId"] == "window" })
        let ink = ExactTamberToken("ink")
        let drawn = Marked(), later = NSView()
        window.addSubview(drawn)
        let kept = ExactTamber(drawn).color(ink)
        XCTAssertEqual(rgb(kept), [0xc8, 0x10, 0x2e])
        var heard = 0
        let observation = ExactTamber(later).observe { heard += 1 }
        window.addSubview(later)
        drawn.marked = 0
        session.apply(session.runtime.setModes(["tone": "cool"]))
        XCTAssertEqual(rgb(kept), [0x0a, 0x7a, 0xff], "the colour kept reads the window's new tuple")
        XCTAssertGreaterThan(drawn.marked, 0, "the moved place's view is drawn again")
        XCTAssertEqual(heard, 0, "observed unmounted: nothing heard before, so no move to hear")
        session.apply(session.runtime.setModes(["tone": "warm"]))
        XCTAssertEqual(heard, 1, "mounted since, it hears its node's move")
        observation.stop()
    }
}
/// A view that counts the times it is marked to be drawn.
private final class Marked: NSView {
    var marked = 0
    override var needsDisplay: Bool {
        get { super.needsDisplay }
        set { if newValue { marked += 1 }; super.needsDisplay = newValue }
    }
}
#endif
