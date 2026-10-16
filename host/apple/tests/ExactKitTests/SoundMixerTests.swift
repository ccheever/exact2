// @ref LLP 1096 D6, D8, §5: the sound arm's C mixer, rendered offline. Given a
// buffer's host time and the source's presentation latency, a voice's first
// frame and its cut land on the frames D6 computes; voices sum; a slot frees
// at its end; an End for a freed slot is ignored; a full ring counts its drop.
// A voice's playback rate reads its sound by linear interpolation and moves
// its end; its pan places it by Web Audio's equal-power law (LLP 1125 D10).
// A second's lookahead fits: 288 slots and a ring of 1024 ops, and a voice
// retracted before it starts frees its slot at once (LLP 1125 D3).
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
            exact_sound_set(m, UInt32(i), p, frames, 1)
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
        XCTAssertTrue(exact_sound_push(m, 1, 7, 0, speaker + 100 / rate, 0.5, 1, 0))
        XCTAssertTrue(exact_sound_push(m, 2, 7, 0, speaker + 300 / rate, 0, 1, 0))
        let out = render(m, at: t)
        XCTAssertEqual(out.firstIndex { $0 != 0 }, 100)
        XCTAssertEqual(out[100], 0.25, accuracy: 1e-6)
        XCTAssertEqual(out[299], 0.25, accuracy: 1e-6)
        XCTAssertEqual(out[300], 0)
        XCTAssertEqual(exact_sound_live(m), 0, "a cut voice frees its slot")
        // An End for the freed slot is ignored.
        XCTAssertTrue(exact_sound_push(m, 2, 7, 0, speaker, 0, 1, 0))
        XCTAssertEqual(render(m, at: t + 512 / rate).filter { $0 != 0 }.count, 0)
    }

    func testVoicesSumAndAVoiceEndsWithItsFile() {
        let m = mixer(frames: 200)
        defer { exact_sound_mixer_free(m) }
        let t = exact_sound_seconds(ticks(20))
        XCTAssertTrue(exact_sound_push(m, 1, 1, 0, t + latency, 1, 1, 0))
        XCTAssertTrue(exact_sound_push(m, 1, 2, 1, t + latency + 50 / rate, 1, 1, 0))
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
        XCTAssertTrue(exact_sound_push(m, 1, 1, 0, t - 0.040, 1, 1, 0))
        let out = render(m, at: t)
        XCTAssertEqual(out[0], 0.5, accuracy: 1e-6, "its attack is never skipped")
        XCTAssertEqual(exact_sound_late(m), 1)
        // A voice waiting for a later buffer keeps its slot.
        XCTAssertTrue(exact_sound_push(m, 1, 2, 1, t + 1, 1, 1, 0))
        _ = render(m, at: t + 512 / rate)
        XCTAssertEqual(exact_sound_live(m), 2)
    }

    /// A stereo mixer whose sound 0 is `planes` (one plane per source channel, `frames` each),
    /// copied to both channels from a mono source as the arm does.
    private func stereo(_ planes: [[Float]]) -> OpaquePointer {
        let m = exact_sound_mixer(2, rate)!
        let frames = planes[0].count
        let p = UnsafeMutablePointer<Float>.allocate(capacity: 2 * frames)
        for c in 0..<2 { for f in 0..<frames { p[c * frames + f] = planes[min(c, planes.count - 1)][f] } }
        samples.append(p)
        exact_sound_set(m, 0, p, UInt32(frames), UInt32(planes.count))
        exact_sound_latency(m, latency)
        return m
    }

    /// One interleaved stereo buffer rendered at host second `at`, as (left, right) frames.
    private func renderStereo(_ m: OpaquePointer, at: Double, frames: Int = 512) -> [(Float, Float)] {
        var out = [Float](repeating: 0, count: 2 * frames)
        var stamp = AudioTimeStamp()
        stamp.mHostTime = ticks(at)
        stamp.mFlags = .hostTimeValid
        out.withUnsafeMutableBufferPointer { buffer in
            var list = AudioBufferList(mNumberBuffers: 1, mBuffers: AudioBuffer(mNumberChannels: 2, mDataByteSize: UInt32(2 * frames * 4), mData: buffer.baseAddress))
            _ = exact_sound_render(m, &stamp, UInt32(frames), &list)
        }
        return (0..<frames).map { (out[2 * $0], out[2 * $0 + 1]) }
    }

    func testAPlaybackRateInterpolatesBetweenFramesAndMovesTheEnd() {
        let m = exact_sound_mixer(1, rate)!
        defer { exact_sound_mixer_free(m) }
        // A ramp: frame i is i / 1000, for 200 frames.
        let p = UnsafeMutablePointer<Float>.allocate(capacity: 200)
        for i in 0..<200 { p[i] = Float(i) / 1000 }
        samples.append(p)
        exact_sound_set(m, 0, p, 200, 1)
        exact_sound_latency(m, latency)
        let t = exact_sound_seconds(ticks(50))
        // Half the rate: each output frame moves half a frame, so odd frames fall between two.
        XCTAssertTrue(exact_sound_push(m, 1, 1, 0, t + latency, 1, 0.5, 0))
        let slow = render(m, at: t)
        XCTAssertEqual(slow[1], 0.0005, accuracy: 1e-6)
        XCTAssertEqual(slow[2], 0.001, accuracy: 1e-6)
        XCTAssertEqual(slow[301], 0.1505, accuracy: 1e-6)
        XCTAssertEqual(slow[399], 0.0995, accuracy: 1e-6, "past the last frame is silence: halfway from 0.199 to 0")
        XCTAssertEqual(slow[400], 0, "200 frames at half the rate last 400")
        // Twice the rate: every other frame, and half as long.
        XCTAssertTrue(exact_sound_push(m, 1, 2, 0, t + 512 / rate + latency, 1, 2, 0))
        let fast = render(m, at: t + 512 / rate)
        XCTAssertEqual(fast[1], 0.002, accuracy: 1e-6)
        XCTAssertEqual(fast[99], 0.198, accuracy: 1e-6)
        XCTAssertEqual(fast[100], 0, "200 frames at twice the rate last 100")
        XCTAssertEqual(exact_sound_live(m), 0)
    }

    func testAPanPlacesAMonoSoundByTheEqualPowerLaw() {
        let m = stereo([[Float](repeating: 0.5, count: 100)])
        defer { exact_sound_mixer_free(m) }
        let t = exact_sound_seconds(ticks(60))
        XCTAssertTrue(exact_sound_push(m, 1, 1, 0, t + latency, 1, 1, -1))
        XCTAssertTrue(exact_sound_push(m, 1, 2, 0, t + latency + 200 / rate, 1, 1, 0.5))
        XCTAssertTrue(exact_sound_push(m, 1, 3, 0, t + latency + 400 / rate, 1, 1, 0))
        let out = renderStereo(m, at: t)
        XCTAssertEqual(out[50].0, 0.5, accuracy: 1e-6, "hard left")
        XCTAssertEqual(out[50].1, 0, accuracy: 1e-6)
        // x = (0.5 + 1) / 2 = 0.75.
        XCTAssertEqual(out[250].0, 0.5 * Float(cos(0.75 * Double.pi / 2)), accuracy: 1e-6)
        XCTAssertEqual(out[250].1, 0.5 * Float(sin(0.75 * Double.pi / 2)), accuracy: 1e-6)
        // A pan of 0 leaves the voice as it was: the mono sound on both sides, whole.
        XCTAssertEqual(out[450].0, 0.5, accuracy: 1e-6)
        XCTAssertEqual(out[450].1, 0.5, accuracy: 1e-6)
    }

    func testAPanOfAStereoSoundFoldsTheFarSideIn() {
        let m = stereo([[Float](repeating: 0.4, count: 100), [Float](repeating: 0.2, count: 100)])
        defer { exact_sound_mixer_free(m) }
        let t = exact_sound_seconds(ticks(70))
        XCTAssertTrue(exact_sound_push(m, 1, 1, 0, t + latency, 1, 1, -0.5))
        XCTAssertTrue(exact_sound_push(m, 1, 2, 0, t + latency + 200 / rate, 0.5, 1, 1))
        let out = renderStereo(m, at: t)
        // Left of centre, x = pan + 1 = 0.5: left keeps itself and takes the right times cos(x π/2).
        let g = Float(cos(Double.pi / 4))
        XCTAssertEqual(out[50].0, 0.4 + 0.2 * g, accuracy: 1e-6)
        XCTAssertEqual(out[50].1, 0.2 * g, accuracy: 1e-6)
        // Hard right, x = 1, at gain 0.5: the left folded into the right, nothing left.
        XCTAssertEqual(out[250].0, 0, accuracy: 1e-6)
        XCTAssertEqual(out[250].1, 0.5 * (0.2 + 0.4), accuracy: 1e-6)
    }

    /// A mono mixer whose sound 0 is `planes` as the arm stores them for a mono output:
    /// one plane for a mono file, both for a stereo one.
    private func mono(_ planes: [[Float]]) -> OpaquePointer {
        let m = exact_sound_mixer(1, rate)!
        let frames = planes[0].count
        let p = UnsafeMutablePointer<Float>.allocate(capacity: planes.count * frames)
        for (c, plane) in planes.enumerated() { for f in 0..<frames { p[c * frames + f] = plane[f] } }
        samples.append(p)
        exact_sound_set(m, 0, p, UInt32(frames), UInt32(planes.count))
        exact_sound_latency(m, latency)
        return m
    }

    func testOnAMonoOutputAPanIsPannedInStereoThenDownMixed() {
        // Web Audio: the panner's stereo output, then the destination's speaker down-mix, 0.5 × (L + R).
        let m = mono([[Float](repeating: 0.5, count: 100)])
        defer { exact_sound_mixer_free(m) }
        let t = exact_sound_seconds(ticks(80))
        XCTAssertTrue(exact_sound_push(m, 1, 1, 0, t + latency, 1, 1, -1))
        XCTAssertTrue(exact_sound_push(m, 1, 2, 0, t + latency + 200 / rate, 1, 1, 0.5))
        XCTAssertTrue(exact_sound_push(m, 1, 3, 0, t + latency + 400 / rate, 1, 1, 0))
        let out = render(m, at: t)
        XCTAssertEqual(out[50], 0.25, accuracy: 1e-6, "hard left: [0.5, 0], down-mixed")
        let x = 0.75 * Double.pi / 2
        XCTAssertEqual(out[250], Float(0.5 * 0.5 * (cos(x) + sin(x))), accuracy: 1e-6)
        XCTAssertEqual(out[450], 0.5, accuracy: 1e-6, "a pan of 0 takes no panner: the level it had")
    }

    func testOnAMonoOutputAStereoSoundIsAveragedAndItsPanFoldsFirst() {
        let m = mono([[Float](repeating: 0.4, count: 100), [Float](repeating: 0.2, count: 100)])
        defer { exact_sound_mixer_free(m) }
        let t = exact_sound_seconds(ticks(90))
        XCTAssertTrue(exact_sound_push(m, 1, 1, 0, t + latency, 1, 1, 0))
        XCTAssertTrue(exact_sound_push(m, 1, 2, 0, t + latency + 200 / rate, 1, 1, 0.5))
        XCTAssertTrue(exact_sound_push(m, 1, 3, 0, t + latency + 400 / rate, 0.5, 1, -1))
        let out = render(m, at: t)
        XCTAssertEqual(out[50], 0.3, accuracy: 1e-6, "unpanned: the two averaged, as before")
        // Right of centre, x = pan = 0.5: [L·g, R + L·g], g = cos(π/4) = sin(π/4).
        let g = Float(cos(Double.pi / 4))
        XCTAssertEqual(out[250], 0.5 * (0.4 * g + 0.2 + 0.4 * g), accuracy: 1e-6)
        // Hard left at gain 0.5: [L + R, 0], down-mixed.
        XCTAssertEqual(out[450], 0.5 * 0.5 * (0.4 + 0.2), accuracy: 1e-6)
    }

    func testAFullRingDropsAndCounts() {
        let m = mixer()
        defer { exact_sound_mixer_free(m) }
        for i in 0..<1024 { XCTAssertTrue(exact_sound_push(m, 2, UInt64(i), 0, 0, 0, 1, 0)) }
        XCTAssertFalse(exact_sound_push(m, 2, 999, 0, 0, 0, 1, 0))
        XCTAssertEqual(exact_sound_dropped(m), 1)
        _ = render(m, at: 40)
        XCTAssertTrue(exact_sound_push(m, 2, 999, 0, 0, 0, 1, 0), "a render empties the ring")
    }

    func testASecondAheadFitsAndARetractedVoiceFreesItsSlot() {
        let m = mixer()
        defer { exact_sound_mixer_free(m) }
        let t = exact_sound_seconds(ticks(50))
        // 288 voices waiting a second ahead, in slots; a 289th finds none.
        for i in 0..<288 { XCTAssertTrue(exact_sound_push(m, 1, UInt64(i + 1), 0, t + 0.5 + Double(i) / 1000, 0.1, 1, 0)) }
        XCTAssertTrue(exact_sound_push(m, 1, 289, 0, t + 0.9, 0.1, 1, 0))
        _ = render(m, at: t)
        XCTAssertEqual(exact_sound_live(m), 288)
        XCTAssertEqual(exact_sound_dropped(m), 1)
        // An edit retracts one before it starts: its slot is free in the same render, and a new voice takes it.
        XCTAssertTrue(exact_sound_push(m, 2, 288, 0, t + 0.5 + 287.0 / 1000, 0, 1, 0))
        XCTAssertTrue(exact_sound_push(m, 1, 290, 1, t + 0.95, 0.1, 1, 0))
        _ = render(m, at: t + 512 / rate)
        XCTAssertEqual(exact_sound_live(m), 288)
        XCTAssertEqual(exact_sound_dropped(m), 1, "the new voice took the freed slot")
    }
}
