import XCTest
@testable import ExactKit

final class ActivityLoadingTests: XCTestCase {
    func testMissingArmReportsLoaderReasonOnEachAttempt() {
        let path = "/missing-wp9-activity-arm.dylib"
        for _ in 0..<2 {
            XCTAssertThrowsError(try ActivityLibrary.load(path: path)) { error in
                let message = String(describing: error)
                XCTAssertTrue(message.contains(path), message)
                XCTAssertTrue(message.contains("dlopen"), message)
            }
        }
    }
    func testMissingEntryReportsItsSymbol() {
        XCTAssertThrowsError(try ActivityLibrary.load(path: "/usr/lib/libSystem.B.dylib")) { error in
            let message = String(describing: error)
            XCTAssertTrue(message.contains("exact_activity_open"), message)
            XCTAssertTrue(message.contains("dlsym"), message)
        }
    }
}
