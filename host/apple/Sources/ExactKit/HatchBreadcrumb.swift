// The crash breadcrumb (@ref LLP 1075.003.000.001 §4.4). A Swift trap, an
// Objective-C exception or a Rust panic in hatch code ends the process and
// cannot be caught, so the host leaves a record of the hatch calls that
// were open, which the next launch reads: "the last run ended while inside
// hatch element avatar (built, call 4127)". It says "ended while inside", not
// "crashed in": another thread may have been the cause. On in production too.
//
// One file a process, `hatch-breadcrumb-<pid>`, 4,096 bytes, mapped shared:
// a store reaches the page cache with no syscall, and the kernel keeps the
// page if the process dies. The process holds a lock on its file for as long
// as it lives; a later launch reads and deletes each file it can lock.
//
//   8 slots of 512 bytes, one a session:
//      0  u32 used        4  u32 depth      8  u32 overflow    12  u32 calls
//     16  16 bytes: the session's label, UTF-8
//     32  8 records of 60 bytes, a stack:
//           0  u32 checksum (of the 56 bytes after it)
//           4  u32 incarnation    8  u32 call    12  u8 moment   13  u8 length
//          14  46 bytes: the hatch, as the journal names it, UTF-8
//
// A push writes its record, then publishes the new depth; a pop lowers the
// depth. A reader trusts a record only below the published depth and with a
// valid checksum, so a death inside a push leaves the last whole record. A
// call nested past 8 only counts (`overflow`).
import Foundation

final class HatchBreadcrumb {
    static let slots = 8, slotBytes = 512, depthMax = 8, headerBytes = 32, recordBytes = 60, nameBytes = 46, labelBytes = 16
    static let moments = ["built", "changed", "ended", "called"]
    static let prefix = "hatch-breadcrumb-"

    let path: String
    private let fd: Int32
    private let base: UnsafeMutableRawPointer
    private var taken = [Bool](repeating: false, count: HatchBreadcrumb.slots)

    /// Where this app's breadcrumbs live: its Application Support directory,
    /// which the OS does not purge between a crash and the next launch;
    /// under the agent, a scratch directory its next launch finds again.
    static var directory: String {
        let id = Bundle.main.bundleIdentifier ?? "app"
        return ExactEnv.agentMode ? (NSTemporaryDirectory() as NSString).appendingPathComponent("exact-agent-breadcrumbs-\(id)")
            : NSHomeDirectory() + "/Library/Application Support/exact/" + id
    }

    /// What earlier runs of this app left: one line for each session that
    /// ended inside a hatch call. Read once, when `shared` is first asked for.
    private(set) static var lastEnds: [String] = []
    /// Whether a session's journal has said them yet: the first to connect does.
    static var reported = false
    /// This process's file, made when the first session's hatches connect
    /// (after first pixel); nil when it cannot be made. Earlier runs' files
    /// are read and deleted first.
    static let shared: HatchBreadcrumb? = {
        let directory = HatchBreadcrumb.directory
        lastEnds = read(directory: directory)
        guard let crumbs = HatchBreadcrumb(directory: directory) else { return nil }
        // A clean exit deletes its own file; a death leaves it to be read.
        ownPath = strdup(crumbs.path)
        atexit { if let ownPath { unlink(ownPath) } }
        return crumbs
    }()
    private nonisolated(unsafe) static var ownPath: UnsafeMutablePointer<CChar>?

    init?(directory: String, pid: Int32 = getpid()) {
        try? FileManager.default.createDirectory(atPath: directory, withIntermediateDirectories: true)
        path = (directory as NSString).appendingPathComponent("\(Self.prefix)\(pid)")
        let size = Self.slots * Self.slotBytes
        fd = open(path, O_CREAT | O_RDWR | O_TRUNC, 0o600)
        guard fd >= 0, ftruncate(fd, off_t(size)) == 0, flock(fd, LOCK_EX | LOCK_NB) == 0,
              let map = mmap(nil, size, PROT_READ | PROT_WRITE, MAP_SHARED, fd, 0), map != MAP_FAILED else {
            if fd >= 0 { Darwin.close(fd) }
            return nil
        }
        base = map
    }

    /// The process is done with its file: unmap, unlock, and delete it or
    /// leave it as a death would (a test's stand-in for one).
    func close(delete: Bool) {
        munmap(base, Self.slots * Self.slotBytes)
        Darwin.close(fd)
        if delete { unlink(path) }
    }

    private func slot(_ index: Int) -> UnsafeMutableRawPointer { base + index * Self.slotBytes }

    /// A free slot for a session, cleared, or nil when all 8 are taken: that
    /// session runs without a breadcrumb.
    func take(label: String) -> Int? {
        guard let index = taken.firstIndex(of: false) else { return nil }
        taken[index] = true
        let s = slot(index)
        s.initializeMemory(as: UInt8.self, repeating: 0, count: Self.slotBytes)
        let bytes = Array(label.utf8.prefix(Self.labelBytes))
        bytes.withUnsafeBytes { (s + 16).copyMemory(from: $0.baseAddress!, byteCount: bytes.count) }
        s.storeBytes(of: 1, as: UInt32.self)
        return index
    }

    /// A session's end clears and frees its own slot, and no other's.
    func free(_ index: Int) {
        guard taken.indices.contains(index), taken[index] else { return }
        slot(index).initializeMemory(as: UInt8.self, repeating: 0, count: Self.slotBytes)
        taken[index] = false
    }

