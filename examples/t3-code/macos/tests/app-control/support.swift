import Darwin
import Foundation
import XCTest

/// A short private temp root (a Unix socket path must stay under 104 bytes): the tests never use
/// the shared `$TMPDIR/t3code-<uid>` directory the real app and CLI share.
func shortRoot() -> String {
    let base = NSTemporaryDirectory().hasSuffix("/") ? NSTemporaryDirectory() : NSTemporaryDirectory() + "/"
    let root = base + "t3a" + String(UUID().uuidString.prefix(4)).lowercased()
    try! FileManager.default.createDirectory(atPath: root, withIntermediateDirectories: true)
    return root
}

/// makeTarget: the address for `<root>/userdata` with `<root>` as the temp directory.
func target(_ root: String) -> (address: String, directory: String) {
    T3AppControlAddress.resolve(stateDir: root + "/userdata", tempDir: root, userId: getuid())
}

func request(_ requestId: String, platform: String = "darwin") -> [String: Any] {
    ["version": 1, "requestId": requestId, "type": "open-workspace", "workspaceRoot": NSTemporaryDirectory() + "project", "platform": platform]
}
func success(_ request: [String: Any]) -> [String: Any] {
    ["version": 1, "requestId": request["requestId"] as! String, "ok": true, "projectId": "project-1", "threadId": "thread-1"]
}

/// A blocking client socket on `address` (nil when it cannot connect).
func connectClient(_ address: String) -> Int32? {
    let fd = socket(AF_UNIX, SOCK_STREAM, 0)
    var addr = sockaddr_un()
    addr.sun_family = sa_family_t(AF_UNIX); addr.sun_len = UInt8(MemoryLayout<sockaddr_un>.size)
    withUnsafeMutableBytes(of: &addr.sun_path) { $0.copyBytes(from: Array(address.utf8)) }
    let connected = withUnsafePointer(to: &addr) { $0.withMemoryRebound(to: sockaddr.self, capacity: 1) { connect(fd, $0, socklen_t(MemoryLayout<sockaddr_un>.size)) } }
    guard connected == 0 else { close(fd); return nil }
    var yes: Int32 = 1; setsockopt(fd, SOL_SOCKET, SO_NOSIGPIPE, &yes, socklen_t(MemoryLayout<Int32>.size))
    var wait = timeval(tv_sec: 5, tv_usec: 0); setsockopt(fd, SOL_SOCKET, SO_RCVTIMEO, &wait, socklen_t(MemoryLayout<timeval>.size))
    return fd
}
func send(_ fd: Int32, _ text: String) { _ = Array(text.utf8).withUnsafeBytes { write(fd, $0.baseAddress!, $0.count) } }
/// Reads one line (nil at end of stream or a 5 s silence).
func readLine(_ fd: Int32) -> String? {
    var data = Data(), byte: UInt8 = 0
    while read(fd, &byte, 1) == 1 { if byte == 0x0A { return String(decoding: data, as: UTF8.self) }; data.append(byte) }
    return data.isEmpty ? nil : String(decoding: data, as: UTF8.self)
}
/// exchange: one request line, one answer line.
func exchange(_ address: String, _ payload: [String: Any]) -> [String: Any]? {
    exchangeRaw(address, String(decoding: try! JSONSerialization.data(withJSONObject: payload), as: UTF8.self) + "\n")
}
func exchangeRaw(_ address: String, _ line: String) -> [String: Any]? {
    guard let fd = connectClient(address) else { return nil }
    defer { close(fd) }
    send(fd, line)
    return readLine(fd).flatMap { try? JSONSerialization.jsonObject(with: Data($0.utf8)) as? [String: Any] }
}

/// The server's own queue, as T3AppControl gives it one.
let serverQueue = DispatchQueue(label: "t3.app-control.tests")

func startServer(_ target: (address: String, directory: String), handle: @escaping ([String: Any], @escaping ([String: Any]) -> Void) -> Void = { request, done in done(success(request)) },
                 cancel: @escaping (String) -> Void = { _ in }) throws -> T3AppControlServer {
    try serverQueue.sync {
        try T3AppControlServer(address: target.address, directory: target.directory, userId: getuid(), queue: serverQueue, handle: handle, cancel: cancel, onReclaimError: { _ in })
    }
}
func closeServer(_ server: T3AppControlServer) { serverQueue.sync { server.close() } }
func exists(_ path: String) -> Bool { var info = stat(); return lstat(path, &info) == 0 }
func mode(_ path: String) -> mode_t { var info = stat(); _ = lstat(path, &info); return info.st_mode & 0o777 }

/// A ported test under its original name: the output says whether it passed, to read against the reference.
extension XCTestCase {
    func named(_ name: String, _ body: () throws -> Void) rethrows {
        let before = testRun?.totalFailureCount ?? 0
        try body()
        print("T3 app control: \"\(name)\" \((testRun?.totalFailureCount ?? 0) == before ? "passed" : "FAILED")")
    }
}
