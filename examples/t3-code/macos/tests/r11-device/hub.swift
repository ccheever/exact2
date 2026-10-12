import AppKit
import CryptoKit
import Network
import VideoToolbox

/// Lane r11-device (a copy of r10-device's test hub): a loopback stand-in for the server's `/api/device-hub` proxy in front of
/// serve-sim and serve-emu. One TCP listener answers plain HTTP (the AVCC and MJPEG bodies, the
/// accessibility tree, server-sent events) and upgrades `…/ws` requests to WebSockets by hand, so
/// a client sees one origin as it does through the proxy. It speaks the wire formats
/// packages/client-runtime/src/device/stream.ts and apps/web/src/components/device/deviceHubApi.ts
/// read; no device or device tools are involved.
final class Hub {
    let listener: NWListener
    private(set) var port: UInt16 = 0
    let queue = DispatchQueue(label: "r11-device-hub")
    private var lines: [String] = []
    private var messages: [(path: String, data: Data, text: Bool)] = []
    private var sockets: [(path: String, connection: NWConnection)] = []
    /// path prefix (after /api/device-hub) → handler writing the response.
    var http: [(String, (String, NWConnection) -> Void)] = []
    var onSocket: ((String, NWConnection) -> Void)?
    /// Answer upgrades with 401, as the proxy does for a spent ticket.
    var refuseSockets = false
    /// Live feeds' pacing timers (lane r9-device), cancelled with the hub.
    var timers: [DispatchSourceTimer] = []

    init() throws {
        listener = try NWListener(using: .tcp, on: .any)
        let ready = DispatchSemaphore(value: 0)
        listener.stateUpdateHandler = { if case .ready = $0 { ready.signal() } }
        listener.newConnectionHandler = { [weak self] connection in
            guard let self else { return }
            connection.start(queue: self.queue)
            self.readHead(connection, Data())
        }
        listener.start(queue: queue)
        _ = ready.wait(timeout: .now() + 5)
        port = listener.port?.rawValue ?? 0
    }
    deinit { timers.forEach { $0.cancel() }; listener.cancel(); sockets.forEach { $0.connection.cancel() } }

    var origin: URL { URL(string: "http://127.0.0.1:\(port)")! }
    var requests: [String] { queue.sync { lines } }
    func received(_ path: String) -> [Data] { queue.sync { messages.filter { $0.path.contains(path) }.map(\.data) } }
    func texts(_ path: String) -> [String] { queue.sync { messages.filter { $0.path.contains(path) && $0.text }.compactMap { String(data: $0.data, encoding: .utf8) } } }

    private func readHead(_ connection: NWConnection, _ buffer: Data) {
        connection.receive(minimumIncompleteLength: 1, maximumLength: 65_536) { [weak self] data, _, done, error in
            guard let self, error == nil else { return }
            var buffer = buffer
            if let data { buffer.append(data) }
            guard let end = buffer.range(of: Data("\r\n\r\n".utf8)) else { if !done { self.readHead(connection, buffer) }; return }
            let head = String(decoding: buffer[..<end.lowerBound], as: UTF8.self)
            let first = String(head.split(separator: "\r\n").first ?? "")
            self.lines.append(first)
            let path = first.split(separator: " ").dropFirst().first.map(String.init) ?? ""
            if head.lowercased().contains("upgrade: websocket"), let key = head.split(separator: "\r\n").first(where: { $0.lowercased().hasPrefix("sec-websocket-key:") })?.split(separator: ":", maxSplits: 1).last?.trimmingCharacters(in: .whitespaces) {
                if self.refuseSockets { return self.respond(connection, status: "401 Unauthorized", body: Data()) }
                let accept = Data(Insecure.SHA1.hash(data: Data((key + "258EAFA5-E914-47DA-95CA-C5AB0DC85B11").utf8))).base64EncodedString()
                connection.send(content: Data("HTTP/1.1 101 Switching Protocols\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Accept: \(accept)\r\n\r\n".utf8), completion: .idempotent)
                self.sockets.append((path, connection))
                self.readFrames(connection, path, Data(buffer[end.upperBound...]))
                self.onSocket?(path, connection)
                return
            }
            if let handler = self.http.first(where: { $0.0.hasPrefix("=") ? path == String($0.0.dropFirst()) : path.contains($0.0) })?.1 { handler(path, connection) }
            else { self.respond(connection, status: "404 Not Found", body: Data()) }
        }
    }

    func respond(_ connection: NWConnection, status: String = "200 OK", type: String = "application/json", body: Data) {
        connection.send(content: Data("HTTP/1.1 \(status)\r\nContent-Type: \(type)\r\nContent-Length: \(body.count)\r\nConnection: close\r\n\r\n".utf8) + body, completion: .contentProcessed { _ in connection.cancel() })
    }
    /// An open-ended body (a live stream): headers, then whatever `write` sends later.
    func open(_ connection: NWConnection, type: String) {
        connection.send(content: Data("HTTP/1.1 200 OK\r\nContent-Type: \(type)\r\nCache-Control: no-store\r\nConnection: close\r\n\r\n".utf8), completion: .idempotent)
    }
    func write(_ connection: NWConnection, _ data: Data) { connection.send(content: data, completion: .idempotent) }

