// `t3 app <dir>` opens a project in the running app (20261005-app-activation): the control socket
// and the broker, after T3 Code's desktop app (MIT, see LICENSE-T3; reference 1e2ecbd975:
// apps/desktop/src/app/DesktopAppActivation.ts `startDesktopAppControlServer`,
// DesktopAppActivationBroker.ts, packages/shared/src/desktopAppControl.ts
// `resolveDesktopAppControlAddress`, packages/contracts/src/desktopAppActivation.ts).
//
// The CLI (`t3 app`, the server's own command, apps/server/src/cli/app.ts) dials
// `<$TMPDIR>/t3code-<uid>/<24 hex of sha256(<T3 home>/userdata)>.sock`, writes one JSON line and
// waits for one JSON line. The app listens there while its embedded server runs: started once the
// server's start is under way (DesktopApp.ts bootstrap order), never while the Local environment is
// off (the reference's relaunch then has no socket; this app's in-place stopgap closes it), and
// closed with the last session (exact2 #200: `destroy()` runs at quit). A bind failure is logged
// and the app goes on. Requests wait until the window says it is ready (desktop-activation.ts), go
// to it one at a time, and the window is brought to the front when one arrives.
//
// Two apps on one T3 home share the path: the newer takes it over (bind a staging name, then
// `rename`), a close unlinks the path only while it is still this app's inode, and an app binds
// the path again when it vanishes (a directory watch; `link` claims it only while free).
import AppKit
import CryptoKit
import Darwin
import Foundation

/// resolveDesktopAppControlAddress, the Unix half (macOS: no named pipe).
enum T3AppControlAddress {
    /// The first 12 bytes of SHA-256, as 24 hex digits.
    static func shortHash(_ value: String) -> String {
        SHA256.hash(data: Data(value.utf8)).prefix(12).map { String(format: "%02x", $0) }.joined()
    }

    /// Node's `os.tmpdir()` on macOS, which the CLI uses: TMPDIR, TMP or TEMP, else /tmp, less one trailing slash.
    static func nodeTmpdir(_ env: [String: String]) -> String {
        var path = [env["TMPDIR"], env["TMP"], env["TEMP"]].compactMap { $0 }.first { !$0.isEmpty } ?? "/tmp"
        if path.count > 1, path.hasSuffix("/") { path.removeLast() }
        return path
    }

    /// `path.join` / `path.resolve` of an absolute POSIX path: `.` and `..` resolved, repeated and
    /// trailing slashes dropped. Symlinks are never resolved (the CLI does not resolve them either).
    static func normalize(_ path: String) -> String {
        var parts: [Substring] = []
        for part in path.split(separator: "/", omittingEmptySubsequences: true) {
            if part == "." { continue }
            if part == ".." { if !parts.isEmpty { parts.removeLast() }; continue }
            parts.append(part)
        }
        return "/" + parts.joined(separator: "/")
    }

    /// The socket and its directory for a state directory (`<T3 home>/userdata`).
    static func resolve(stateDir: String, tempDir: String, userId: uid_t?) -> (address: String, directory: String) {
        let hash = shortHash(stateDir)
        let directory = normalize("\(tempDir)/t3code-\(userId.map { String($0) } ?? String(hash.prefix(12)))")
        return (normalize("\(directory)/\(hash).sock"), directory)
    }
}

/// The protocol's shapes (desktopAppActivation.ts), as JSON objects.
enum T3ActivationProtocol {
    static let version = 1
    static let platforms: Set<String> = ["darwin", "linux", "win32"]
    static let codes: Set<String> = ["invalid-request", "renderer-unavailable", "environment-unavailable", "platform-mismatch",
                                     "project-create-failed", "thread-open-failed", "request-timeout", "internal-error"]

    static func failure(_ requestId: String, _ code: String, _ message: String) -> [String: Any] {
        ["version": version, "requestId": requestId, "ok": false, "code": code, "message": message]
    }
    private static func isVersion(_ value: Any?) -> Bool {
        guard let number = value as? NSNumber, CFGetTypeID(number) != CFBooleanGetTypeID() else { return false }
        return number.doubleValue == Double(version)
    }
    private static func nonEmpty(_ value: Any?) -> String? { (value as? String).flatMap { $0.isEmpty ? nil : $0 } }
    private static func isBool(_ value: Any?) -> Bool { (value as? NSNumber).map { CFGetTypeID($0) == CFBooleanGetTypeID() } ?? false }

