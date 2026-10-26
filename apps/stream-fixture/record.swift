// Records the stream fixture's H.264 (LLP 1133.003 §9): ten seconds of a
// scrolling screen at 60 fps, encoded by VideoToolbox with serve-sim's
// settings (`packages/serve-sim/Sources/SimNative/H264Encoder.swift`:
// real time, High profile, no frame reordering, low-latency rate control,
// 6 Mbit/s, a key frame every 5 s), once at an iPhone's screen size and
// once at an Android emulator's. `serve.mjs` runs it when its recordings
// are missing and replays them in a loop; ten seconds is two key-frame
// intervals, so the loop restarts at a key frame.
//
//   xcrun swift apps/stream-fixture/record.swift <out-dir>
//
// Writes `ios.avcc`, serve-sim's `/stream.avcc` body (a `u32` big-endian
// length, a tag byte: 1 the `avcC` description, 2 a key frame, 3 a delta;
// the AVCC payload), and `android.semu`, serve-emu's WebSocket messages
// (each a 16-byte header, magic `SEMU`, version 1, a key flag and a `u64`
// presentation time in microseconds, over Annex-B with the parameter sets
// before each key frame), each message after a `u32` big-endian length.
import CoreMedia
import CoreVideo
import Foundation
import VideoToolbox

let out = URL(fileURLWithPath: CommandLine.arguments.dropFirst().first ?? ".")
let fps: Int32 = 60, seconds: Int32 = 10, bitrate = 6_000_000

struct Sample { let key: Bool; let avcc: Data; let format: CMFormatDescription }

func be32(_ v: UInt32) -> Data { withUnsafeBytes(of: v.bigEndian) { Data($0) } }
func be64(_ v: UInt64) -> Data { withUnsafeBytes(of: v.bigEndian) { Data($0) } }

/// A tall page of light rows with darker "text" runs, scrolled 6 px a
/// frame: what a list scrolling in a simulator looks like to an encoder.
func page(width: Int, height: Int) -> [UInt8] {
  var rng: UInt64 = 0x9E37_79B9_7F4A_7C15
  func next() -> UInt64 { rng ^= rng << 13; rng ^= rng >> 7; rng ^= rng << 17; return rng }
  var bytes = [UInt8](repeating: 0xF4, count: width * height * 4)
  var y = 0
  while y < height {
    let rowHeight = 44 + Int(next() % 40)
    let shade = UInt8(0xE0 + next() % 0x18)
    for yy in y..<min(height, y + 2) { for x in 0..<width { bytes[(yy * width + x) * 4 + 1] = shade } }
    var x = 32
    while x < width - 64 {
      let run = 8 + Int(next() % 90)
      let ink = UInt8(0x20 + next() % 0x60)
      for yy in (y + 14)..<min(height, y + 28) {
        for xx in x..<min(width - 32, x + run) {
          let i = (yy * width + xx) * 4
          bytes[i] = ink; bytes[i + 1] = ink; bytes[i + 2] = UInt8(min(255, Int(ink) + 40))
        }
      }
      x += run + 6 + Int(next() % 12)
    }
    y += rowHeight
  }
  return bytes
}

func record(width: Int, height: Int) -> [Sample] {
  var samples = [Sample]()
  let lock = NSLock()
  var session: VTCompressionSession?
  let spec = [kVTVideoEncoderSpecification_EnableLowLatencyRateControl: kCFBooleanTrue!] as CFDictionary
  let status = VTCompressionSessionCreate(
    allocator: nil, width: Int32(width), height: Int32(height), codecType: kCMVideoCodecType_H264,
    encoderSpecification: spec, imageBufferAttributes: nil, compressedDataAllocator: nil,
    outputCallback: nil, refcon: nil, compressionSessionOut: &session)
  guard status == noErr, let session else { fatalError("VTCompressionSessionCreate \(status)") }
  let properties: [(CFString, CFTypeRef)] = [
    (kVTCompressionPropertyKey_RealTime, kCFBooleanTrue!),
    (kVTCompressionPropertyKey_ProfileLevel, kVTProfileLevel_H264_High_AutoLevel),
    (kVTCompressionPropertyKey_AllowFrameReordering, kCFBooleanFalse!),
    (kVTCompressionPropertyKey_AverageBitRate, NSNumber(value: bitrate)),
    (kVTCompressionPropertyKey_ExpectedFrameRate, NSNumber(value: fps)),
    (kVTCompressionPropertyKey_MaxKeyFrameInterval, NSNumber(value: fps * 5)),
  ]
  for (key, value) in properties { VTSessionSetProperty(session, key: key, value: value) }
  VTCompressionSessionPrepareToEncodeFrames(session)

  var state: UInt64 = 0x2545_F491_4F6C_DD1D
  func noise() -> UInt64 { state ^= state << 13; state ^= state >> 7; state ^= state << 17; return state >> 24 }
  let tall = height * 4
  let source = page(width: width, height: tall)
  for frame in 0..<Int(fps * seconds) {
    var buffer: CVPixelBuffer?
    CVPixelBufferCreate(nil, width, height, kCVPixelFormatType_32BGRA,
                        [kCVPixelBufferIOSurfacePropertiesKey: [:]] as CFDictionary, &buffer)
    guard let buffer else { fatalError("CVPixelBufferCreate") }
    CVPixelBufferLockBaseAddress(buffer, [])
    let base = CVPixelBufferGetBaseAddress(buffer)!.assumingMemoryBound(to: UInt8.self)
    let stride = CVPixelBufferGetBytesPerRow(buffer)
    let offset = (frame * 6) % (tall - height)
    source.withUnsafeBufferPointer { src in
      for y in 0..<height {
        memcpy(base + y * stride, src.baseAddress! + (offset + y) * width * 4, width * 4)
      }
    }
    // A playing video in the page: new 8-pixel blocks every frame, which
    // keep the encoder near its bit rate, a busier screen than a list.
    let (top, bottom) = (height / 3, height / 3 + height / 10)
    for by in Swift.stride(from: top, to: bottom, by: 8) {
      for bx in Swift.stride(from: width / 8, to: width * 7 / 8, by: 8) {
        let v = UInt8(112 + noise() % 32)
        for y in by..<min(bottom, by + 8) { memset(base + y * stride + bx * 4, Int32(v), 32) }
      }
    }
    CVPixelBufferUnlockBaseAddress(buffer, [])
    let pts = CMTime(value: CMTimeValue(frame), timescale: fps)
    let force = frame % Int(fps * 5) == 0
    let options = force ? [kVTEncodeFrameOptionKey_ForceKeyFrame: kCFBooleanTrue!] as CFDictionary : nil
    VTCompressionSessionEncodeFrame(session, imageBuffer: buffer, presentationTimeStamp: pts,
                                    duration: CMTime(value: 1, timescale: fps), frameProperties: options,
                                    infoFlagsOut: nil) { status, _, sample in
      guard status == noErr, let sample, let block = CMSampleBufferGetDataBuffer(sample),
            let format = CMSampleBufferGetFormatDescription(sample) else { return }
      let attachments = CMSampleBufferGetSampleAttachmentsArray(sample, createIfNecessary: false) as? [[CFString: Any]]
      let key = !(attachments?.first?[kCMSampleAttachmentKey_NotSync] as? Bool ?? false)
      var length = 0
      var pointer: UnsafeMutablePointer<CChar>?
      CMBlockBufferGetDataPointer(block, atOffset: 0, lengthAtOffsetOut: nil, totalLengthOut: &length, dataPointerOut: &pointer)
      let data = Data(bytes: pointer!, count: length)
      lock.lock(); samples.append(Sample(key: key, avcc: data, format: format)); lock.unlock()
    }
    // Real time: one frame in flight, as serve-sim's encoder awaits each.
    VTCompressionSessionCompleteFrames(session, untilPresentationTimeStamp: pts)
  }
  VTCompressionSessionCompleteFrames(session, untilPresentationTimeStamp: .invalid)
  VTCompressionSessionInvalidate(session)
  return samples
}