    // MARK: WebSocket frames (RFC 6455; the server's frames are unmasked)

    private func readFrames(_ connection: NWConnection, _ path: String, _ pending: Data) {
        var buffer = pending
        while let (opcode, payload, used) = Hub.frame(buffer) {
            buffer.removeFirst(used)
            if opcode == 8 { connection.cancel(); return }
            if opcode == 1 || opcode == 2 { messages.append((path, payload, opcode == 1)) }
        }
        connection.receive(minimumIncompleteLength: 1, maximumLength: 1 << 20) { [weak self] data, _, done, error in
            guard let self, error == nil else { return }
            if let data { buffer.append(data) }
            if !done || !buffer.isEmpty { self.readFrames(connection, path, buffer) }
        }
    }

    static func frame(_ data: Data) -> (UInt8, Data, Int)? {
        let bytes = [UInt8](data)
        guard bytes.count >= 2 else { return nil }
        let opcode = bytes[0] & 0x0f, masked = bytes[1] & 0x80 != 0
        var length = Int(bytes[1] & 0x7f), offset = 2
        if length == 126 { guard bytes.count >= 4 else { return nil }; length = Int(bytes[2]) << 8 | Int(bytes[3]); offset = 4 }
        else if length == 127 { guard bytes.count >= 10 else { return nil }; length = (2..<10).reduce(0) { $0 << 8 | Int(bytes[$1]) }; offset = 10 }
        let mask = masked ? Array(bytes[offset..<min(bytes.count, offset + 4)]) : []
        if masked { offset += 4 }
        guard bytes.count >= offset + length else { return nil }
        var payload = Array(bytes[offset..<offset + length])
        if masked { for i in payload.indices { payload[i] ^= mask[i % 4] } }
        return (opcode, Data(payload), offset + length)
    }

    func push(_ path: String, _ data: Data, text: Bool = false) {
        queue.async {
            var head: [UInt8] = [0x80 | (text ? 1 : 2)]
            if data.count < 126 { head.append(UInt8(data.count)) }
            else if data.count < 65_536 { head += [126, UInt8(data.count >> 8), UInt8(data.count & 0xff)] }
            else { head.append(127); head += (0..<8).reversed().map { UInt8((data.count >> ($0 * 8)) & 0xff) } }
            for socket in self.sockets where socket.path.contains(path) { socket.connection.send(content: Data(head) + data, completion: .idempotent) }
        }
    }
    func close(_ path: String, code: UInt16) {
        queue.async {
            for socket in self.sockets where socket.path.contains(path) {
                socket.connection.send(content: Data([0x88, 2, UInt8(code >> 8), UInt8(code & 0xff)]), completion: .contentProcessed { _ in socket.connection.cancel() })
            }
            self.sockets.removeAll { $0.path.contains(path) }
        }
    }
    var socketCount: Int { queue.sync { sockets.count } }
}

/// Synthetic H.264 from VideoToolbox: a solid colour with a white bar, as SPS / PPS and
/// length-prefixed samples (serve-sim's AVCC) or Annex B (serve-emu).
struct Encoded {
    var sps = Data(), pps = Data()
    var samples: [(data: Data, key: Bool)] = []
    var avcC: Data { R7H264.avcC(sps: sps, pps: pps) }
    func annexB(_ index: Int, parameterSets: Bool) -> Data {
        var out = Data()
        if parameterSets { out += Data([0, 0, 0, 1]) + sps + Data([0, 0, 0, 1]) + pps }
        var bytes = [UInt8](samples[index].data), offset = 0
        while offset + 4 <= bytes.count {
            let length = Int(R7H264.be32(bytes, offset))
            bytes[offset] = 0; bytes[offset + 1] = 0; bytes[offset + 2] = 0; bytes[offset + 3] = 1
            out += Data(bytes[offset..<offset + 4 + length])
            offset += 4 + length
        }
        return out
    }

