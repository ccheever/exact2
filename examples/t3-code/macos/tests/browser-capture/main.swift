import AppKit
import XCTest

// The recording overlay, encoder and decorations (recording.swift), Annotate's overlay (annotate.swift), and what a
// Browser frame hands the `t3-browser-page` hatch: artifacts, the separate window, the recording ops, the agent's cursor
// and the profiles' old cookies (capture.swift), a saved recording's upload (upload.swift), against CaptureFixture (fixture.swift). EXACT_ASSETS (else the recipe's T3_APP_DIR) names examples/t3-code, whose
// assets/ hold the page scripts; T3_BROWSER_TEST_DIR, when set, receives the images and recordings the tests leave.
// The README recipe exports T3_APP_DIR (the app directory): the page scripts load from its assets/ as EXACT_ASSETS.
if ProcessInfo.processInfo.environment["EXACT_ASSETS"] == nil, let app = ProcessInfo.processInfo.environment["T3_APP_DIR"] { setenv("EXACT_ASSETS", app, 0) }
_ = NSApplication.shared
NSApp.setActivationPolicy(.accessory)
let suite = XCTestSuite(name: "browser-capture")
for testCase in [BrowserCaptureReferenceTests.self, BrowserCaptureTests.self, BrowserAnnotateTests.self, RecordingDecorationsTests.self, BrowserRecordingTests.self, BrowserRecordingUploadTests.self] as [XCTestCase.Type] {
    suite.addTest(XCTestSuite(forTestCaseClass: testCase))
}
suite.run()
let run = suite.testRun!
print("Executed \(run.executionCount) tests, with \(run.totalFailureCount) failures")
exit(run.totalFailureCount == 0 ? 0 : 1)