    /// Schema.is(DesktopAppActivationRequest).
    static func request(_ value: Any) -> [String: Any]? {
        guard let raw = value as? [String: Any], isVersion(raw["version"]), nonEmpty(raw["requestId"]) != nil, raw["type"] as? String == "open-workspace",
              nonEmpty(raw["workspaceRoot"]) != nil, let platform = raw["platform"] as? String, platforms.contains(platform) else { return nil }
        return raw
    }
    /// Schema.is(DesktopAppActivationResponse): what the window may answer with.
    static func response(_ value: Any?) -> [String: Any]? {
        guard let raw = value as? [String: Any], isVersion(raw["version"]), nonEmpty(raw["requestId"]) != nil, isBool(raw["ok"]) else { return nil }
        if raw["ok"] as? Bool == true { return nonEmpty(raw["projectId"]) != nil && nonEmpty(raw["threadId"]) != nil ? raw : nil }
        guard let code = raw["code"] as? String, codes.contains(code), nonEmpty(raw["message"]) != nil else { return nil }
        return raw
    }
    /// requestIdFromUnknown: an invalid request's id when it has one, for the answer.
    static func requestId(fromUnknown value: Any) -> String {
        guard let id = (value as? [String: Any])?["requestId"] as? String, !id.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty else { return "invalid-request" }
        return id
    }
}

/// DesktopAppActivationBroker: holds CLI requests until the window is ready to handle them, hands
/// them over one at a time, and answers each one exactly once. Not thread-safe: one serial queue
/// (T3AppControl's) calls it, and `schedule` runs its timeouts there.
final class T3ActivationBroker {
    typealias Request = [String: Any]
    typealias Response = [String: Any]
    /// Runs `fire` once after the interval; returns what cancels it.
    typealias Schedule = (TimeInterval, @escaping () -> Void) -> () -> Void
    struct RendererGone: Error {}

    private final class Pending {
        let request: Request
        let resolve: (Response) -> Void
        var cancelTimeout: () -> Void = {}
        var dispatched = false
        init(_ request: Request, _ resolve: @escaping (Response) -> Void) { self.request = request; self.resolve = resolve }
    }

    private var order: [String] = []
    private var pending: [String: Pending] = [:]
    private let requestTimeout: TimeInterval
    private let activate: () -> Void
    private let schedule: Schedule
    private var renderer: ((Request) throws -> Void)?
    private var closed = false

    init(requestTimeout: TimeInterval, activate: @escaping () -> Void, schedule: @escaping Schedule) {
        self.requestTimeout = requestTimeout; self.activate = activate; self.schedule = schedule
    }

    var pendingCount: Int { order.count }
    /// The request handed to the window and not answered yet.
    var dispatchedId: String? { order.first { pending[$0]?.dispatched == true } }
    func pendingRequest(_ requestId: String) -> Request? { pending[requestId]?.request }

    func request(_ request: Request, resolve: @escaping (Response) -> Void) {
        let id = request["requestId"] as? String ?? ""
        if closed { return resolve(T3ActivationProtocol.failure(id, "renderer-unavailable", "T3 Code is shutting down.")) }
        if pending[id] != nil { return resolve(T3ActivationProtocol.failure(id, "invalid-request", "The request id is already in use.")) }
        let entry = Pending(request, resolve)
        entry.cancelTimeout = schedule(requestTimeout) { [weak self] in
            self?.settle(T3ActivationProtocol.failure(id, "request-timeout", "The desktop app did not finish opening the project in time."))
        }
        pending[id] = entry; order.append(id)
        activate()
        flush()
    }

    func registerRenderer(_ send: @escaping (Request) throws -> Void) {
        renderer = send
        flush()
    }

    func clearRenderer() {
        renderer = nil
        for id in order where pending[id]?.dispatched == true {
            settle(T3ActivationProtocol.failure(id, "renderer-unavailable", "The T3 Code window closed before it opened the project."))
        }
    }

    func complete(_ response: Response) { settle(response) }

    func cancel(_ requestId: String) {
        settle(T3ActivationProtocol.failure(requestId, "renderer-unavailable", "The command closed before T3 Code was ready."))
    }

    func close() {
        closed = true
        renderer = nil
        for id in order { settle(T3ActivationProtocol.failure(id, "renderer-unavailable", "T3 Code is shutting down.")) }
    }

    private func flush() {
        guard let renderer, !order.contains(where: { pending[$0]?.dispatched == true }) else { return }
        guard let id = order.first(where: { pending[$0]?.dispatched == false }), let entry = pending[id] else { return }
        entry.dispatched = true
        do { try renderer(entry.request) } catch { entry.dispatched = false; self.renderer = nil }
    }

