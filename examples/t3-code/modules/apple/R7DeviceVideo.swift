#if os(macOS)
import AppKit
import CoreMedia
import Foundation
import VideoToolbox

/// Lane r7-device: the hub's H.264 streams and device keyboard input (MIT reference, see
/// LICENSE-T3: packages/client-runtime/src/device/stream.ts). serve-sim (iOS) sends an HTTP
/// `stream.avcc` body of `u32be length, u8 tag, payload` envelopes (1 avcC description, 2
/// keyframe, 3 delta, 4 JPEG seed); serve-emu (Android) sends H.264 access units in Annex B over
/// `ws?device=<serial>&frame-meta=1`, each behind a 16-byte "SEMU" header (magic, version, key
/// flag, pts). Both are decoded here with VideoToolbox, where the reference uses WebCodecs.
enum R7H264 {
    static let semuMagic: UInt32 = 0x53454d55
    static let semuHeaderBytes = 16
    static let softDecodeQueue = 8

    struct Semu: Equatable { let data: Data; let isKey: Bool?; let timestamp: UInt64? }

    /// parseSemuPacket: metadata and the Annex-B payload, or the raw bytes when unframed.
    static func parseSemu(_ raw: Data) -> Semu {
        let bytes = [UInt8](raw)
        if bytes.count > semuHeaderBytes, be32(bytes, 0) == semuMagic, bytes[4] == 1 {
            let pts = (UInt64(be32(bytes, 8)) << 32) | UInt64(be32(bytes, 12))
            return Semu(data: Data(bytes[semuHeaderBytes...]), isKey: bytes[5] & 1 != 0, timestamp: pts)
        }
        return Semu(data: raw, isKey: nil, timestamp: nil)
    }

    static func semu(_ payload: Data, key: Bool, pts: UInt64) -> Data {
        var header = [UInt8](repeating: 0, count: semuHeaderBytes)
        put32(&header, 0, semuMagic); header[4] = 1; header[5] = key ? 1 : 0
        put32(&header, 8, UInt32(pts >> 32)); put32(&header, 12, UInt32(pts & 0xffff_ffff))
        return Data(header) + payload
    }

    /// The NAL units of an Annex-B access unit, without their start codes.
    static func nalUnits(_ data: Data) -> [Data] {
        let bytes = [UInt8](data)
        var starts: [(code: Int, body: Int)] = []
        var index = 0
        while index + 2 < bytes.count {
            if bytes[index] == 0, bytes[index + 1] == 0 {
                if bytes[index + 2] == 1 { starts.append((index, index + 3)); index += 3; continue }
                if index + 3 < bytes.count, bytes[index + 2] == 0, bytes[index + 3] == 1 { starts.append((index, index + 4)); index += 4; continue }
            }
            index += 1
        }
        return starts.enumerated().compactMap { offset, start in
            let end = offset + 1 < starts.count ? starts[offset + 1].code : bytes.count
            return end > start.body ? Data(bytes[start.body..<end]) : nil
        }
    }

    /// scanAccessUnit: the keyframe flag (an IDR slice) and the SPS / PPS it carries.
    static func scan(_ data: Data) -> (isKey: Bool, sps: Data?, pps: Data?) {
        var isKey = false, sps: Data?, pps: Data?
        for nal in nalUnits(data) {
            switch nal.first.map({ $0 & 0x1f }) {
            case 5: isKey = true
            case 7: if sps == nil { sps = nal }
            case 8: if pps == nil { pps = nal }
            default: break
            }
        }
        return (isKey, sps, pps)
    }

    /// Annex B → length-prefixed samples, leaving out parameter sets and delimiters (they live in
    /// the format description).
    static func avccSample(annexB: Data) -> Data {
        var out = Data()
        for nal in nalUnits(annexB) {
            guard let type = nal.first.map({ $0 & 0x1f }), type != 7, type != 8, type != 9 else { continue }
            var length = [UInt8](repeating: 0, count: 4)
            put32(&length, 0, UInt32(nal.count))
            out.append(contentsOf: length); out.append(nal)
        }
        return out
    }

