import Foundation

/// Where remote and `data:` image bytes are spooled (`RasterInput`): one
/// directory per process under `tmp/exact-raster/`, holding an exclusive
/// `flock` on its `.lock` for the process's life. A file is removed when its
/// input is released, but a process that is killed never releases them, so
/// the first spool of a launch removes every directory whose lock no live
/// process holds. On macOS the temporary directory is the user's, shared by
/// every app, which is why a live owner is told by its lock, not by age.
/// Creating a directory and sweeping both hold `.namespace.lock`, so a sweep
/// never sees a directory between its creation and its lock.
final class RasterSpool: @unchecked Sendable {
    static let shared = RasterSpool(tmp: URL(fileURLWithPath: NSTemporaryDirectory()))
    /// A spool asks for the namespace lock this often before it gives up for
    /// now (the image fails, and the next spool asks again).
    static let attempts = 20, pause: useconds_t = 25_000

    /// Tests only: runs between seeing a symlinked root and unlinking it, so
    /// another launch's repair can be put exactly there.
    nonisolated(unsafe) static var sawSymlink: (() -> Void)?

    let tmp: URL
    private let guarded = NSLock()
    private var established: (url: URL, lock: Int32)?

    init(tmp: URL) { self.tmp = tmp }

    /// A fresh spool file's URL in this process's directory.
    static func file() throws -> URL { try shared.directory().appendingPathComponent(UUID().uuidString) }

    /// This process's spool directory, made (and the root swept) on first
    /// use. A failure throws and the next spool tries again.
    func directory() throws -> URL {
        guarded.lock(); defer { guarded.unlock() }
        if let established { return established.url }
        #if os(iOS) || os(tvOS)
        // Before this directory, spools were loose `tmp/exact-raster-<uuid>`
        // files; an app's own sandboxed temporary directory holds no one else's.
        Self.sweepLoose(tmp, now: Date())
        #endif
        let made = try Self.establish(in: tmp.appendingPathComponent("exact-raster", isDirectory: true))
        established = made
        return made.url
    }

    /// The descriptor holding this process's lock, once established.
    var heldLock: Int32? { guarded.lock(); defer { guarded.unlock() }; return established?.lock }

    /// Sweep `root`, then make a directory in it whose lock the returned
    /// descriptor holds (never closed: it closes at exit, and `O_CLOEXEC`
    /// keeps an exec'd child from holding it after).
    static func establish(in root: URL) throws -> (url: URL, lock: Int32) {
        // A symlink in the root's place is unlinked, never followed (unlink
        // cannot remove a directory, so a real root another launch just made
        // survives a concurrent repair), and a real directory takes its place.
        var info = stat()
        if lstat(root.path, &info) == 0, info.st_mode & S_IFMT == S_IFLNK {
            sawSymlink?()
            unlink(root.path)
        }
        if mkdir(root.path, 0o700) != 0, errno != EEXIST { throw POSIXError(POSIXErrorCode(rawValue: errno) ?? .EIO) }
        // A link that could not be unlinked (an immutable link, a read-only
        // tmp) is not a root: nothing is opened through it.
        guard lstat(root.path, &info) == 0, info.st_mode & S_IFMT == S_IFDIR else { throw POSIXError(.ENOTDIR) }
        let namespace = try lockFile(root.appendingPathComponent(".namespace.lock"), create: true, attempts: attempts)
        defer { close(namespace) }
        // Swept and made only inside a real directory, checked again under the lock.
        guard lstat(root.path, &info) == 0, info.st_mode & S_IFMT == S_IFDIR else { throw POSIXError(.ENOTDIR) }
        sweep(root)
        let mine = root.appendingPathComponent(UUID().uuidString, isDirectory: true)
        if mkdir(mine.path, 0o700) != 0 { throw POSIXError(POSIXErrorCode(rawValue: errno) ?? .EIO) }
        do {
            return (mine, try lockFile(mine.appendingPathComponent(".lock"), create: true, attempts: 1))
        } catch {
            try? FileManager.default.removeItem(at: mine)
            throw error
        }
    }

    /// Remove the directories under `root` that no live process owns: one
    /// whose lock file is gone, or whose lock this process can take. The
    /// caller holds the namespace lock. Any other failure (a lock that cannot
    /// be opened says nothing of its owner) keeps the directory.
    static func sweep(_ root: URL) {
        let fm = FileManager.default
        guard let entries = try? fm.contentsOfDirectory(at: root, includingPropertiesForKeys: [.isDirectoryKey, .isSymbolicLinkKey]) else { return }
        for dir in entries {
            // A real directory only: a symlink is left alone, never followed.
            guard let values = try? dir.resourceValues(forKeys: [.isDirectoryKey, .isSymbolicLinkKey]),
                  values.isDirectory == true, values.isSymbolicLink != true else { continue }
            let fd = open(dir.appendingPathComponent(".lock").path, O_RDWR | O_CLOEXEC | O_NOFOLLOW)
            if fd < 0 {
                if errno == ENOENT { try? fm.removeItem(at: dir) }
                continue
            }
            if flock(fd, LOCK_EX | LOCK_NB) == 0 { try? fm.removeItem(at: dir) }
            close(fd)
        }
    }

    /// Remove loose `exact-raster-<uuid>` files older than a minute.
    static func sweepLoose(_ tmp: URL, now: Date) {
        let fm = FileManager.default
        guard let entries = try? fm.contentsOfDirectory(at: tmp, includingPropertiesForKeys: [.contentModificationDateKey, .isRegularFileKey]) else { return }
        for file in entries where file.lastPathComponent.hasPrefix("exact-raster-") {
            guard let values = try? file.resourceValues(forKeys: [.contentModificationDateKey, .isRegularFileKey]), values.isRegularFile == true,
                  let modified = values.contentModificationDate, now.timeIntervalSince(modified) > 60 else { continue }
            try? fm.removeItem(at: file)
        }
    }

    /// An open descriptor on `url` holding its exclusive lock, asked for up to
    /// `attempts` times (never a blocking wait: another process holding it
    /// cannot stall the image workers), or a thrown errno.
    private static func lockFile(_ url: URL, create: Bool, attempts: Int) throws -> Int32 {
        let fd = open(url.path, O_RDWR | O_CLOEXEC | O_NOFOLLOW | (create ? O_CREAT : 0), 0o600)
        guard fd >= 0 else { throw POSIXError(POSIXErrorCode(rawValue: errno) ?? .EIO) }
        for attempt in 1...max(1, attempts) {
            if flock(fd, LOCK_EX | LOCK_NB) == 0 { return fd }
            let code = errno
            if code != EWOULDBLOCK || attempt == attempts { close(fd); throw POSIXError(POSIXErrorCode(rawValue: code) ?? .EIO) }
            usleep(pause)
        }
        close(fd)
        throw POSIXError(.EWOULDBLOCK)
    }
}