    private func settle(_ response: Response) {
        guard let id = response["requestId"] as? String, let entry = pending.removeValue(forKey: id) else { return }
        order.removeAll { $0 == id }
        entry.cancelTimeout()
        entry.resolve(response)
        flush()
    }
}

/// startDesktopAppControlServer on a Unix socket. Every method runs on `queue`, the one that
/// `handle`'s answers arrive on.
final class T3AppControlServer {
    struct Failure: Error, CustomStringConvertible {
        let description: String
        var code: Int32 = 0
        static func errno(_ what: String, _ path: String) -> Failure {
            let code = Darwin.errno
            return Failure(description: "\(what) \(path): \(String(cString: strerror(code)))", code: code)
        }
    }
    static let maxRequestBytes = 64 * 1024
    /// The time a client has to send its request line (an idle connection is dropped).
    static var requestLineTimeout: TimeInterval = 5

    let address: String
    let directory: String
    private let userId: uid_t
    private let queue: DispatchQueue
    private let queueKey = DispatchSpecificKey<UInt8>()
    private let handle: ([String: Any], @escaping ([String: Any]) -> Void) -> Void
    private let cancel: (String) -> Void
    private let onReclaimError: (String) -> Void
    private var listener: Listener?
    private(set) var inode: UInt64?
    private var watcher: DispatchSourceFileSystemObject?
    private var connections: [ObjectIdentifier: Connection] = [:]
    private var closed = false

    // A process exit that skips the sessions' teardown (the agent driver's end of drive calls `exit(0)`)
    // still removes the socket files that are still this process's, as T3LocalBackend's exit hook stops its server.
    private static let boundLock = NSLock()
    private static var bound: [ObjectIdentifier: (address: String, inode: UInt64)] = [:]
    private static let exitHook: Void = { atexit { T3AppControlServer.unlinkAtExit() } }()
    private static var exiting = false
    private static var isExiting: Bool { boundLock.lock(); defer { boundLock.unlock() }; return exiting }
    private static func unlinkAtExit() {
        // The watch must not bind the path again while the rest of the exit (the server's stop) runs.
        boundLock.lock(); exiting = true; let records = Array(bound.values); bound.removeAll(); boundLock.unlock()
        for record in records where (try? inodeAt(record.address)) == record.inode { unlink(record.address) }
    }
    private func record(_ inode: UInt64?) {
        Self.boundLock.lock(); defer { Self.boundLock.unlock() }
        if let inode { Self.bound[ObjectIdentifier(self)] = (address, inode) } else { Self.bound.removeValue(forKey: ObjectIdentifier(self)) }
    }

    private final class Listener {
        let source: DispatchSourceRead
        init(fd: Int32, queue: DispatchQueue, accept: @escaping (Int32) -> Void) {
            source = DispatchSource.makeReadSource(fileDescriptor: fd, queue: queue)
            source.setEventHandler { while true { let client = Darwin.accept(fd, nil, nil); if client < 0 { return }; accept(client) } }
            source.setCancelHandler { Darwin.close(fd) }
            source.resume()
        }
        func close() { source.cancel() }
    }

    private final class Connection {
        let fd: Int32
        var source: DispatchSourceRead?
        var timer: DispatchSourceTimer?
        var buffer = Data()
        var handled = false, responseSent = false, closed = false
        var activeRequestId: String?
        init(fd: Int32) { self.fd = fd }
    }

    /// Takes the address over (call it on `queue`). Throws why it could not.
    init(address: String, directory: String, userId: uid_t, queue: DispatchQueue,
         handle: @escaping ([String: Any], @escaping ([String: Any]) -> Void) -> Void,
         cancel: @escaping (String) -> Void, onReclaimError: @escaping (String) -> Void) throws {
        self.address = address; self.directory = directory; self.userId = userId; self.queue = queue
        self.handle = handle; self.cancel = cancel; self.onReclaimError = onReclaimError
        queue.setSpecific(key: queueKey, value: 1)
        _ = try Self.prepareDirectory(directory, userId: userId)
        let bound = try Self.bindUnix(directory: directory, address: address, claimFree: false)
        listener = Listener(fd: bound.fd, queue: queue, accept: { [weak self] client in self?.accepted(client) })
        inode = bound.inode
        _ = Self.exitHook
        record(inode)
        watch()
        // A removal before the watch started.
        reclaimReporting()
    }

