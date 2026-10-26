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

/// For a test that needs Caltrain's archive: its plan calls Caltrain's data
/// functions (`stations`, `nextTrains`) or it drives Caltrain's own plan.
/// `build.mjs --test` links Caltrain's archive by default; with another app's,
/// the test does not apply, so it skips even under EXACT_REQUIRED.
func caltrainArchive() throws {
    #if !EXACT_LINKS_CALTRAIN
    throw XCTSkip("n/a: needs Caltrain's archive, and this run links another app's (`bun host/apple/build.mjs --test caltrain-apple` links it)")
    #endif
}

/// A fixture's plan, compiled from `scripts/fixtures/<name>.contract` at this commit by
/// `build.mjs --test` and named in `EXACT_<KEY>_PLAN`.
func fixturePlan(_ key: String) throws -> Data {
    guard let path = ProcessInfo.processInfo.environment["EXACT_\(key)_PLAN"] else {
        throw missing("fixture", "no EXACT_\(key)_PLAN: build.mjs --test compiles the fixtures in scripts/fixtures")
    }
    return try Data(contentsOf: URL(fileURLWithPath: path))
}

#if os(iOS)
import UIKit

/// Wait for the hosted test app's scene to connect before checking its prerequisites.
func hostedWindowScene() -> UIWindowScene? {
    MainActor.assumeIsolated {
        func connected() -> UIWindowScene? {
            UIApplication.shared.connectedScenes.compactMap { $0 as? UIWindowScene }.first
        }
        guard Bundle.main.object(forInfoDictionaryKey: "UIApplicationSceneManifest") != nil else { return connected() }
        let deadline = Date().addingTimeInterval(5)
        while connected() == nil, Date() < deadline {
            RunLoop.main.run(mode: .default, before: Date().addingTimeInterval(0.01))
        }
        return connected()
    }
}
#endif
