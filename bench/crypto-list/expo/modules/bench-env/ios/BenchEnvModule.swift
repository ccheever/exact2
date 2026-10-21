import ExpoModulesCore

// Exposes the launch environment's benchmark switches to JS as constants
// (the same approach as bench/heavy-list/expo's modules/bench-env).
public class BenchEnvModule: Module {
  public func definition() -> ModuleDefinition {
    Name("BenchEnv")
    Constant("BENCH_LIVE") { ProcessInfo.processInfo.environment["BENCH_LIVE"] ?? "" }
    Constant("BENCH_SCENARIO") { ProcessInfo.processInfo.environment["BENCH_SCENARIO"] ?? "" }
    Constant("BENCH_FREEZE") { ProcessInfo.processInfo.environment["BENCH_FREEZE"] ?? "" }
    Constant("BENCH_START_INDEX") { ProcessInfo.processInfo.environment["BENCH_START_INDEX"] ?? "" }
  }
}