    // ── Address ownership ──────────────────────────────────────────────
    /// Makes sure the socket directory is safe to use (0700, this user's, not a symlink). True when it had to create it.
    static func prepareDirectory(_ directory: String, userId: uid_t) throws -> Bool {
        var info = stat()
        let existed = lstat(directory, &info) == 0
        if !existed {
            do { try FileManager.default.createDirectory(atPath: directory, withIntermediateDirectories: true, attributes: [.posixPermissions: 0o700]) }
            catch { throw Failure(description: "mkdir \(directory): \(error.localizedDescription)") }
        }
        guard lstat(directory, &info) == 0 else { throw Failure.errno("lstat", directory) }
        guard info.st_mode & S_IFMT == S_IFDIR else { throw Failure(description: "\(directory) is not a directory.") }
        guard info.st_uid == userId else { throw Failure(description: "\(directory) is owned by another user.") }
        guard chmod(directory, 0o700) == 0 else { throw Failure.errno("chmod", directory) }
        return !existed
    }

    static func inodeAt(_ path: String) throws -> UInt64? {
        var info = stat()
        if lstat(path, &info) == 0 { return UInt64(info.st_ino) }
        if errno == ENOENT { return nil }
        throw Failure.errno("lstat", path)
    }

    /// socket + bind + listen on `path` (non-blocking, close-on-exec).
    static func listen(at path: String) throws -> Int32 {
        var addr = sockaddr_un()
        let bytes = Array(path.utf8)
        guard bytes.count < MemoryLayout.size(ofValue: addr.sun_path) else { throw Failure(description: "\(path) is too long for a Unix socket (\(bytes.count) bytes).") }
        let fd = socket(AF_UNIX, SOCK_STREAM, 0)
        guard fd >= 0 else { throw Failure.errno("socket", path) }
        _ = fcntl(fd, F_SETFD, FD_CLOEXEC); _ = fcntl(fd, F_SETFL, fcntl(fd, F_GETFL) | O_NONBLOCK)
        addr.sun_family = sa_family_t(AF_UNIX)
        addr.sun_len = UInt8(MemoryLayout<sockaddr_un>.size)
        withUnsafeMutableBytes(of: &addr.sun_path) { $0.copyBytes(from: bytes) }
        let bound = withUnsafePointer(to: &addr) { $0.withMemoryRebound(to: sockaddr.self, capacity: 1) { bind(fd, $0, socklen_t(MemoryLayout<sockaddr_un>.size)) } }
        guard bound == 0 else { let failure = Failure.errno("bind", path); Darwin.close(fd); throw failure }
        guard Darwin.listen(fd, 128) == 0 else { let failure = Failure.errno("listen", path); Darwin.close(fd); unlink(path); throw failure }
        return fd
    }

    /// Binds a staging path and moves it onto the address: `rename` takes the address over in one
    /// step; `link` claims it only while it is free (EEXIST otherwise). A later close of the old
    /// listener then never touches the address.
    static func bindUnix(directory: String, address: String, claimFree: Bool) throws -> (fd: Int32, inode: UInt64) {
        var random = [UInt8](repeating: 0, count: 6)
        _ = SecRandomCopyBytes(kSecRandomDefault, random.count, &random)
        let staging = "\(directory)/\(random.map { String(format: "%02x", $0) }.joined()).tmp"
        let fd = try listen(at: staging)
        do {
            guard chmod(staging, 0o600) == 0 else { throw Failure.errno("chmod", staging) }
            guard let inode = try inodeAt(staging) else { throw Failure(description: "\(staging) vanished.") }
            if claimFree {
                guard link(staging, address) == 0 else { throw Failure.errno("link", address) }
                unlink(staging)
            } else {
                guard rename(staging, address) == 0 else { throw Failure.errno("rename", address) }
            }
            return (fd, inode)
        } catch {
            Darwin.close(fd); unlink(staging)
            throw error
        }
    }

    private func watch() {
        watcher?.cancel(); watcher = nil
        let fd = open(directory, O_EVTONLY)
        guard fd >= 0 else { return onReclaimError(Failure.errno("watch", directory).description) }
        let source = DispatchSource.makeFileSystemObjectSource(fileDescriptor: fd, eventMask: [.write, .delete, .rename, .link, .revoke], queue: queue)
        source.setEventHandler { [weak self] in self?.reclaimReporting() }
        source.setCancelHandler { Darwin.close(fd) }
        watcher = source
        source.resume()
    }

