// What a window is called once something is open.
//
// The folder rather than the file: a reader shows the filename in its own
// chrome, and it is the *project* you have open that you pick out of a
// window list. @ref LLP 1033 D7
#if os(macOS)
import AppKit
import XCTest
@testable import ExactKit

final class WindowTitleTests: XCTestCase {
    /// A corpus directory is titled by the folder it lives in — which is the
    /// repository, and the reason this rule was chosen.
    func testACorpusIsTitledByTheProjectAroundIt() {
        XCTAssertEqual(
            ExactDocuments.windowTitle(for: "/Users/x/projects/exact2/llp"),
            "\(ExactEnv.appName) — exact2"
        )
    }

    /// A file is titled by the folder that holds it, for the same reason.
    func testAFileIsTitledByItsFolder() {
        XCTAssertEqual(
            ExactDocuments.windowTitle(for: "/Users/x/projects/exact2/README.md"),
            "\(ExactEnv.appName) — exact2"
        )
    }

    /// A trailing slash is not a folder called "".
    func testATrailingSlashIsNotAName() {
        XCTAssertEqual(
            ExactDocuments.windowTitle(for: "/Users/x/projects/exact2/llp/"),
            "\(ExactEnv.appName) — exact2"
        )
    }

    /// Nothing above it is just the app: a title with a dangling dash reads
    /// like a bug, and at the root there is no project to name.
    func testTheRootIsJustTheApp() {
        XCTAssertEqual(ExactDocuments.windowTitle(for: "/"), ExactEnv.appName)
        XCTAssertEqual(ExactDocuments.windowTitle(for: ""), ExactEnv.appName)
    }
}
#endif