    /// The SPS / PPS and NAL length size of an avcC record (ISO/IEC 14496-15).
    static func parameterSets(avcC: Data) -> (sps: [Data], pps: [Data], nalLength: Int)? {
        let bytes = [UInt8](avcC)
        guard bytes.count >= 7, bytes[0] == 1 else { return nil }
        let nalLength = Int(bytes[4] & 0x03) + 1
        var index = 5, sps: [Data] = [], pps: [Data] = []
        let spsCount = Int(bytes[index] & 0x1f); index += 1
        for _ in 0..<spsCount {
            guard index + 2 <= bytes.count else { return nil }
            let length = Int(bytes[index]) << 8 | Int(bytes[index + 1]); index += 2
            guard index + length <= bytes.count else { return nil }
            sps.append(Data(bytes[index..<index + length])); index += length
        }
        guard index < bytes.count else { return nil }
        let ppsCount = Int(bytes[index]); index += 1
        for _ in 0..<ppsCount {
            guard index + 2 <= bytes.count else { return nil }
            let length = Int(bytes[index]) << 8 | Int(bytes[index + 1]); index += 2
            guard index + length <= bytes.count else { return nil }
            pps.append(Data(bytes[index..<index + length])); index += length
        }
        return sps.isEmpty || pps.isEmpty ? nil : (sps, pps, nalLength)
    }

    /// An avcC record for one SPS and PPS (what serve-sim's description envelope carries).
    static func avcC(sps: Data, pps: Data) -> Data {
        let s = [UInt8](sps)
        var out: [UInt8] = [1, s.count > 1 ? s[1] : 0x42, s.count > 2 ? s[2] : 0, s.count > 3 ? s[3] : 0x1e, 0xff, 0xe1]
        out += [UInt8(sps.count >> 8), UInt8(sps.count & 0xff)] + s
        out += [1, UInt8(pps.count >> 8), UInt8(pps.count & 0xff)] + [UInt8](pps)
        return Data(out)
    }

    /// avcCodecString: `avc1.PPCCLL` from an avcC record or an SPS NAL.
    static func codecString(_ data: Data) -> String {
        let bytes = [UInt8](data)
        guard bytes.count >= 4 else { return "avc1.42E01E" }
        return String(format: "avc1.%02x%02x%02x", bytes[1], bytes[2], bytes[3])
    }

    /// AvccDemuxer: complete envelopes out of a fragmented AVCC body.
    final class Demuxer {
        enum Kind: UInt8 { case description = 1, keyframe = 2, delta = 3, seed = 4 }
        private var buffer = Data()
        func push(_ data: Data) -> [(kind: Kind, payload: Data)] {
            buffer.append(data)
            var chunks: [(kind: Kind, payload: Data)] = [], offset = buffer.startIndex
            while buffer.endIndex - offset >= 4 {
                let length = Int(buffer[offset]) << 24 | Int(buffer[offset + 1]) << 16 | Int(buffer[offset + 2]) << 8 | Int(buffer[offset + 3])
                guard buffer.endIndex - offset - 4 >= length else { break }
                if length >= 1, let kind = Kind(rawValue: buffer[offset + 4]) {
                    chunks.append((kind, Data(buffer[(offset + 5)..<(offset + 4 + length)])))
                }
                offset += 4 + length
            }
            buffer = Data(buffer[offset...])
            return chunks
        }
        func reset() { buffer = Data() }
        static func envelope(_ kind: Kind, _ payload: Data) -> Data {
            var head = [UInt8](repeating: 0, count: 5)
            put32(&head, 0, UInt32(payload.count + 1)); head[4] = kind.rawValue
            return Data(head) + payload
        }
    }

    static func be32(_ bytes: [UInt8], _ at: Int) -> UInt32 {
        UInt32(bytes[at]) << 24 | UInt32(bytes[at + 1]) << 16 | UInt32(bytes[at + 2]) << 8 | UInt32(bytes[at + 3])
    }
    static func put32(_ bytes: inout [UInt8], _ at: Int, _ value: UInt32) {
        bytes[at] = UInt8(value >> 24); bytes[at + 1] = UInt8((value >> 16) & 0xff); bytes[at + 2] = UInt8((value >> 8) & 0xff); bytes[at + 3] = UInt8(value & 0xff)
    }
}