    private func reclaimReporting() {
        do { try reclaim() } catch { onReclaimError("\(error)") }
    }

    /// Binds the address again if its socket file is gone (directory changes run this too). Never
    /// replaces a socket that exists, so two apps cannot trade the path back and forth.
    func reclaim() throws {
        if closed || Self.isExiting { return }
        if try Self.inodeAt(address) != nil { return }
        // A watch follows the directory's inode, so a recreated directory needs a new one.
        if try Self.prepareDirectory(directory, userId: userId) { watch() }
        let next: (fd: Int32, inode: UInt64)
        do { next = try Self.bindUnix(directory: directory, address: address, claimFree: true) }
        catch let failure as Failure where failure.code == EEXIST { return } // another app bound it first
        let previous = listener
        listener = Listener(fd: next.fd, queue: queue, accept: { [weak self] client in self?.accepted(client) })
        inode = next.inode
        record(inode)
        previous?.close()
    }

    /// Stops listening, drops the connections (each unanswered request is cancelled) and removes the
    /// socket file while it is still this server's.
    func close() {
        if closed { return }
        closed = true
        watcher?.cancel(); watcher = nil
        for connection in Array(connections.values) { destroy(connection) }
        listener?.close(); listener = nil
        if let inode, (try? Self.inodeAt(address)) == inode { unlink(address) }
        record(nil)
    }

    // ── Connections: one JSON line in, one JSON line out ─────────────────
    private func accepted(_ fd: Int32) {
        guard !closed else { Darwin.close(fd); return }
        var yes: Int32 = 1
        setsockopt(fd, SOL_SOCKET, SO_NOSIGPIPE, &yes, socklen_t(MemoryLayout<Int32>.size))
        _ = fcntl(fd, F_SETFD, FD_CLOEXEC); _ = fcntl(fd, F_SETFL, fcntl(fd, F_GETFL) | O_NONBLOCK)
        let connection = Connection(fd: fd)
        connections[ObjectIdentifier(connection)] = connection
        let source = DispatchSource.makeReadSource(fileDescriptor: fd, queue: queue)
        source.setEventHandler { [weak self, weak connection] in if let self, let connection { self.readable(connection) } }
        source.setCancelHandler { Darwin.close(fd) }
        connection.source = source
        armTimeout(connection)
        source.resume()
    }

    private func armTimeout(_ connection: Connection) {
        connection.timer?.cancel(); connection.timer = nil
        guard !connection.handled else { return }
        let timer = DispatchSource.makeTimerSource(queue: queue)
        timer.schedule(deadline: .now() + Self.requestLineTimeout)
        timer.setEventHandler { [weak self, weak connection] in if let self, let connection { self.destroy(connection) } }
        connection.timer = timer
        timer.resume()
    }

    private func readable(_ connection: Connection) {
        var chunk = [UInt8](repeating: 0, count: 16 * 1024)
        while !connection.closed {
            let count = read(connection.fd, &chunk, chunk.count)
            if count > 0 { received(connection, chunk[0..<count]); continue }
            if count < 0, errno == EINTR { continue }
            if count < 0, errno == EAGAIN || errno == EWOULDBLOCK { return }
            return destroy(connection) // end of stream or an error: the client went away
        }
    }

    private func received(_ connection: Connection, _ bytes: ArraySlice<UInt8>) {
        guard !connection.handled else { return }
        connection.buffer.append(contentsOf: bytes)
        armTimeout(connection)
        if connection.buffer.count > Self.maxRequestBytes {
            connection.handled = true
            return finish(connection, T3ActivationProtocol.failure("invalid-request", "invalid-request", "The desktop app request is too large."))
        }
        guard let newline = connection.buffer.firstIndex(of: 0x0A) else { return }
        connection.handled = true
        armTimeout(connection)
        let line = connection.buffer[connection.buffer.startIndex..<newline]
        guard let parsed = try? JSONSerialization.jsonObject(with: Data(line), options: [.fragmentsAllowed]) else {
            return finish(connection, T3ActivationProtocol.failure("invalid-request", "invalid-request", "The desktop app request is not valid JSON."))
        }
        guard let request = T3ActivationProtocol.request(parsed), let id = request["requestId"] as? String else {
            return finish(connection, T3ActivationProtocol.failure(T3ActivationProtocol.requestId(fromUnknown: parsed), "invalid-request", "The desktop app request is invalid."))
        }
        connection.activeRequestId = id
        handle(request) { [weak self, weak connection] response in
            guard let self else { return }
            let answer = { if let connection { self.finish(connection, response) } }
            if DispatchQueue.getSpecific(key: self.queueKey) != nil { answer() } else { self.queue.async(execute: answer) }
        }
    }

