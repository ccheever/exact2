#if os(iOS)
import UIKit
import XCTest
@testable import ExactKit
@testable import ExactSurfaces

/// A canvas hears the session through its one owner (LLP 1096 D8), which
/// follows the iOS 27 session life cycle.
final class CanvasAudioIOSTests: XCTestCase {
    override class func setUp() { super.setUp(); ExactSurfaces.install() } // LLP 1047.001 D4

    /// On an iPhone on iOS 27 a race was silent: an interruption began while
    /// the app was in the background, its end never came, and the canvas
    /// never asked for the session again.
    func testAnInterruptionWhoseEndNeverCameIsOverWhenTheAppIsActiveAgain() {
        let s = ExactApp.shared.makeSession(label: "canvas-audio-ios")
        let window = UIWindow(frame: UIScreen.main.bounds)
        window.addSubview(s.presenter.viewport)
        window.isHidden = false
        defer { window.isHidden = true; s.destroy() }
        var attempts = 0
        let lifecycle = CanvasLifecycle(s.surfaceHost, activate: { attempts += 1; return true })
        defer { lifecycle.interruption(began: false, shouldResume: true) }
        XCTAssertFalse(lifecycle.hidden)
        lifecycle.interruption(began: true, shouldResume: false)
        lifecycle.requestAudio(userInitiated: false)
        XCTAssertEqual(attempts, 0, "an interruption under way keeps the session")

        NotificationCenter.default.post(name: UIApplication.didBecomeActiveNotification, object: nil)
        XCTAssertEqual(attempts, 1, "no other app plays, so the canvas takes the session back")
    }
}
#endif