    static func make(width: Int, height: Int, colors: [NSColor]) -> Encoded {
        var result = Encoded()
        var session: VTCompressionSession?
        VTCompressionSessionCreate(allocator: nil, width: Int32(width), height: Int32(height), codecType: kCMVideoCodecType_H264, encoderSpecification: nil, imageBufferAttributes: nil, compressedDataAllocator: nil, outputCallback: nil, refcon: nil, compressionSessionOut: &session)
        guard let session else { return result }
        VTSessionSetProperty(session, key: kVTCompressionPropertyKey_RealTime, value: kCFBooleanTrue)
        VTSessionSetProperty(session, key: kVTCompressionPropertyKey_ProfileLevel, value: kVTProfileLevel_H264_Baseline_AutoLevel)
        VTSessionSetProperty(session, key: kVTCompressionPropertyKey_AllowFrameReordering, value: kCFBooleanFalse)
        VTSessionSetProperty(session, key: kVTCompressionPropertyKey_MaxKeyFrameInterval, value: 30 as CFNumber)
        let lock = NSLock()
        for (index, color) in colors.enumerated() {
            var buffer: CVPixelBuffer?
            CVPixelBufferCreate(nil, width, height, kCVPixelFormatType_32BGRA, [kCVPixelBufferCGImageCompatibilityKey: true, kCVPixelBufferCGBitmapContextCompatibilityKey: true] as CFDictionary, &buffer)
            guard let buffer else { continue }
            CVPixelBufferLockBaseAddress(buffer, [])
            let context = CGContext(data: CVPixelBufferGetBaseAddress(buffer), width: width, height: height, bitsPerComponent: 8, bytesPerRow: CVPixelBufferGetBytesPerRow(buffer), space: CGColorSpace(name: CGColorSpace.sRGB)!, bitmapInfo: CGImageAlphaInfo.premultipliedFirst.rawValue | CGBitmapInfo.byteOrder32Little.rawValue)!
            context.setFillColor(color.cgColor); context.fill(CGRect(x: 0, y: 0, width: width, height: height))
            context.setFillColor(NSColor.white.cgColor); context.fill(CGRect(x: width / 4, y: height * 2 / 3, width: width / 2, height: height / 8))
            CVPixelBufferUnlockBaseAddress(buffer, [])
            let done = DispatchSemaphore(value: 0)
            let force = index == 0 ? [kVTEncodeFrameOptionKey_ForceKeyFrame: true] as CFDictionary : nil
            VTCompressionSessionEncodeFrame(session, imageBuffer: buffer, presentationTimeStamp: CMTime(value: CMTimeValue(index), timescale: 30), duration: CMTime(value: 1, timescale: 30), frameProperties: force, infoFlagsOut: nil) { status, _, sample in
                defer { done.signal() }
                guard status == noErr, let sample, let block = CMSampleBufferGetDataBuffer(sample) else { return }
                let attachments = CMSampleBufferGetSampleAttachmentsArray(sample, createIfNecessary: false) as? [[CFString: Any]]
                let key = !(attachments?.first?[kCMSampleAttachmentKey_NotSync] as? Bool ?? false)
                var length = 0, pointer: UnsafeMutablePointer<CChar>?
                CMBlockBufferGetDataPointer(block, atOffset: 0, lengthAtOffsetOut: nil, totalLengthOut: &length, dataPointerOut: &pointer)
                let data = Data(bytes: pointer!, count: length)
                lock.lock(); defer { lock.unlock() }
                if key, let format = CMSampleBufferGetFormatDescription(sample) {
                    for (index, target) in [0, 1].enumerated() {
                        var set: UnsafePointer<UInt8>?, size = 0
                        CMVideoFormatDescriptionGetH264ParameterSetAtIndex(format, parameterSetIndex: target, parameterSetPointerOut: &set, parameterSetSizeOut: &size, parameterSetCountOut: nil, nalUnitHeaderLengthOut: nil)
                        if let set { if index == 0 { result.sps = Data(bytes: set, count: size) } else { result.pps = Data(bytes: set, count: size) } }
                    }
                }
                result.samples.append((data, key))
            }
            VTCompressionSessionCompleteFrames(session, untilPresentationTimeStamp: .invalid)
            _ = done.wait(timeout: .now() + 5)
        }
        VTCompressionSessionInvalidate(session)
        return result
    }
}

/// The colour of a CGImage's pixel at a relative point (sRGB, 0…255).
func pixel(_ image: CGImage, x: Double, y: Double) -> (r: Int, g: Int, b: Int) {
    let rep = NSBitmapImageRep(cgImage: image)
    guard let color = rep.colorAt(x: Int(Double(image.width) * x), y: Int(Double(image.height) * y))?.usingColorSpace(.sRGB) else { return (0, 0, 0) }
    return (Int(color.redComponent * 255), Int(color.greenComponent * 255), Int(color.blueComponent * 255))
}

func keyEvent(_ type: NSEvent.EventType, keyCode: UInt16, characters: String, flags: NSEvent.ModifierFlags = [], window: NSWindow) -> NSEvent {
    NSEvent.keyEvent(with: type, location: .zero, modifierFlags: flags, timestamp: ProcessInfo.processInfo.systemUptime, windowNumber: window.windowNumber, context: nil, characters: characters, charactersIgnoringModifiers: characters, isARepeat: false, keyCode: keyCode)!
}

func mouseEvent(_ type: NSEvent.EventType, at point: NSPoint, in view: NSView, flags: NSEvent.ModifierFlags = []) -> NSEvent {
    let location = view.convert(point, to: nil)
    return NSEvent.mouseEvent(with: type, location: location, modifierFlags: flags, timestamp: ProcessInfo.processInfo.systemUptime, windowNumber: view.window!.windowNumber, context: nil, eventNumber: 0, clickCount: 1, pressure: 1)!
}
