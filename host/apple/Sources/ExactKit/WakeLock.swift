// @ref LLP 1125 D11. `wakelock="screen"`: the display does not dim or lock
// for idleness while any session's displayed element says so. The runner
// decides which elements hold it (`exact_runner::wakelock`, the agent's
// `state.wakelock`); a batch says when that changes, and each presenter
// holds or lets go here. The process's lock is taken by the first holder and
// given back with the last: on iOS and tvOS UIKit's idle timer, on the Mac a
// power-management assertion against idle display sleep.
import Foundation
#if os(macOS)
import IOKit.pwr_mgt
#else
import UIKit
#endif

package enum WakeLock {
    nonisolated(unsafe) private static var holders: Set<ObjectIdentifier> = []
    #if os(macOS)
    nonisolated(unsafe) private static var assertion: IOPMAssertionID = 0
    #endif

    /// Whether any holder holds it now (tests).
    static var held: Bool { !holders.isEmpty }

    /// `holder` (a session's presenter) holds the lock or lets it go.
    static func hold(_ holder: AnyObject, _ on: Bool) {
        let was = held
        if on { holders.insert(ObjectIdentifier(holder)) } else { holders.remove(ObjectIdentifier(holder)) }
        if held != was { platform(held) }
    }

    private static func platform(_ on: Bool) {
        #if os(macOS)
        if on {
            let result = IOPMAssertionCreateWithName(kIOPMAssertionTypePreventUserIdleDisplaySleep as CFString,
                                                     IOPMAssertionLevel(kIOPMAssertionLevelOn), "wakelock=\"screen\"" as CFString, &assertion)
            if result != kIOReturnSuccess { assertion = 0; fputs("exact: wakelock: the display-sleep assertion was refused (\(result))\n", stderr) }
        } else if assertion != 0 {
            IOPMAssertionRelease(assertion)
            assertion = 0
        }
        #else
        // The agent keeps the idle timer off for the whole drive
        // (`Agent.startSocket`): the last holder leaves it so.
        if on || !ExactEnv.agentMode { UIApplication.shared.isIdleTimerDisabled = on }
        #endif
    }
}

extension Presenter {
    /// The batch's word on the wake lock, when it says one.
    func applyWakeLock(_ batch: Batch) {
        if let held = batch.wakelock { WakeLock.hold(self, held) }
    }
}
