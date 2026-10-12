// The embedded server's failure log (20261005-embedded-server-runtime), after T3 Code's desktop
// app (MIT, see LICENSE-T3; reference 1e2ecbd975: apps/desktop/src/app/DesktopObservability.ts
// `appendBoundedOutputChunk`, `makeRotatingLogFileWriter`, `makeBackendOutputLogShape`).
//
// Each server run's stdout and stderr are kept in memory (the last MiB, at most 256 chunks) and
// written to `<T3 home>/userdata/logs/server-child.log` (rotating 10 MiB × 10, JSON lines) only
// when the run fails: an unexpected exit, or a readiness round that timed out. A clean stop
// discards them.
import Foundation

enum T3LocalOutputStream: String { case stdout, stderr }

/// The per-run buffer (`BackendOutputSession`): chunks with a start offset into the first.
struct T3LocalOutputSession {
    static let maxBytes = 1024 * 1024
    static let maxChunks = 256
    struct Chunk { let stream: T3LocalOutputStream; let bytes: Data; var offset: Int }
    let runId: String
    let startDetails: String
    var chunks: [Chunk] = []
    var byteLength = 0

    /// `appendBoundedOutputChunk`: keep the last MiB and the last 256 chunks.
    func appending(_ stream: T3LocalOutputStream, _ chunk: Data) -> T3LocalOutputSession {
        if chunk.isEmpty { return self }
        let retained = chunk.count > Self.maxBytes ? Data(chunk.suffix(Self.maxBytes)) : Data(chunk)
        var chunks = self.chunks + [Chunk(stream: stream, bytes: retained, offset: 0)]
        var byteLength = self.byteLength + retained.count
        var overflow = max(0, byteLength - Self.maxBytes)
        var first = 0
        while overflow > 0, first < chunks.count {
            let kept = chunks[first].bytes.count - chunks[first].offset
            if kept <= overflow {
                overflow -= kept; byteLength -= kept; first += 1
                continue
            }
            chunks[first].offset += overflow
            byteLength -= overflow
            overflow = 0
        }
        let excess = max(0, chunks.count - first - Self.maxChunks)
        for index in first..<(first + excess) { byteLength -= chunks[index].bytes.count - chunks[index].offset }
        first += excess
        var next = self
        next.chunks = Array(chunks[first...])
        next.byteLength = byteLength
        return next
    }
}

/// `makeRotatingLogFileWriter`: append, and rotate `<file>.1 … .<maxFiles>` past `maxBytes`.
final class T3LocalRotatingFile {
    let path: URL
    let maxBytes: Int
    let maxFiles: Int
    private var size: Int

    init(path: URL, maxBytes: Int = 10 * 1024 * 1024, maxFiles: Int = 10) {
        self.path = path; self.maxBytes = maxBytes; self.maxFiles = maxFiles
        try? FileManager.default.createDirectory(at: path.deletingLastPathComponent(), withIntermediateDirectories: true)
        size = (try? FileManager.default.attributesOfItem(atPath: path.path)[.size] as? Int) ?? 0
        pruneOverflowBackups()
    }

    private func suffixed(_ index: Int) -> URL { URL(fileURLWithPath: "\(path.path).\(index)") }

    private func pruneOverflowBackups() {
        let directory = path.deletingLastPathComponent(), base = path.lastPathComponent
        for name in (try? FileManager.default.contentsOfDirectory(atPath: directory.path)) ?? [] where name.hasPrefix(base + ".") {
            if let index = Int(name.dropFirst(base.count + 1)), index > maxFiles {
                try? FileManager.default.removeItem(at: directory.appendingPathComponent(name))
            }
        }
    }

    private func rotate() {
        let files = FileManager.default
        try? files.removeItem(at: suffixed(maxFiles))
        if maxFiles > 1 {
            for index in stride(from: maxFiles - 1, through: 1, by: -1) where files.fileExists(atPath: suffixed(index).path) {
                try? files.moveItem(at: suffixed(index), to: suffixed(index + 1))
            }
        }
        if files.fileExists(atPath: path.path) { try? files.moveItem(at: path, to: suffixed(1)) }
        size = 0
    }