    private func finish(_ connection: Connection, _ response: [String: Any]) {
        connection.responseSent = true
        guard !connection.closed else { return }
        if var data = try? JSONSerialization.data(withJSONObject: response, options: [.sortedKeys]) {
            data.append(0x0A)
            _ = fcntl(connection.fd, F_SETFL, fcntl(connection.fd, F_GETFL) & ~O_NONBLOCK)
            data.withUnsafeBytes { raw in
                var offset = 0
                while offset < raw.count {
                    let written = write(connection.fd, raw.baseAddress! + offset, raw.count - offset)
                    if written < 0, errno == EINTR { continue }
                    if written <= 0 { return }
                    offset += written
                }
            }
        }
        destroy(connection) // socket.end(): the answer is the last thing on the connection
    }

    private func destroy(_ connection: Connection) {
        guard !connection.closed else { return }
        connection.closed = true
        connection.timer?.cancel(); connection.timer = nil
        connection.source?.cancel(); connection.source = nil
        connections.removeValue(forKey: ObjectIdentifier(connection))
        // A client that leaves before its answer cancels its request.
        if !connection.responseSent, let id = connection.activeRequestId { cancel(id) }
    }
}

/// The app's one control socket and broker, shared by its sessions: each session's module is a
/// window that may be ready to open projects (`activationReady`); the latest ready one is handed
/// the requests. It follows the embedded server's status (T3LocalBackend `t3Home`, `enabled`).
final class T3AppControl {
    static let shared = T3AppControl()
    static let topic = "t3.activation"
    /// REQUEST_TIMEOUT_MS; `T3_ACTIVATION_TIMEOUT_MS` shortens it in a development build (the expired-request row).
    static func requestTimeout(env: [String: String], packaged: Bool) -> TimeInterval {
        if !packaged, let raw = env["T3_ACTIVATION_TIMEOUT_MS"], let ms = Double(raw), ms >= 1 { return ms / 1000 }
        return 15
    }

    let queue = DispatchQueue(label: "com.exact.t3code.activation", qos: .userInitiated)

    private final class Window {
        weak var module: AnyObject?
        let changed: (String) -> Void
        var ready = false
        var token = ""
        var handed: String?
        /// Its NSWindow began to close: the page is gone for the broker (the session's own teardown can wait on its last answer).
        var closed = false
        weak var nsWindow: NSWindow?
        var closeObserver: NSObjectProtocol?
        init(module: AnyObject, changed: @escaping (String) -> Void) { self.module = module; self.changed = changed }
    }
    private var windows: [ObjectIdentifier: Window] = [:]
    private var renderer: ObjectIdentifier?
    private var server: T3AppControlServer?
    private var broker: T3ActivationBroker?
    private var runningFor = ""
    private var failedFor = ""
    private var lastError = ""
    private var observing = false

    // Seams for the tests.
    var environment: () -> [String: String] = { ProcessInfo.processInfo.environment }
    var backendStatus: () -> [String: Any] = { T3LocalBackend.shared.statusValue() }
    var observeBackend: (AnyObject, URL, @escaping () -> Void) -> Void = { owner, dataRoot, changed in T3LocalBackend.shared.attach(owner, dataRoot: dataRoot, changed: { _ in changed() }) }
    var stopObservingBackend: (AnyObject) -> Void = { T3LocalBackend.shared.detach($0) }
    var activateWindow: () -> Void = { DispatchQueue.main.async { T3AppControl.activateMainWindow() } }
    var packaged: () -> Bool = { T3LocalPolicy.packaged(resources: Bundle.main.resourceURL) }
    var log: (String) -> Void = { FileHandle.standardError.write(Data("t3.activation: \($0)\n".utf8)) }

    /// DesktopWindow.activate: the app to the front, its window restored and focused. No toast.
    static func activateMainWindow() {
        NSApp.activate(ignoringOtherApps: true)
        guard let window = activationTarget(main: NSApp.mainWindow, windows: NSApp.windows) else { return }
        if window.isMiniaturized { window.deminiaturize(nil) }
        window.makeKeyAndOrderFront(nil)
    }

