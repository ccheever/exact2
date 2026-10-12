// Question attachments have a private byte namespace, independent of ordinary composer collectors.
// @ref llp/1109.005-composer-and-transcript.decision.md#scratch-tasks-and-queue-boundaries
import Foundation
import CryptoKit
import Darwin

protocol T3QuestionAttachmentDisk {
    func load() throws -> Data?
    func replace(_ data: Data) throws
    func write(_ id: String, bytes: Data) throws
    func read(_ id: String, limit: Int) throws -> Data
    func remove(_ id: String) throws
}

struct T3QuestionAttachmentFailure: Error, LocalizedError {
    let message: String
    var errorDescription: String? { message }
    init(_ message: String) { self.message = message }
}

/// The directory is opened without following links on every operation. No disk work in init.
struct T3QuestionAttachmentFiles: T3QuestionAttachmentDisk {
    let dataRoot: URL
    private func failed() -> T3QuestionAttachmentFailure { .init("Question attachment storage is unavailable. Keep the original files.") }
    private func isEmpty(_ fd: Int32) throws -> Bool {
        guard let stream = fdopendir(dup(fd)) else { throw failed() }
        defer { closedir(stream) }
        errno = 0
        while let entry = readdir(stream) {
            let name = withUnsafePointer(to: entry.pointee.d_name) {
                $0.withMemoryRebound(to: CChar.self, capacity: Int(MAXNAMLEN) + 1) { String(cString: $0) }
            }
            if name != "." && name != ".." { return false }
        }
        guard errno == 0 else { throw failed() }
        return true
    }
    private func directory(create: Bool) throws -> Int32 {
        let parent = Darwin.open(dataRoot.path, O_RDONLY | O_DIRECTORY | O_NOFOLLOW)
        guard parent >= 0 else { throw failed() }
        defer { Darwin.close(parent) }
        if create && mkdirat(parent, "question-attachments", 0o700) != 0 && errno != EEXIST { throw failed() }
        let fd = openat(parent, "question-attachments", O_RDONLY | O_DIRECTORY | O_NOFOLLOW)
        guard fd >= 0 else { throw failed() }
        return fd
    }
    private func valid(_ id: String) throws {
        guard UUID(uuidString: id)?.uuidString.lowercased() == id else { throw failed() }
    }
    private func writeAll(_ data: Data, to fd: Int32) throws {
        try data.withUnsafeBytes { bytes in
            var offset = 0
            while offset < bytes.count {
                let n = Darwin.write(fd, bytes.baseAddress!.advanced(by: offset), bytes.count - offset)
                if n < 0 && errno == EINTR { continue }
                guard n > 0 else { throw failed() }
                offset += n
            }
        }
        guard fsync(fd) == 0 else { throw failed() }
    }
    private func readFile(_ name: String, in directory: Int32, limit: Int) throws -> Data {
        let fd = openat(directory, name, O_RDONLY | O_NOFOLLOW | O_NONBLOCK)
        guard fd >= 0 else { throw failed() }
        defer { Darwin.close(fd) }
        var before = stat(), after = stat(), current = stat()
        guard fstat(fd, &before) == 0, (before.st_mode & S_IFMT) == S_IFREG,
              before.st_size >= 0, before.st_size <= limit else { throw failed() }
        var result = Data(), buffer = [UInt8](repeating: 0, count: 65_536)
        while true {
            let n = Darwin.read(fd, &buffer, buffer.count)
            if n < 0 && errno == EINTR { continue }
            guard n >= 0 else { throw failed() }
            if n == 0 { break }
            guard result.count + n <= limit else { throw failed() }
            result.append(contentsOf: buffer.prefix(n))
        }
        func same(_ a: stat, _ b: stat) -> Bool {
            a.st_dev == b.st_dev && a.st_ino == b.st_ino && a.st_size == b.st_size
                && a.st_mtimespec.tv_sec == b.st_mtimespec.tv_sec && a.st_mtimespec.tv_nsec == b.st_mtimespec.tv_nsec
                && a.st_ctimespec.tv_sec == b.st_ctimespec.tv_sec && a.st_ctimespec.tv_nsec == b.st_ctimespec.tv_nsec
        }
        guard fstat(fd, &after) == 0, same(before, after), result.count == before.st_size,
              fstatat(directory, name, &current, AT_SYMLINK_NOFOLLOW) == 0, same(before, current) else { throw failed() }
        return result
    }
    func load() throws -> Data? {
        let parent = Darwin.open(dataRoot.path, O_RDONLY | O_DIRECTORY | O_NOFOLLOW)
        guard parent >= 0 else { throw failed() }
        defer { Darwin.close(parent) }
        var info = stat()
        if fstatat(parent, "question-attachments", &info, AT_SYMLINK_NOFOLLOW) != 0 {
            guard errno == ENOENT else { throw failed() }; return nil
        }
        let dir = try directory(create: false); defer { Darwin.close(dir) }
        if fstatat(dir, "inventory.json", &info, AT_SYMLINK_NOFOLLOW) != 0 {
            guard errno == ENOENT, try isEmpty(dir) else {
                // Missing state beside any unknown byte fails closed. A failed first
                // replacement can leave only the empty directory and is retryable.
                throw failed()
            }
            return nil
        }
        return try readFile("inventory.json", in: dir, limit: 16 * 1024 * 1024)
    }
    func replace(_ data: Data) throws {
        let dir = try directory(create: true); defer { Darwin.close(dir) }
        let temporary = ".inventory-" + UUID().uuidString.lowercased()
        let fd = openat(dir, temporary, O_WRONLY | O_CREAT | O_EXCL | O_NOFOLLOW, 0o600)
        guard fd >= 0 else { throw failed() }
        defer { Darwin.close(fd); unlinkat(dir, temporary, 0) }
        try writeAll(data, to: fd)
        guard renameat(dir, temporary, dir, "inventory.json") == 0, fsync(dir) == 0 else { throw failed() }
    }
    func write(_ id: String, bytes: Data) throws {
        try valid(id)
        let dir = try directory(create: false); defer { Darwin.close(dir) }
        var info = stat()
        if fstatat(dir, id, &info, AT_SYMLINK_NOFOLLOW) == 0 {
            guard (info.st_mode & S_IFMT) == S_IFREG,
                  try readFile(id, in: dir, limit: bytes.count) == bytes else { throw failed() }
            return
        }
        guard errno == ENOENT else { throw failed() }
        let temporary = ".bytes-" + UUID().uuidString.lowercased()
        let fd = openat(dir, temporary, O_WRONLY | O_CREAT | O_EXCL | O_NOFOLLOW, 0o600)
        guard fd >= 0 else { throw failed() }
        defer { Darwin.close(fd); unlinkat(dir, temporary, 0) }
        try writeAll(bytes, to: fd)
        if renameatx_np(dir, temporary, dir, id, UInt32(RENAME_EXCL)) != 0 {
            guard errno == EEXIST, try readFile(id, in: dir, limit: bytes.count) == bytes else { throw failed() }
            return
        }
        guard fsync(dir) == 0 else { throw failed() }
    }
    func read(_ id: String, limit: Int) throws -> Data {
        try valid(id)
        let dir = try directory(create: false); defer { Darwin.close(dir) }
        return try readFile(id, in: dir, limit: limit)
    }
    func remove(_ id: String) throws {
        try valid(id)
        let dir = try directory(create: false); defer { Darwin.close(dir) }
        var info = stat()
        if fstatat(dir, id, &info, AT_SYMLINK_NOFOLLOW) != 0 {
            guard errno == ENOENT else { throw failed() }; return
        }
        guard (info.st_mode & S_IFMT) == S_IFREG, unlinkat(dir, id, 0) == 0, fsync(dir) == 0 else { throw failed() }
    }
}

func t3QuestionDigest(_ data: Data) -> String { SHA256.hash(data: data).map { String(format: "%02x", $0) }.joined() }
