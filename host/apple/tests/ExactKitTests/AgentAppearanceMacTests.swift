#if os(macOS)
import AppKit
import XCTest
@testable import ExactKit

/// LLP 1069.007 D2 on AppKit: the agent's system appearance is the one the
/// runtime reads as `prefers-color-scheme`, whenever it changes, so a
/// `prefer` re-tells nothing it does not name (issue X58).
final class AgentAppearanceMacTests: XCTestCase {
    /// The scheme's own subtree: a node that mounts again when it changes.
    private static let plan: Result<Data, Error> = Result {
        let dir = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
        try FileManager.default.createDirectory(at: dir, withIntermediateDirectories: true)
        defer { try? FileManager.default.removeItem(at: dir) }
        let source = "shape Viewport\n  prefersColorScheme: string\ncomponent App\n  resource viewport = exactViewport() as shape Viewport\n  view\n    column\n      when viewport.prefersColorScheme == \"dark\"\n        text \"dark\"\n      else\n        text \"light\"\n"
        try source.write(to: dir.appendingPathComponent("app.contract"), atomically: true, encoding: .utf8)
        let compiler = Process()
        compiler.executableURL = URL(fileURLWithPath: try XCTUnwrap(ProcessInfo.processInfo.environment["EXACT_CONTRACT"], "build.mjs --test builds the Contract compiler and names it"))
        compiler.arguments = ["build", dir.appendingPathComponent("app.contract").path, "-o", dir.appendingPathComponent("app.plan").path]
        try compiler.run(); compiler.waitUntilExit()
        XCTAssertEqual(compiler.terminationStatus, 0)
        return try Data(contentsOf: dir.appendingPathComponent("app.plan"))
    }
    private var restore: () -> Void = {}

    override func setUp() {
        _ = NSApplication.shared
        let (appearance, system) = (NSApp.appearance, Agent.systemAppearance)
        let (agent, contrast, gamut, range, page) = (DisplayPreferences.agent, DisplayPreferences.agentContrast, DisplayPreferences.agentGamut, DisplayRange.pinned, PageFacts.agent)
        restore = {
            NSApp.appearance = appearance; Agent.systemAppearance = system
            DisplayPreferences.agent = agent; DisplayPreferences.agentContrast = contrast; DisplayPreferences.agentGamut = gamut
            DisplayRange.pinned = range; PageFacts.agent = page
        }
    }
    override func tearDown() { restore() }

    /// The node that says the scheme the runtime holds: its id and its word.
    private func scheme(_ session: ExactSession) -> (id: UInt32, text: String)? {
        session.presenter.views.values.first { ["dark", "light"].contains($0.props["text"]) }.map { ($0.id, $0.props["text"]!) }
    }

    /// Both ways, whatever the Mac's appearance: the agent's scheme reaches
    /// the runtime when it is set, not at the next fact a `prefer` names.
    func testTheAgentsSystemSchemeIsToldAndAPageFactLeavesIt() throws {
        let session = ExactApp.shared.makeSession(label: "agent-appearance")
        defer { session.destroy() }
        XCTAssertNil(session.boot(plan: try Self.plan.get(), size: CGSize(width: 390, height: 844)).error)
        let agent = session.agentInstance
        agent.systemScheme(dark: true)
        XCTAssertEqual(scheme(session)?.text, "dark", "the agent's dark is told")
        agent.systemScheme(dark: false)
        let light = try XCTUnwrap(scheme(session))
        XCTAssertEqual(light.text, "light", "the agent's light is told")
        let reply = agent.prefer(["page": ["has-focus": false]])
        XCTAssertEqual((reply["media"] as? [String: Any])?["prefers-color-scheme"] as? String, "light")
        XCTAssertEqual(scheme(session)?.id, light.id, "a page fact leaves the scheme's subtree mounted")
    }

    /// Under the agent the appearance is light from launch, as
    /// `Agent.systemAppearance` starts: on the app, which the window draws,
    /// and read by the first boot, whatever the Mac's; an app's own stays.
    func testTheAgentsAppearanceIsLightFromLaunch() throws {
        NSApp.appearance = nil
        Agent.systemAppearance = Agent.launchAppearance()
        XCTAssertEqual(Agent.systemAppearance?.name, .aqua)
        XCTAssertEqual(NSApp.effectiveAppearance.bestMatch(from: [.aqua, .darkAqua]), .aqua, "the app draws it")
        let session = ExactApp.shared.makeSession(label: "agent-appearance-launch")
        defer { session.destroy() }
        XCTAssertNil(session.boot(plan: try Self.plan.get(), size: CGSize(width: 390, height: 844)).error)
        let light = try XCTUnwrap(scheme(session))
        XCTAssertEqual(light.text, "light", "the first boot is told it")
        _ = session.agentInstance.prefer(["page": ["has-focus": false]])
        XCTAssertEqual(scheme(session)?.id, light.id, "the first prefer changes only what it names")
        NSApp.appearance = NSAppearance(named: .darkAqua)
        _ = Agent.launchAppearance()
        XCTAssertEqual(NSApp.appearance?.name, .darkAqua, "an app's own appearance stays")
    }
}
#endif