    /// The window `activate` brings back (ElectronWindow's focusedMainOrFirst, restored when minimized): the main
    /// window, else the first document window on screen, else the first one in the Dock. Chosen by kind, not by
    /// `canBecomeMain`, which AppKit answers false for a window that is not visible: a minimized window was never a
    /// candidate, stayed in the Dock, and its page never handled the request (fix-misc-batch, #298 bug 7).
    static func activationTarget<Window: T3ActivationWindow>(main: Window?, windows: [Window]) -> Window? {
        if let main { return main }
        let documents = windows.filter(\.isActivationDocument)
        return documents.first(where: \.isVisible) ?? documents.first(where: \.isMiniaturized)
    }

    /// A session's module arrived. The first one starts following the embedded server, as a listener
    /// of its own: the module attached first, so this never begins the backend, and `detach` lets go
    /// before the last module does, so this never keeps it running.
    func attach(_ module: AnyObject, dataRoot: URL, changed: @escaping (String) -> Void) {
        queue.sync {
            windows[ObjectIdentifier(module)] = Window(module: module, changed: changed)
            if !observing {
                observing = true
                observeBackend(self, dataRoot) { [weak self] in self?.queue.async { self?.backendChanged() } }
            }
            backendChanged()
        }
    }

    /// A session's module was destroyed (a window closed, or the app quits): its requests fail as
    /// the reference's renderer going away does; the last one closes the socket.
    func detach(_ module: AnyObject) {
        queue.sync {
            let key = ObjectIdentifier(module)
            if let observer = windows.removeValue(forKey: key)?.closeObserver { NotificationCenter.default.removeObserver(observer) }
            if renderer == key { renderer = nil; broker?.clearRenderer() }
            guard windows.isEmpty else { return }
            stop()
            if observing { observing = false; stopObservingBackend(self) }
        }
    }

    /// On `queue`: listen while the embedded server is wanted (switch on, not refused) and its home is known.
    private func backendChanged() {
        let status = backendStatus()
        let home = (status["t3Home"] as? String).flatMap { $0.isEmpty ? nil : $0 }
        let wanted = status["enabled"] as? Bool != false && status["state"] as? String != "refused" && !windows.isEmpty
        guard wanted, let home else { failedFor = ""; return stop() }
        let stateDir = T3AppControlAddress.normalize("\(home)/userdata")
        if runningFor == stateDir || failedFor == stateDir { return }
        stop()
        start(stateDir: stateDir)
    }

    private func start(stateDir: String) {
        let target = T3AppControlAddress.resolve(stateDir: stateDir, tempDir: T3AppControlAddress.nodeTmpdir(environment()), userId: getuid())
        let queue = self.queue
        let broker = T3ActivationBroker(requestTimeout: Self.requestTimeout(env: environment(), packaged: packaged()), activate: activateWindow) { seconds, fire in
            let item = DispatchWorkItem(block: fire)
            queue.asyncAfter(deadline: .now() + seconds, execute: item)
            return { item.cancel() }
        }
        do {
            server = try T3AppControlServer(address: target.address, directory: target.directory, userId: getuid(), queue: queue,
                                            handle: { [weak self, weak broker] request, done in
                                                let id = request["requestId"] as? String ?? ""
                                                guard let broker else { return done(T3ActivationProtocol.failure(id, "renderer-unavailable", "T3 Code is shutting down.")) }
                                                self?.log("request \(id) arrived")
                                                broker.request(request) { response in
                                                    self?.log("request \(id) answered: \(response["ok"] as? Bool == true ? "opened project \(response["projectId"] ?? "") thread \(response["threadId"] ?? "")" : "\(response["code"] ?? "") \(response["message"] ?? "")")")
                                                    done(response)
                                                } },
                                            cancel: { [weak self, weak broker] id in self?.log("request \(id): its command line closed the connection"); broker?.cancel(id) },
                                            onReclaimError: { [weak self] message in self?.log("failed to restore the desktop app control socket: \(message)") })
            self.broker = broker
            runningFor = stateDir; failedFor = ""; lastError = ""
            log("desktop app control socket ready at \(target.address)")
            if let renderer, windows[renderer]?.ready == true { broker.registerRenderer(sender(renderer)) }
        } catch {
            broker.close()
            failedFor = stateDir; lastError = "\(error)"
            log("desktop app control socket unavailable: \(error)")
        }
    }

    private func stop() {
        guard server != nil || broker != nil else { return }
        server?.close(); broker?.close()
        server = nil; broker = nil; runningFor = ""
        for window in windows.values { window.handed = nil }
    }

