// A session booted for a wake, with no view presented (LLP 1133.017 D7).
#if os(iOS)
import UIKit

extension ExactSession {
    /// Boot for a wake that found no module ready: the plan at the screen's
    /// size, its page hidden, then at once what first pixel opens on a
    /// launch with a view (the app's module, its extensions, the data
    /// module), since nothing presents and no pixel waits. A launch in the
    /// background may connect a scene, whose view is never drawn there, so a
    /// session already booted under one is opened the same way. A view that
    /// is attached or drawn later takes the session as it stands, and its
    /// next layout tells the runner its size and insets.
    ///
    /// The session's `firstDrawMs` is then this activation's time, and the
    /// update store's after-pixel check runs in the wake. In front, a wake
    /// waits for first pixel, as every event of a launch with a view does.
    public func bootForWake() {
        guard state != .destroyed, UIApplication.shared.applicationState == .background else { return }
        let started = ExactEnv.wall()
        if !booted {
            boot(size: UIScreen.main.bounds.size)
            guard booted else { return WakeStore.log("wakes: the boot with no view was refused: \(bootError ?? "?")") }
            WakeStore.log("wakes: booted \(view == nil ? "with no view" : "under an undrawn view") in \(Int(ExactEnv.wall() - started)) ms")
        }
        guard firstDrawMs == nil else { return }
        WakeStore.log("wakes: the module and the data open for the wake, \(view == nil ? "no view" : "a view") attached")
        firstDrawn(generation: generation, token: updateToken)
    }
}
#endif