    /// The depth is stored after the record it admits is whole: a call the
    /// optimizer cannot see through keeps the two stores in that order.
    @inline(never)
    private func publish(_ s: UnsafeMutableRawPointer, depth: UInt32) { s.storeBytes(of: depth, toByteOffset: 4, as: UInt32.self) }

    private static func checksum(_ record: UnsafeRawPointer) -> UInt32 {
        var h: UInt32 = 2_166_136_261
        for i in 4..<recordBytes { h = (h ^ UInt32(record.load(fromByteOffset: i, as: UInt8.self))) &* 16_777_619 }
        return h
    }

    /// A hatch call begins.
    func push(_ index: Int, name: String, moment: String, incarnation: UInt32) {
        let s = slot(index)
        let call = s.load(fromByteOffset: 12, as: UInt32.self) &+ 1
        s.storeBytes(of: call, toByteOffset: 12, as: UInt32.self)
        let depth = s.load(fromByteOffset: 4, as: UInt32.self)
        guard depth < UInt32(Self.depthMax) else {
            s.storeBytes(of: s.load(fromByteOffset: 8, as: UInt32.self) &+ 1, toByteOffset: 8, as: UInt32.self)
            return
        }
        let r = s + Self.headerBytes + Int(depth) * Self.recordBytes
        r.storeBytes(of: incarnation, toByteOffset: 4, as: UInt32.self)
        r.storeBytes(of: call, toByteOffset: 8, as: UInt32.self)
        r.storeBytes(of: UInt8(Self.moments.firstIndex(of: moment) ?? 3), toByteOffset: 12, as: UInt8.self)
        var bytes = Array(name.utf8.prefix(Self.nameBytes))
        while String(bytes: bytes, encoding: .utf8) == nil, !bytes.isEmpty { bytes.removeLast() }   // cut at a character
        r.storeBytes(of: UInt8(bytes.count), toByteOffset: 13, as: UInt8.self)
        (r + 14).initializeMemory(as: UInt8.self, repeating: 0, count: Self.nameBytes)
        bytes.withUnsafeBytes { if let from = $0.baseAddress { (r + 14).copyMemory(from: from, byteCount: bytes.count) } }
        r.storeBytes(of: Self.checksum(r), as: UInt32.self)
        publish(s, depth: depth + 1)
    }

    /// The call returned.
    func pop(_ index: Int) {
        let s = slot(index)
        let overflow = s.load(fromByteOffset: 8, as: UInt32.self)
        if overflow > 0 { s.storeBytes(of: overflow - 1, toByteOffset: 8, as: UInt32.self); return }
        let depth = s.load(fromByteOffset: 4, as: UInt32.self)
        if depth > 0 { publish(s, depth: depth - 1) }
    }

    /// Every breadcrumb in `directory` whose process is gone (its file can be
    /// locked), read and deleted: one line for each session that ended with a
    /// hatch call open, naming the innermost one whose record is whole.
    static func read(directory: String) -> [String] {
        let files = ((try? FileManager.default.contentsOfDirectory(atPath: directory)) ?? []).filter { $0.hasPrefix(prefix) }.sorted()
        var lines: [String] = []
        for file in files {
            let path = (directory as NSString).appendingPathComponent(file)
            let fd = open(path, O_RDWR)
            guard fd >= 0 else { continue }
            defer { Darwin.close(fd) }
            // Its process still holds it: that run has not ended.
            guard flock(fd, LOCK_EX | LOCK_NB) == 0 else { continue }
            var bytes = [UInt8](repeating: 0, count: slots * slotBytes)
            let got = bytes.withUnsafeMutableBytes { pread(fd, $0.baseAddress, $0.count, 0) }
            unlink(path)
            guard got == bytes.count else { continue }
            bytes.withUnsafeBytes { all in
                for index in 0..<slots {
                    let s = all.baseAddress! + index * slotBytes
                    let depth = Int(s.loadUnaligned(fromByteOffset: 4, as: UInt32.self))
                    guard s.loadUnaligned(as: UInt32.self) == 1, depth >= 1, depth <= depthMax else { continue }
                    // The innermost record that is whole.
                    guard let r = (0..<depth).reversed().map({ s + headerBytes + $0 * recordBytes })
                        .first(where: { $0.loadUnaligned(as: UInt32.self) == checksum($0) && Int($0.load(fromByteOffset: 13, as: UInt8.self)) <= nameBytes }) else { continue }
                    let name = String(decoding: UnsafeRawBufferPointer(start: r + 14, count: Int(r.load(fromByteOffset: 13, as: UInt8.self))), as: UTF8.self)
                    let moment = moments[min(3, Int(r.load(fromByteOffset: 12, as: UInt8.self)))]
                    let overflow = s.loadUnaligned(fromByteOffset: 8, as: UInt32.self)
                    let label = String(decoding: UnsafeRawBufferPointer(start: s + 16, count: labelBytes).prefix { $0 != 0 }, as: UTF8.self)
                    lines.append("the last run ended while inside hatch \(name) (\(moment), call \(r.loadUnaligned(fromByteOffset: 8, as: UInt32.self)))"
                        + (overflow > 0 ? " (+\(overflow) nested)" : "") + (label.isEmpty ? "" : ", session \(label)")
                        + "; if it repeats, launch with EXACT_HATCHES=off")
                }
            }
        }
        return lines
    }
}