    /// The window as the broker's renderer: remember the request it is handed and tell it.
    private func sender(_ key: ObjectIdentifier) -> (T3ActivationBroker.Request) throws -> Void {
        { [weak self] request in
            guard let window = self?.windows[key], window.module != nil else { throw T3ActivationBroker.RendererGone() }
            window.handed = request["requestId"] as? String
            self?.log("request \(window.handed ?? "") handed to the window")
            window.changed(Self.topic)
        }
    }

    /// The module's window, once one of its views is in it: when it starts to close, the window's
    /// requests fail at once, as the reference's renderer `destroyed` event does. (Exact destroys the
    /// session, and so the module, only after the window's last answer settles, which a request that
    /// waits on the server would hold.)
    func observeWindow(_ module: AnyObject, _ nsWindow: NSWindow) {
        queue.sync {
            // The session's document window, not a popup's panel; kept while it stays open.
            guard nsWindow.canBecomeMain, let window = windows[ObjectIdentifier(module)], window.nsWindow !== nsWindow,
                  window.nsWindow == nil || window.nsWindow?.isVisible == false else { return }
            if let observer = window.closeObserver { NotificationCenter.default.removeObserver(observer) }
            window.nsWindow = nsWindow
            log("watching the window \(nsWindow.windowNumber) for its close")
            window.closeObserver = NotificationCenter.default.addObserver(forName: NSWindow.willCloseNotification, object: nsWindow, queue: nil) { [weak self, weak module] _ in
                if let module { self?.windowClosing(module) }
            }
        }
    }

    func windowClosing(_ module: AnyObject) {
        queue.sync {
            let key = ObjectIdentifier(module)
            guard let window = windows[key] else { return }
            log("the window is closing\(renderer == key && broker?.dispatchedId != nil ? ": its request fails" : "")")
            window.closed = true; window.ready = false; window.handed = nil
            if renderer == key { renderer = nil; broker?.clearRenderer() }
        }
    }

    /// setRendererReady. A new token from the same window is a new page (a reload): what the old
    /// one was handed fails, as a renderer navigation does in the reference.
    func setReady(_ module: AnyObject, ready: Bool, token: String) {
        queue.sync {
            let key = ObjectIdentifier(module)
            guard let window = windows[key], !window.closed else { return }
            window.ready = ready
            if !ready {
                if renderer == key { renderer = nil; window.handed = nil; broker?.clearRenderer() }
                return
            }
            if renderer != key || (window.token != "" && window.token != token) {
                if renderer != nil { broker?.clearRenderer() }
                window.handed = nil
            }
            window.token = token
            renderer = key
            broker?.registerRenderer(sender(key))
        }
    }

    /// The window's view of its handed request, and the socket's state.
    func status(for module: AnyObject) -> [String: Any] {
        queue.sync {
            let window = windows[ObjectIdentifier(module)]
            let handed = window?.handed.flatMap { id in broker?.dispatchedId == id ? id : nil } ?? ""
            return ["dispatched": handed, "ready": window?.ready == true].merging(presentationLocked()) { first, _ in first }
        }
    }

    /// The handed request, while it still waits for this window's answer.
    func handedRequest(for module: AnyObject, requestId: String) -> [String: Any]? {
        queue.sync {
            guard let window = windows[ObjectIdentifier(module)], window.handed == requestId, broker?.dispatchedId == requestId else { return nil }
            return broker?.pendingRequest(requestId)
        }
    }

    /// complete: the window's answer. Nil when taken, else why not.
    func complete(_ value: Any?) -> String? {
        guard let response = T3ActivationProtocol.response(value) else { return "The activation response is invalid." }
        queue.sync { broker?.complete(response) }
        return nil
    }

    /// The socket and its requests at a glance (the tests read it).
    func presentation() -> [String: Any] { queue.sync { presentationLocked() } }
    private func presentationLocked() -> [String: Any] {
        ["listening": server != nil, "address": server?.address ?? "", "pending": broker?.pendingCount ?? 0,
         "handed": broker?.dispatchedId ?? "", "windowReady": renderer.flatMap { windows[$0]?.ready } ?? false, "error": lastError]
    }
}

/// What `T3AppControl.activationTarget` reads of a window (NSWindow, and the tests' stand-ins).
protocol T3ActivationWindow: AnyObject {
    var isVisible: Bool { get }
    var isMiniaturized: Bool { get }
    /// A titled document window: not a panel (a popup, a menu, the snapshot flash), which never holds the session.
    var isActivationDocument: Bool { get }
}
extension NSWindow: T3ActivationWindow {
    var isActivationDocument: Bool { !(self is NSPanel) && styleMask.contains(.titled) }
}
