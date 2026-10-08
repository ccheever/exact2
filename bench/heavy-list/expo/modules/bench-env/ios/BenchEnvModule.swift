import ExpoModulesCore

// Exposes the launch environment's benchmark switches to JS as constants.
public class BenchEnvModule: Module {
  public func definition() -> ModuleDefinition {
    Name("BenchEnv")
    Constant("BENCH_LIVE") { ProcessInfo.processInfo.environment["BENCH_LIVE"] ?? "" }
    Constant("BENCH_START_INDEX") { ProcessInfo.processInfo.environment["BENCH_START_INDEX"] ?? "" }
  }
}
