import AppKit
import XCTest

// Compile with T3ImageFit.swift: SnapShot's draft image fitting (fit.swift).
_ = NSApplication.shared
let suite = XCTestSuite(name: "attach")
suite.addTest(XCTestSuite(forTestCaseClass: ImageFitTests.self))
suite.run()
let run = suite.testRun!
print("Executed \(run.executionCount) tests, with \(run.totalFailureCount) failures")
exit(run.totalFailureCount == 0 ? 0 : 1)
