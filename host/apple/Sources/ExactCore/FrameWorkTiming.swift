import Foundation

/// Monotonic timestamps for an asynchronous frame update. Elapsed durations include
/// descheduling, batch decoding and synchronous host callbacks.
package struct FrameWorkTiming {
    package var capture: UInt64
    package var frame: Double?
    package var queued: Double
    package var began: Double
    package var finished: Double
    package var landed: Double = 0
}
