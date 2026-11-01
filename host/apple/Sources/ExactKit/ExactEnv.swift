import Foundation
import QuartzCore

/// The process facts every session reads: the agent drives the app
/// (LLP 1012 — the driver owns the clock), a smoke run prints and exits.
public enum ExactEnv {
    /// The process's environment. A production bake never enters agent mode
    /// (LLP 1069.007 D2, ruled) and takes no module override (LLP 1024 D5):
    /// its `EXACT_AGENT`, `EXACT_AGENT_*`, `EXACT_HATCHES` and
    /// `EXACT_MODULES` variables are dropped here, from the process too,
    /// before this host or its library reads one.
    public static let environment: [String: String] = {
        var environment = ProcessInfo.processInfo.environment
        let agent = environment.keys.filter { ["EXACT_AGENT", "EXACT_HATCHES", "EXACT_MODULES"].contains($0) || $0.hasPrefix("EXACT_AGENT_") }.sorted()
        guard !agent.isEmpty, productionBake else { return environment }
        FileHandle.standardError.write(Data("exact: a production build ignores \(agent.joined(separator: ", ")) (LLP 1069.007 D2, LLP 1024 D5)\n".utf8))
        for key in agent { environment[key] = nil; unsetenv(key) }
        return environment
    }()
    /// Whether this binary was baked with `EXACT_UPDATE_TRUST=production`
    /// (`BakedCompatibility.production`).
    static let productionBake = BakedCompatibility.production(BakedCompatibility.json)
    public static let agentMode = environment["EXACT_AGENT"] == "1"
    /// `EXACT_AGENT_BIND` (`--bind`, LLP 1133.001 D7): extensions answered by their implementations under the agent.
    static let agentBind = Set((environment["EXACT_AGENT_BIND"] ?? "").split(separator: ",").map(String.init))
    /// `EXACT_AGENT_TIMING=platform` (LLP 1035.003 D5, opt-in): under the
    /// agent carrier, UIKit's own transitions, presentations and keyboard
    /// animations keep their natural timing — the ordinary app with a
    /// socket, for observing a gesture's native motion. The default freezes
    /// them, which the smoke depends on.
    public static let agentTiming = environment["EXACT_AGENT_TIMING"] ?? "agent"
    /// Whether the agent's world is settled between calls: native animation
    /// applies at once. False under `platform` timing.
    public static let agentFreezes = agentMode && agentTiming != "platform"
    public static let smoke = environment["EXACT_SMOKE"] == "1"
    /// Milliseconds since the process's `main`: the wall clock for startup stamps.
    public static let t0 = CACurrentMediaTime()
    public static func wall() -> Double { (CACurrentMediaTime() - t0) * 1000 }
    /// Startup stamps, milliseconds from `main`, in order (the smoke prints them).
    nonisolated(unsafe) public static var stamps: [(String, Double)] = []
    public static func stamp(_ label: String) { stamps.append((label, wall())) }
}
