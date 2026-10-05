// @ref LLP 1096 D6, D8, §5: the sound arm's C mixer, rendered offline. Given a
// buffer's host time and the source's presentation latency, a voice's first
// frame and its cut land on the frames D6 computes; voices sum; a slot frees
// at its end; an End for a freed slot is ignored; a full ring counts its drop.
import AudioToolbox
import ExactSoundRender
import XCTest

final class SoundMixerTests: XCTestCase {
    private let rate = 48_000.0
    private let latency = 0.001
    private var samples: [UnsafeMutablePointer<Float>] = []

    override func tearDown() { samples.forEach { $0.deallocate() }; samples = [] }

    /// A mixer with sound 0 = `frames` frames of 0.5 and sound 1 = 0.25, one channel.
    private func mixer(frames: UInt32 = 4_800) -> OpaquePointer {
        let m = exact_sound_mixer(1, rate)!
        for (i, value) in [Float(0.5), 0.25].enumerated() {
            let p = UnsafeMutablePointer<Float>.allocate(capacity: Int(frames))
            p.initialize(repeating: value, count: Int(frames))
            samples.append(p)
            exact_sound_set(m, UInt32(i), p, frames)
        }
        exact_sound_latency(m, latency)
        return m
    }

    /// The host ticks for `seconds` on mach_absolute_time's base.
    private func ticks(_ seconds: Double) -> UInt64 {
        var base = mach_timebase_info_data_t()
        mach_timebase_info(&base)
        return UInt64(seconds * 1e9 * Double(base.denom) / Double(base.numer))
    }

    /// One buffer of `frames` frames rendered at host second `at`.
    private func render(_ m: OpaquePointer, at: Double, frames: Int = 512) -> [Float] {
        var out = [Float](repeating: 0, count: frames)
        var stamp = AudioTimeStamp()
        stamp.mHostTime = ticks(at)
        stamp.mFlags = .hostTimeValid
        out.withUnsafeMutableBufferPointer { buffer in
            var list = AudioBufferList(mNumberBuffers: 1, mBuffers: AudioBuffer(mNumberChannels: 1, mDataByteSize: UInt32(frames * 4), mData: buffer.baseAddress))
            _ = exact_sound_render(m, &stamp, UInt32(frames), &list)
        }
        return out
    }

    func testFirstFrameAndCutLandWhereRunnerTimeMapsThem() {
        let m = mixer()
        defer { exact_sound_mixer_free(m) }
        // The buffer renders at t; its first frame reaches the speaker at t + latency.
        let t = exact_sound_seconds(ticks(10))
        let speaker = t + latency
        XCTAssertTrue(exact_sound_push(m, 1, 7, 0, speaker + 100 / rate, 0.5))
        XCTAssertTrue(exact_sound_push(m, 2, 7, 0, speaker + 300 / rate, 0))
        let out = render(m, at: t)
        XCTAssertEqual(out.firstIndex { $0 != 0 }, 100)
        XCTAssertEqual(out[100], 0.25, accuracy: 1e-6)
        XCTAssertEqual(out[299], 0.25, accuracy: 1e-6)
        XCTAssertEqual(out[300], 0)
        XCTAssertEqual(exact_sound_live(m), 0, "a cut voice frees its slot")
        // An End for the freed slot is ignored.
        XCTAssertTrue(exact_sound_push(m, 2, 7, 0, speaker, 0))
        XCTAssertEqual(render(m, at: t + 512 / rate).filter { $0 != 0 }.count, 0)
    }

    func testVoicesSumAndAVoiceEndsWithItsFile() {
        let m = mixer(frames: 200)
        defer { exact_sound_mixer_free(m) }
        let t = exact_sound_seconds(ticks(20))
        XCTAssertTrue(exact_sound_push(m, 1, 1, 0, t + latency, 1))
        XCTAssertTrue(exact_sound_push(m, 1, 2, 1, t + latency + 50 / rate, 1))
        let out = render(m, at: t)
        XCTAssertEqual(out[0], 0.5, accuracy: 1e-6)
        XCTAssertEqual(out[60], 0.75, accuracy: 1e-6)
        XCTAssertEqual(out[220], 0.25, accuracy: 1e-6, "the first voice's file ended at 200")
        XCTAssertEqual(out[260], 0, "the second's at 250")
        XCTAssertEqual(exact_sound_live(m), 0)
    }

    func testAVoiceInThePastStartsFromItsFirstFrame() {
        let m = mixer()
        defer { exact_sound_mixer_free(m) }
        let t = exact_sound_seconds(ticks(30))
        XCTAssertTrue(exact_sound_push(m, 1, 1, 0, t - 0.040, 1))
        let out = render(m, at: t)
        XCTAssertEqual(out[0], 0.5, accuracy: 1e-6, "its attack is never skipped")
        XCTAssertEqual(exact_sound_late(m), 1)
        // A voice waiting for a later buffer keeps its slot.
        XCTAssertTrue(exact_sound_push(m, 1, 2, 1, t + 1, 1))
        _ = render(m, at: t + 512 / rate)
        XCTAssertEqual(exact_sound_live(m), 2)
    }

    func testAFullRingDropsAndCounts() {
        let m = mixer()
        defer { exact_sound_mixer_free(m) }
        for i in 0..<256 { XCTAssertTrue(exact_sound_push(m, 2, UInt64(i), 0, 0, 0)) }
        XCTAssertFalse(exact_sound_push(m, 2, 999, 0, 0, 0))
        XCTAssertEqual(exact_sound_dropped(m), 1)
        _ = render(m, at: 40)
        XCTAssertTrue(exact_sound_push(m, 2, 999, 0, 0, 0), "a render empties the ring")
    }
}
