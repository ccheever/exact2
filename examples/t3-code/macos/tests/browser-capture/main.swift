import AppKit
import XCTest

// browser-surface part 3 (capture): Annotate's overlay (annotate.swift), the recording overlay, encoder and decorations
// (recording.swift), and screenshots, artifacts, held pages, the separate window and the capture ops (capture.swift),
// against CaptureFixture (fixture.swift). Run with EXACT_ASSETS naming examples/t3-code (the page scripts load from its
// assets/) and, optionally, T3_BROWSER_TEST_DIR for the images and recordings the tests leave.
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
