// Wait for UIKit's foreground eligibility before announcing agent readiness.
// SpringBoard can keep launch and fold overlays up after activation; the
// simulator driver waits for them separately (`host/apple/cloak.mjs`).
#if os(iOS) || os(tvOS)
import UIKit

extension Agent {
    /// The real-touch aim and display-read refusal, or nil. UIKit's
    /// [`foregroundActive`](https://developer.apple.com/documentation/uikit/uiscene/activationstate-swift.enum/foregroundactive)
    /// means "running in the foreground and is currently receiving events".
    /// The scene can still be inactive while SpringBoard brings the app in,
    /// or while a system alert, another app or the home screen covers it.
    /// Every build has one scene, so its activation state is the app's.
    static func frontRefusal(_ window: UIWindow?) -> String? {
        if window?.windowScene?.activationState != .foregroundActive { return "the app's scene is not foreground-active" }
        if window?.isKeyWindow != true { return "the app's window is not key" }
        return nil
    }

    /// More than three times the longest measured foreground wait, 4.4 s on
    /// an iPhone 18 Pro simulator with iOS 27.0. Stay under the driver's 20 s
    /// bound on `ready` so a launch that never comes in front reaches the
    /// tap's refusal rather than the driver's timeout.
    static let frontLimit: TimeInterval = 15

    /// Wait off the main thread for an active scene and key window, allowing
    /// UIKit to activate the scene while the driver waits for `ready`.
    /// Keep the agent available for diagnostics if the deadline expires.
    static func awaitFront(_ window: @escaping () -> UIWindow?) {
        let deadline = DispatchTime.now() + frontLimit
        let shown = DispatchSemaphore(value: 0)
        let notifications = NotificationCenter.default
        let observers = DispatchQueue.main.sync { () -> [NSObjectProtocol] in
            let check = { if frontRefusal(window()) == nil { shown.signal() } }
            let names = [UIScene.didActivateNotification, UIWindow.didBecomeKeyNotification]
            let tokens = names.map { notifications.addObserver(forName: $0, object: nil, queue: .main) { _ in check() } }
            check()
            return tokens
        }
        if shown.wait(timeout: deadline) == .timedOut, let refusal = DispatchQueue.main.sync(execute: { frontRefusal(window()) }) {
            FileHandle.standardError.write(Data("exact: \(refusal) after \(Int(frontLimit)) s; sending \"ready\" for diagnostics; real taps remain refused\n".utf8))
        }
        DispatchQueue.main.async { observers.forEach(notifications.removeObserver) }
    }
}
#endif
