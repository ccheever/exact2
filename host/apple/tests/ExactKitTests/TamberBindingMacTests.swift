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

    private static let kindsPlan: Result<Data, Error> = Result {
        let dir = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
        try FileManager.default.createDirectory(at: dir, withIntermediateDirectories: true)
        defer { try? FileManager.default.removeItem(at: dir) }
        let design = "tamber t\n\naxes\n  tone = warm | cool\n\ntheme\n  space.row = 12px\n    when tone=cool: 8px\n  opacity.faded: number = 0.5\n  type.sys = 600 1rem system-ui\n"
        let source = "component App\n  view\n    column padding=\"space.row\" opacity=\"opacity.faded\"\n      text \"a\" testId=\"window\" font=\"type.sys\"\n      column -exact-mode=\"tone=cool\" padding=\"space.row\"\n        text \"b\" testId=\"pinned\"\n"
        try design.write(to: dir.appendingPathComponent("app.tamber"), atomically: true, encoding: .utf8)
        try source.write(to: dir.appendingPathComponent("app.contract"), atomically: true, encoding: .utf8)
        let compiler = Process()
        compiler.executableURL = URL(fileURLWithPath: try XCTUnwrap(ProcessInfo.processInfo.environment["EXACT_CONTRACT"], "build.mjs --test builds the Contract compiler and names it"))
        compiler.arguments = ["build", dir.appendingPathComponent("app.contract").path, "-o", dir.appendingPathComponent("app.plan").path]
        try compiler.run(); compiler.waitUntilExit()
        XCTAssertEqual(compiler.terminationStatus, 0)
        return try Data(contentsOf: dir.appendingPathComponent("app.plan"))
    }

    /// Stage 6 G: a length, a number and a font for a place (a pin's tuple
    /// its own), the font at the person's text size and Bold Text's step,
    /// and `observe` hearing both move.
    func testALengthNumberAndFontResolveAndObserveHearsTextSizeAndBoldText() throws {
        _ = NSApplication.shared
        let session = ExactApp.shared.makeSession(label: "tamber-binding-kinds")
        defer { session.destroy(); DisplayPreferences.agentBoldText = nil }
        XCTAssertNil(session.boot(plan: try Self.kindsPlan.get(), size: CGSize(width: 390, height: 844)).error)
        let view = { (id: String) in try XCTUnwrap(session.presenter.views.values.first { $0.props["testId"] == id }) }
        let root = ExactTamber(session: session)
        XCTAssertEqual(root.length(ExactTamberToken("space.row")), 12)
        XCTAssertEqual(ExactTamber(try view("pinned")).length(ExactTamberToken("space.row")), 8, "a pin's tuple")
        XCTAssertEqual(root.number(ExactTamberToken("opacity.faded")), 0.5)
        XCTAssertEqual(root.length(ExactTamberToken("nonesuch"), fallback: 3), 3)
        let sys = ExactTamberToken("type.sys")
        let fallback = NSFont.systemFont(ofSize: 5)
        let font = ExactTamber(try view("window")).font(sys, fallback: fallback)
        XCTAssertGreaterThan(font.pointSize, 5, "1rem at the root font size, not the fallback")
        var heard = 0
        let observation = ExactTamber(try view("window")).observe { heard += 1 }
        session.apply(session.runtime.setRootFontSize(20))
        XCTAssertEqual(heard, 1, "the person's text size moved it")
        XCTAssertEqual(ExactTamber(try view("window")).font(sys, fallback: fallback).pointSize, 20, accuracy: 0.01)
        let before = NSFontManager.shared.weight(of: ExactTamber(try view("window")).font(sys, fallback: fallback))
        DisplayPreferences.agentBoldText = true
        XCTAssertEqual(heard, 2, "Bold Text moved it")
        let after = NSFontManager.shared.weight(of: ExactTamber(try view("window")).font(sys, fallback: fallback))
        XCTAssertGreaterThan(after, before, "a system family one step heavier")
        observation.stop()
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
        XCTAssertEqual(heard, 1, "observed unmounted, the first batch since mounting moves it off the fallback")
        session.apply(session.runtime.setModes(["tone": "warm"]))
        XCTAssertEqual(heard, 2, "and it hears its node's next move")
        observation.stop()
    }

    /// Review round 3: a session's first binding, observed unmounted, hears
    /// its node once mounted.
    func testAFirstObserverMadeUnmountedHearsOnceMounted() throws {
        _ = NSApplication.shared
        let session = ExactApp.shared.makeSession(label: "tamber-binding-first")
        defer { session.destroy() }
        XCTAssertNil(session.boot(plan: try Self.plan.get(), size: CGSize(width: 390, height: 844)).error)
        let window = try XCTUnwrap(session.presenter.views.values.first { $0.props["testId"] == "window" })
        let later = NSView()
        var heard = 0
        let observation = ExactTamber(later).observe { heard += 1 }
        window.addSubview(later)
        session.apply(session.runtime.setModes(["tone": "cool"]))
        session.apply(session.runtime.setModes(["tone": "warm"]))
        XCTAssertEqual(heard, 2, "the first batch since mounting moves it off the fallback, the second moves it")
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