/// One VideoToolbox session. Frames come back on the main queue as CGImages; a decode error is
/// reported once per session (stream.ts recoverDecoder decides what to do).
final class R7H264Decoder {
    private var session: VTDecompressionSession?
    private var format: CMVideoFormatDescription?
    private let queue = DispatchQueue(label: "r7-device-decode")
    private var pending = 0
    private let lock = NSLock()
    private var failed = false
    private(set) var nalLength = 4
    let onFrame: (CGImage) -> Void
    let onError: () -> Void
    var configured: Bool { session != nil }
    /// decodeQueueSize: samples handed over and not yet decoded (counted on the decode queue).
    var queueSize: Int { lock.lock(); defer { lock.unlock() }; return pending }

    init(onFrame: @escaping (CGImage) -> Void, onError: @escaping () -> Void) { self.onFrame = onFrame; self.onError = onError }
    deinit { close() }

    /// configureDecoder with an avcC description (iOS) or an SPS / PPS pair (Android).
    func configure(avcC: Data) -> Bool {
        guard let sets = R7H264.parameterSets(avcC: avcC) else { return false }
        return configure(sps: sets.sps[0], pps: sets.pps[0], nalLength: sets.nalLength)
    }

    func configure(sps: Data, pps: Data, nalLength: Int = 4) -> Bool {
        close()
        var description: CMVideoFormatDescription?
        let status: OSStatus = sps.withUnsafeBytes { spsBytes in
            pps.withUnsafeBytes { ppsBytes in
                let pointers = [spsBytes.bindMemory(to: UInt8.self).baseAddress!, ppsBytes.bindMemory(to: UInt8.self).baseAddress!]
                let sizes = [sps.count, pps.count]
                return CMVideoFormatDescriptionCreateFromH264ParameterSets(allocator: kCFAllocatorDefault, parameterSetCount: 2, parameterSetPointers: pointers, parameterSetSizes: sizes, nalUnitHeaderLength: Int32(nalLength), formatDescriptionOut: &description)
            }
        }
        guard status == noErr, let description else { return false }
        let attributes: [CFString: Any] = [kCVPixelBufferPixelFormatTypeKey: kCVPixelFormatType_32BGRA, kCVPixelBufferIOSurfacePropertiesKey: [:] as CFDictionary]
        var created: VTDecompressionSession?
        let made = VTDecompressionSessionCreate(allocator: kCFAllocatorDefault, formatDescription: description, decoderSpecification: nil, imageBufferAttributes: attributes as CFDictionary, outputCallback: nil, decompressionSessionOut: &created)
        guard made == noErr, let created else { return false }
        VTSessionSetProperty(created, key: kVTDecompressionPropertyKey_RealTime, value: kCFBooleanTrue)
        session = created; format = description; self.nalLength = nalLength; failed = false
        return true
    }

