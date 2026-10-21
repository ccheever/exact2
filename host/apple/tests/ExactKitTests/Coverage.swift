import Foundation
import XCTest

/// A prerequisite this run lacks (a built module, a font, Metal, a scene):
/// thrown, a skip that names it, and under EXACT_REQUIRED=1, which the async
/// lane sets (scripts/coverage.mjs; forwarded by build.mjs as
/// TEST_RUNNER_EXACT_REQUIRED), a failure, so a lane run cannot pass having
/// skipped what it tests. A test that does not apply here (an OS version)
/// still throws `XCTSkip` itself.
func missing(_ kind: String, _ why: String) -> Error {
    let message = "missing \(kind): \(why)"
    return ProcessInfo.processInfo.environment["EXACT_REQUIRED"] == "1" ? MissingPrerequisite(message: message) : XCTSkip(message)
}

struct MissingPrerequisite: Error, CustomStringConvertible {
    let message: String
    var description: String { "\(message) (EXACT_REQUIRED=1)" }
}

/// A fixture's plan, compiled from `scripts/fixtures/<name>.contract` at this commit by
/// `build.mjs --test` and named in `EXACT_<KEY>_PLAN`.
func fixturePlan(_ key: String) throws -> Data {
    guard let path = ProcessInfo.processInfo.environment["EXACT_\(key)_PLAN"] else {
        throw missing("fixture", "no EXACT_\(key)_PLAN: build.mjs --test compiles the fixtures in scripts/fixtures")
    }
    return try Data(contentsOf: URL(fileURLWithPath: path))
}