    func write(_ bytes: Data) {
        if bytes.isEmpty { return }
        if size > 0 && size + bytes.count > maxBytes { rotate() }
        if !FileManager.default.fileExists(atPath: path.path) { FileManager.default.createFile(atPath: path.path, contents: nil) }
        guard let handle = try? FileHandle(forWritingTo: path) else { return }
        defer { try? handle.close() }
        _ = try? handle.seekToEnd()
        try? handle.write(contentsOf: bytes)
        size += bytes.count
        if size > maxBytes { rotate() }
    }
}

/// `DesktopBackendOutputLogShape` for the primary instance (`server-child.log`).
protocol T3LocalOutputLog: AnyObject {
    func beginSession(details: String)
    func writeOutputChunk(_ stream: T3LocalOutputStream, _ chunk: Data)
    func persistFailureSnapshot(details: String)
    func persistFailure(details: String)
    func discardSession()
}

final class T3LocalFileOutputLog: T3LocalOutputLog {
    static let fiberId = "#backend-child"
    let file: T3LocalRotatingFile
    let instanceId: String
    let runId: String
    private(set) var session: T3LocalOutputSession?
    private let lock = NSLock()
    var now: () -> Date = Date.init

    /// `runId` is the app run's id (DesktopApp.ts `makeDesktopRunId`: 12 hex characters).
    init(logDirectory: URL, instanceId: String = "primary", runId: String) {
        let name = instanceId == "primary" ? "server-child.log" : "server-child-\(instanceId.replacingOccurrences(of: "[^a-zA-Z0-9._-]+", with: "_", options: .regularExpression)).log"
        file = T3LocalRotatingFile(path: logDirectory.appendingPathComponent(name))
        self.instanceId = instanceId; self.runId = runId
    }

    static func sanitize(_ value: String) -> String {
        value.replacingOccurrences(of: "\\s+", with: " ", options: .regularExpression).trimmingCharacters(in: .whitespaces)
    }

    func beginSession(details: String) {
        lock.lock(); defer { lock.unlock() }
        session = T3LocalOutputSession(runId: runId, startDetails: Self.sanitize(details))
    }
    func writeOutputChunk(_ stream: T3LocalOutputStream, _ chunk: Data) {
        lock.lock(); defer { lock.unlock() }
        session = session?.appending(stream, chunk)
    }
    func persistFailureSnapshot(details: String) {
        lock.lock(); let current = session; lock.unlock()
        if let current { writeFailure(current, details: details) }
    }
    func persistFailure(details: String) {
        lock.lock(); let current = session; session = nil; lock.unlock()
        if let current { writeFailure(current, details: details) }
    }
    func discardSession() {
        lock.lock(); session = nil; lock.unlock()
    }

    private static let timestamp: ISO8601DateFormatter = {
        let formatter = ISO8601DateFormatter()
        formatter.formatOptions = [.withInternetDateTime, .withFractionalSeconds]
        return formatter
    }()

    private func record(_ message: String, level: String, _ annotations: [String: Any]) {
        let line: [String: Any] = ["message": message, "level": level, "timestamp": Self.timestamp.string(from: now()),
                                   "annotations": annotations, "spans": [String: Any](), "fiberId": Self.fiberId]
        guard var bytes = try? JSONSerialization.data(withJSONObject: line, options: [.sortedKeys, .withoutEscapingSlashes]) else { return }
        bytes.append(0x0A)
        file.write(bytes)
    }

    private func writeFailure(_ session: T3LocalOutputSession, details: String) {
        let base: [String: Any] = ["component": "desktop-backend-child", "runId": session.runId, "instanceId": instanceId]
        record("backend child process failure output start", level: "ERROR", base.merging(["phase": "START", "details": session.startDetails]) { $1 })
        for chunk in session.chunks {
            let text = String(decoding: chunk.bytes.suffix(from: chunk.bytes.startIndex + chunk.offset), as: UTF8.self)
            record("backend child process output", level: chunk.stream == .stderr ? "ERROR" : "INFO", base.merging(["stream": chunk.stream.rawValue, "text": text]) { $1 })
        }
        record("backend child process failure output end", level: "ERROR", base.merging(["phase": "END", "details": Self.sanitize(details)]) { $1 })
    }
}