    /// One length-prefixed sample (`nalLength`-byte prefixes).
    func decode(sample: Data, pts: Int64) {
        guard let session, let format, !sample.isEmpty else { return }
        lock.lock(); pending += 1; lock.unlock()
        queue.async { [weak self] in
            defer { if let self { self.lock.lock(); self.pending -= 1; self.lock.unlock() } }
            var block: CMBlockBuffer?
            let size = sample.count
            guard CMBlockBufferCreateWithMemoryBlock(allocator: kCFAllocatorDefault, memoryBlock: nil, blockLength: size, blockAllocator: kCFAllocatorDefault, customBlockSource: nil, offsetToData: 0, dataLength: size, flags: kCMBlockBufferAssureMemoryNowFlag, blockBufferOut: &block) == noErr, let block else { return self?.report() ?? () }
            let copied = sample.withUnsafeBytes { CMBlockBufferReplaceDataBytes(with: $0.baseAddress!, blockBuffer: block, offsetIntoDestination: 0, dataLength: size) }
            guard copied == noErr else { return self?.report() ?? () }
            var timing = CMSampleTimingInfo(duration: CMTime(value: 16_667, timescale: 1_000_000), presentationTimeStamp: CMTime(value: pts, timescale: 1_000_000), decodeTimeStamp: .invalid)
            var sizes = [size]
            var buffer: CMSampleBuffer?
            guard CMSampleBufferCreateReady(allocator: kCFAllocatorDefault, dataBuffer: block, formatDescription: format, sampleCount: 1, sampleTimingEntryCount: 1, sampleTimingArray: &timing, sampleSizeEntryCount: 1, sampleSizeArray: &sizes, sampleBufferOut: &buffer) == noErr, let buffer else { return self?.report() ?? () }
            var info = VTDecodeInfoFlags()
            let status = VTDecompressionSessionDecodeFrame(session, sampleBuffer: buffer, flags: [], infoFlagsOut: &info) { status, _, image, _, _ in
                guard status == noErr, let image else { return }
                var cg: CGImage?
                VTCreateCGImageFromCVPixelBuffer(image, options: nil, imageOut: &cg)
                if let cg { DispatchQueue.main.async { self?.onFrame(cg) } }
            }
            if status != noErr { self?.report() }
        }
    }

    private func report() {
        DispatchQueue.main.async { [weak self] in
            guard let self, !self.failed, self.session != nil else { return }
            self.failed = true
            self.onError()
        }
    }

    func close() {
        if let session { queue.sync {}; VTDecompressionSessionInvalidate(session) }
        session = nil; format = nil; failed = false
    }
}

