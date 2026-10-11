#if os(macOS)
import Foundation

/// Lane r11-device: stream.ts decode's soft queue (MIT reference, see LICENSE-T3:
/// packages/client-runtime/src/device/stream.ts SOFT_DECODE_QUEUE). The reference recovers when
/// more than 8 samples wait for the decoder: iOS falls back to MJPEG (no 3D) and Android restarts
/// its decoder. Its decoder is fed as the network delivers. Here samples reach the decoder from
/// the main thread, so a stalled main thread (the 3D phone's first scene build on a loaded Mac, a
/// long layout pass) hands over everything that arrived meanwhile at once: more than 8 waiting
/// samples although the decoder keeps up, and the phone fell back to flat on its first open.
///
/// A backlog over the soft queue therefore opens a window; the feed is behind only when the
/// backlog is still over it a `window` later and has not shrunk. A burst drains within it; a
/// decoder slower than the stream does not, and recovers as the reference's does.
struct R11DecodeBacklog {
    static let window: TimeInterval = 1
    private var since: (at: TimeInterval, pending: Int)?

    /// Whether the decoder has fallen behind, given the samples it holds now.
    mutating func behind(pending: Int, soft: Int = R7H264.softDecodeQueue, now: TimeInterval) -> Bool {
        guard pending > soft else { since = nil; return false }
        guard let start = since else { since = (now, pending); return false }
        guard now - start.at >= Self.window else { return false }
        // Shrinking: still draining a burst. Measure the next window from here.
        if pending < start.pending { since = (now, pending); return false }
        return true
    }

    mutating func reset() { since = nil }
}
#endif
