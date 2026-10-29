import Foundation

/// Wall-clock boundaries for one opt-in asynchronous tick, on the display's monotonic clock.
/// These measure elapsed time, including descheduling and any synchronous host callbacks.
package struct FrameWorkTiming {
    package var capture: UInt64
    package var frame: Double?
    package var queued: Double
    package var began: Double
    package var finished: Double
    package var landed: Double = 0
}