/// Keyboard forwarding (DeviceStreamView onKeyDown / onKeyUp → stream.ts sendKey): iOS takes HID
/// usages from the physical key (`KeyboardEvent.code`), Android takes key codes, Back for Escape
/// and typed text from the produced character (`KeyboardEvent.key`).
enum R7DeviceKeys {
    /// macOS virtual key code → the web's `KeyboardEvent.code`.
    static let codes: [UInt16: String] = [
        0: "KeyA", 1: "KeyS", 2: "KeyD", 3: "KeyF", 4: "KeyH", 5: "KeyG", 6: "KeyZ", 7: "KeyX", 8: "KeyC", 9: "KeyV",
        11: "KeyB", 12: "KeyQ", 13: "KeyW", 14: "KeyE", 15: "KeyR", 16: "KeyY", 17: "KeyT", 18: "Digit1", 19: "Digit2",
        20: "Digit3", 21: "Digit4", 22: "Digit6", 23: "Digit5", 24: "Equal", 25: "Digit9", 26: "Digit7", 27: "Minus",
        28: "Digit8", 29: "Digit0", 30: "BracketRight", 31: "KeyO", 32: "KeyU", 33: "BracketLeft", 34: "KeyI", 35: "KeyP",
        36: "Enter", 37: "KeyL", 38: "KeyJ", 39: "Quote", 40: "KeyK", 41: "Semicolon", 42: "Backslash", 43: "Comma",
        44: "Slash", 45: "KeyN", 46: "KeyM", 47: "Period", 48: "Tab", 49: "Space", 50: "Backquote", 51: "Backspace",
        53: "Escape", 54: "MetaRight", 55: "MetaLeft", 56: "ShiftLeft", 57: "CapsLock", 58: "AltLeft", 59: "ControlLeft",
        60: "ShiftRight", 61: "AltRight", 62: "ControlRight", 76: "NumpadEnter", 115: "Home", 116: "PageUp", 117: "Delete",
        119: "End", 121: "PageDown", 123: "ArrowLeft", 124: "ArrowRight", 125: "ArrowDown", 126: "ArrowUp",
    ]
    /// HID_USAGE_BY_CODE and hidUsageForCode.
    static let named: [String: Int] = [
        "Enter": 0x28, "Escape": 0x29, "Backspace": 0x2a, "Tab": 0x2b, "Space": 0x2c, "Minus": 0x2d, "Equal": 0x2e,
        "BracketLeft": 0x2f, "BracketRight": 0x30, "Backslash": 0x31, "Semicolon": 0x33, "Quote": 0x34, "Backquote": 0x35,
        "Comma": 0x36, "Period": 0x37, "Slash": 0x38, "Delete": 0x4c, "ArrowRight": 0x4f, "ArrowLeft": 0x50,
        "ArrowDown": 0x51, "ArrowUp": 0x52, "ControlLeft": 0xe0, "ShiftLeft": 0xe1, "AltLeft": 0xe2, "MetaLeft": 0xe3,
        "ControlRight": 0xe4, "ShiftRight": 0xe5, "AltRight": 0xe6, "MetaRight": 0xe7,
    ]
    static func usage(code: String) -> Int? {
        let scalars = Array(code.unicodeScalars)
        if code.hasPrefix("Key"), scalars.count == 4, let letter = scalars.last, ("A"..."Z").contains(letter) { return 0x04 + Int(letter.value - 65) }
        if code == "Digit0" { return 0x27 }
        if code.hasPrefix("Digit"), scalars.count == 6, let digit = scalars.last, ("1"..."9").contains(digit) { return 0x1e + Int(digit.value - 49) }
        return named[code]
    }
    /// ANDROID_KEYCODE_BY_KEY, keyed by `KeyboardEvent.key`.
    static let android: [String: Int] = [
        "ArrowUp": 19, "ArrowDown": 20, "ArrowLeft": 21, "ArrowRight": 22, "Tab": 61, "Enter": 66, "Backspace": 67,
        "Delete": 112, "Home": 122, "End": 123, "PageUp": 92, "PageDown": 93,
    ]
    /// `KeyboardEvent.key` for a key AppKit reports: named keys by code, otherwise the character typed.
    static func key(keyCode: UInt16, characters: String?) -> String {
        switch codes[keyCode] {
        case "Enter", "NumpadEnter": return "Enter"
        case let named? where ["Escape", "Tab", "Backspace", "Delete", "Home", "End", "PageUp", "PageDown", "ArrowUp", "ArrowDown", "ArrowLeft", "ArrowRight"].contains(named): return named
        case let modifier? where modifier.hasPrefix("Meta"): return "Meta"
        case let modifier? where modifier.hasPrefix("Shift"): return "Shift"
        case let modifier? where modifier.hasPrefix("Alt"): return "Alt"
        case let modifier? where modifier.hasPrefix("Control"): return "Control"
        default: return characters ?? ""
        }
    }

    /// The packet sendKey writes, or nil when the key is not forwarded.
    static func packet(platform: String, phase: String, keyCode: UInt16, characters: String?, meta: Bool, control: Bool) -> Data? {
        if platform == "ios" {
            guard let code = codes[keyCode], let usage = usage(code: code) else { return nil }
            return R6DeviceWire.tagged(0x06, ["type": phase, "usage": usage])
        }
        guard phase == "down" else { return nil }
        let key = key(keyCode: keyCode, characters: characters)
        if key == "Escape" { return json(["type": "back"]) }
        if let keycode = android[key] { return json(["type": "key", "keycode": keycode]) }
        if key.count == 1, !meta, !control { return json(["type": "text", "text": key]) }
        return nil
    }

    static func json(_ object: [String: Any]) -> Data { (try? JSONSerialization.data(withJSONObject: object, options: [.sortedKeys])) ?? Data("{}".utf8) }

    /// flagsChanged: which modifier key moved, and whether it went down.
    static func modifierPhase(keyCode: UInt16, flags: NSEvent.ModifierFlags) -> String? {
        switch codes[keyCode] {
        case "ShiftLeft", "ShiftRight": return flags.contains(.shift) ? "down" : "up"
        case "ControlLeft", "ControlRight": return flags.contains(.control) ? "down" : "up"
        case "AltLeft", "AltRight": return flags.contains(.option) ? "down" : "up"
        case "MetaLeft", "MetaRight": return flags.contains(.command) ? "down" : "up"
        default: return nil
        }
    }
}
#endif