func avcC(_ format: CMFormatDescription) -> Data {
  let atoms = CMFormatDescriptionGetExtension(format, extensionKey: kCMFormatDescriptionExtension_SampleDescriptionExtensionAtoms) as? [String: Any]
  guard let data = atoms?["avcC"] as? Data else { fatalError("no avcC") }
  return data
}

func parameterSets(_ format: CMFormatDescription) -> [Data] {
  var count = 0
  CMVideoFormatDescriptionGetH264ParameterSetAtIndex(format, parameterSetIndex: 0, parameterSetPointerOut: nil,
                                                     parameterSetSizeOut: nil, parameterSetCountOut: &count, nalUnitHeaderLengthOut: nil)
  return (0..<count).map { i in
    var pointer: UnsafePointer<UInt8>?
    var size = 0
    CMVideoFormatDescriptionGetH264ParameterSetAtIndex(format, parameterSetIndex: i, parameterSetPointerOut: &pointer,
                                                       parameterSetSizeOut: &size, parameterSetCountOut: nil, nalUnitHeaderLengthOut: nil)
    return Data(bytes: pointer!, count: size)
  }
}

func annexB(_ avcc: Data) -> Data {
  var out = Data(capacity: avcc.count + 16)
  var at = avcc.startIndex
  while at + 4 <= avcc.endIndex {
    let n = Int(avcc[at]) << 24 | Int(avcc[at + 1]) << 16 | Int(avcc[at + 2]) << 8 | Int(avcc[at + 3])
    out.append(contentsOf: [0, 0, 0, 1]); out.append(avcc[(at + 4)..<(at + 4 + n)])
    at += 4 + n
  }
  return out
}

func envelope(_ tag: UInt8, _ payload: Data) -> Data { be32(UInt32(payload.count + 1)) + Data([tag]) + payload }

try FileManager.default.createDirectory(at: out, withIntermediateDirectories: true)

let ios = record(width: 1206, height: 2622)
var avccBody = envelope(1, avcC(ios[0].format))
for s in ios { avccBody += envelope(s.key ? 2 : 3, s.avcc) }
try avccBody.write(to: out.appendingPathComponent("ios.avcc"))

let android = record(width: 1080, height: 2400)
var semu = Data()
for (i, s) in android.enumerated() {
  var unit = Data()
  if s.key { for p in parameterSets(s.format) { unit += Data([0, 0, 0, 1]) + p } }
  unit += annexB(s.avcc)
  let header = Data("SEMU".utf8) + Data([1, s.key ? 1 : 0, 0, 0]) + be64(UInt64(i) * 16_667)
  semu += be32(UInt32(header.count + unit.count)) + header + unit
}
try semu.write(to: out.appendingPathComponent("android.semu"))

func summary(_ s: [Sample]) -> String {
  let keys = s.filter(\.key).map(\.avcc.count), deltas = s.filter { !$0.key }.map(\.avcc.count)
  return "\(s.count) frames, \(keys.count) key (max \(keys.max() ?? 0) B), deltas mean \(deltas.reduce(0, +) / max(1, deltas.count)) B"
}
print("ios.avcc: \(summary(ios)); android.semu: \(summary(android))")
