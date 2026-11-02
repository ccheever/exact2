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

    /// Tests only: runs after the namespace is locked, before sweeping.
    nonisolated(unsafe) static var beforeSweep: (() -> Void)?

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
        let rootFD = open(root.path, O_RDONLY | O_DIRECTORY | O_NOFOLLOW | O_CLOEXEC)
        guard rootFD >= 0 else { throw POSIXError(POSIXErrorCode(rawValue: errno) ?? .EIO) }
        defer { close(rootFD) }
        var opened = stat()
        guard fstat(rootFD, &opened) == 0 else { throw POSIXError(POSIXErrorCode(rawValue: errno) ?? .EIO) }
        let namespace = try lockFile(rootFD, name: ".namespace.lock", create: true, attempts: attempts)
        defer { close(namespace) }
        beforeSweep?()
        sweep(rootFD)
        let name = UUID().uuidString
        let mine = root.appendingPathComponent(name, isDirectory: true)
        if mkdirat(rootFD, name, 0o700) != 0 { throw POSIXError(POSIXErrorCode(rawValue: errno) ?? .EIO) }
        do {
            let mineFD = openat(rootFD, name, O_RDONLY | O_DIRECTORY | O_NOFOLLOW | O_CLOEXEC)
            guard mineFD >= 0 else { throw POSIXError(POSIXErrorCode(rawValue: errno) ?? .EIO) }
            defer { close(mineFD) }
            let lock = try lockFile(mineFD, name: ".lock", create: true, attempts: 1)
            // Callers write by URL. Refuse a name that no longer reaches the
            // directory under which the descriptor made this spool.
            guard lstat(root.path, &info) == 0, info.st_mode & S_IFMT == S_IFDIR,
                  info.st_dev == opened.st_dev, info.st_ino == opened.st_ino else {
                close(lock)
                throw POSIXError(.ESTALE)
            }
            return (mine, lock)
        } catch {
            removeDirectory(rootFD, name: name)
            throw error
        }
    }

    /// Remove abandoned spool directories without following a replaced root.
    /// The caller holds the namespace lock.
    static func sweep(_ root: URL) {
        let fd = open(root.path, O_RDONLY | O_DIRECTORY | O_NOFOLLOW | O_CLOEXEC)
        guard fd >= 0 else { return }
        defer { close(fd) }
        sweep(fd)
    }

    private static func sweep(_ root: Int32) {
        for name in names(root) {
            let dir = openat(root, name, O_RDONLY | O_DIRECTORY | O_NOFOLLOW | O_CLOEXEC)
            guard dir >= 0 else { continue }
            let lock = openat(dir, ".lock", O_RDWR | O_CLOEXEC | O_NOFOLLOW)
            let abandoned = lock < 0 ? errno == ENOENT : flock(lock, LOCK_EX | LOCK_NB) == 0
            if abandoned { removeContents(dir); unlinkat(root, name, AT_REMOVEDIR) }
            if lock >= 0 { close(lock) }
            close(dir)
        }
    }

    private static func names(_ fd: Int32) -> [String] {
        // fdopendir owns its descriptor; keep the caller's descriptor open.
        let copy = dup(fd)
        guard copy >= 0 else { return [] }
        guard let directory = fdopendir(copy) else { close(copy); return [] }
        defer { closedir(directory) }
        var names: [String] = []
        while let entry = readdir(directory) {
            let name = withUnsafePointer(to: &entry.pointee.d_name) {
                $0.withMemoryRebound(to: CChar.self, capacity: Int(MAXPATHLEN)) { String(cString: $0) }
            }
            if name != ".", name != ".." { names.append(name) }
        }
        return names
    }

    private static func removeDirectory(_ parent: Int32, name: String) {
        let fd = openat(parent, name, O_RDONLY | O_DIRECTORY | O_NOFOLLOW | O_CLOEXEC)
        guard fd >= 0 else { return }
        defer { close(fd) }
        removeContents(fd)
        unlinkat(parent, name, AT_REMOVEDIR)
    }

    private static func removeContents(_ fd: Int32) {
        for name in names(fd) {
            var info = stat()
            guard fstatat(fd, name, &info, AT_SYMLINK_NOFOLLOW) == 0 else { continue }
            if info.st_mode & S_IFMT == S_IFDIR { removeDirectory(fd, name: name) }
            else { unlinkat(fd, name, 0) }
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

    /// An open descriptor on `name` under `directory`, holding its exclusive lock, asked for up to
    /// `attempts` times (never a blocking wait: another process holding it
    /// cannot stall the image workers), or a thrown errno.
    private static func lockFile(_ directory: Int32, name: String, create: Bool, attempts: Int) throws -> Int32 {
        let fd = openat(directory, name, O_RDWR | O_CLOEXEC | O_NOFOLLOW | (create ? O_CREAT : 0), 0o600)
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
