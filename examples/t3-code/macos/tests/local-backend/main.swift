import Foundation
import XCTest

// The embedded server's native logic (20261005-embedded-server-runtime). Build and run with the
// README's module-test recipe (every file in modules/apple/ and this directory).
let suite = XCTestSuite(name: "T3 local backend")
for tests in [LocalBackendManagerTests.defaultTestSuite, LocalShellEnvironmentTests.defaultTestSuite, LocalOutputLogTests.defaultTestSuite,
              LocalRuntimeInstallTests.defaultTestSuite, LocalPolicyTests.defaultTestSuite, LocalCrashReaperTests.defaultTestSuite,
              LocalProcessTests.defaultTestSuite, LocalAuthTests.defaultTestSuite] { suite.addTest(tests) }
suite.run()
guard let run = suite.testRun, run.executionCount == 44 else { print("T3 local backend: \(suite.testRun?.executionCount ?? 0) tests ran, expected 44"); exit(1) }
print("T3 local backend: \(run.executionCount) tests, \(run.totalFailureCount) failures")
exit(run.hasSucceeded ? 0 : 1)
