import Foundation
import XCTest

// `t3 app <dir>` (20261005-app-activation): the control socket, the broker, the address and the
// app-level control (T3AppControl.swift). Build and run with the README's module-test recipe (every
// file in modules/apple/ and this directory). The reference's tests keep their names in the test
// output (`T3 app control: <name> ok`).
let suite = XCTestSuite(name: "T3 app control")
for tests in [AppControlServerTests.defaultTestSuite, ActivationBrokerTests.defaultTestSuite, AppControlAddressTests.defaultTestSuite,
              AppControlProtocolTests.defaultTestSuite, AppControlLifecycleTests.defaultTestSuite] { suite.addTest(tests) }
suite.run()
let expected = 25
guard let run = suite.testRun, run.executionCount == expected else { print("T3 app control: \(suite.testRun?.executionCount ?? 0) tests ran, expected \(expected)"); exit(1) }
print("T3 app control: \(run.executionCount) tests, \(run.totalFailureCount) failures")
exit(run.hasSucceeded ? 0 : 1)
