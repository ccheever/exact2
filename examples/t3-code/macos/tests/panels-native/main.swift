import AppKit
import XCTest

// The right panel's native preview: the media preview reads only a signed asset route.
// Compiled without T3Module.swift, so the test supplies the module the facade names.
final class PanelsStubModule: ExactModule {}
let exactModule: ExactModule.Type = PanelsStubModule.self

final class PanelsTests: XCTestCase {
    func testAssetRouteOnly() {
        XCTAssertNotNil(T3MediaPreview.assetURL("http://127.0.0.1:1/api/assets/abc"))
        XCTAssertNil(T3MediaPreview.assetURL("file:///etc/hosts"))
        XCTAssertNil(T3MediaPreview.assetURL("http://127.0.0.1:1/api/orchestration/shell"))
        XCTAssertNil(T3MediaPreview.assetURL("https://user@host/api/assets/x"))
    }
}

_ = NSApplication.shared
let suite = XCTestSuite(forTestCaseClass: PanelsTests.self)
suite.run()
let run = suite.testRun!
print("Executed \(run.executionCount) tests, with \(run.totalFailureCount) failures")
exit(run.totalFailureCount == 0 ? 0 : 1)
