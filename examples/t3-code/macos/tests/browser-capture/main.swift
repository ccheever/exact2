import AppKit
import XCTest

// browser-surface part 3 (capture): Annotate's overlay (annotate.swift), the recording overlay, encoder and decorations
// (recording.swift), and screenshots, artifacts, held pages, the separate window and the capture ops (capture.swift),
// against CaptureFixture (fixture.swift). EXACT_ASSETS (else the recipe's T3_APP_DIR) names examples/t3-code, whose
// assets/ hold the page scripts; T3_BROWSER_TEST_DIR, when set, receives the images and recordings the tests leave.
// The README recipe exports T3_APP_DIR (the app directory): the page scripts load from its assets/ as EXACT_ASSETS.
if ProcessInfo.processInfo.environment["EXACT_ASSETS"] == nil, let app = ProcessInfo.processInfo.environment["T3_APP_DIR"] { setenv("EXACT_ASSETS", app, 0) }
_ = NSApplication.shared
NSApp.setActivationPolicy(.accessory)
let suite = XCTestSuite(name: "browser-capture")
for testCase in [BrowserCaptureReferenceTests.self, BrowserCaptureTests.self, BrowserAnnotateTests.self, RecordingDecorationsTests.self, BrowserRecordingTests.self] as [XCTestCase.Type] {
    suite.addTest(XCTestSuite(forTestCaseClass: testCase))
}
suite.run()
let run = suite.testRun!
print("Executed \(run.executionCount) tests, with \(run.totalFailureCount) failures")
exit(run.totalFailureCount == 0 ? 0 : 1)
