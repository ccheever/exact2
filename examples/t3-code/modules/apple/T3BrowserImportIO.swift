#if os(macOS)
import AppKit
import CommonCrypto
import CryptoKit
import Darwin
import Foundation
import Security
import SQLite3

/// The cookie import's reads (browser-surface part 4; MIT reference, see LICENSE-T3, T3 Code 1e2ecbd975:
/// apps/desktop/src/preview/BrowserImport/*): the primitives the data module's port of the readers runs on
/// (browser-import-io.ts `ImportIO`): `stat` (following links), directories, files (staged, read in chunks), a read-only
/// open (TCC gates the open), `readlink`, read-only SQLite queries and `VACUUM INTO` snapshots, the Keychain's generic
/// password (in-process, so the consent prompt names this app), PBKDF2-SHA1, AES-CBC / AES-GCM, SHA-256, `kill(pid, 0)`,
/// the host name and addresses, and the fcntl lock Firefox holds on `.parentlock`.
///
/// Only the packaged build reads the user's browsers. Every other build, agent runs included, reads nothing unless
/// `T3_BROWSER_IMPORT_HOME` names a fixture home (never the account's own home): every path must resolve inside it,
/// the Keychain is never asked (the fixture's `.t3-browser-import.json` answers `keychain`), and paths it lists under
/// `tccDenied` fail an open or a read with EPERM as TCC does, so the Full Disk Access step can be shown without
/// granting anything. Full Disk Access is never requested from code: Allow opens System Settings in the packaged build
/// and is only recorded in any other.
final class T3BrowserImportIO {
    enum Policy: Equatable {
        case packaged(home: String)
        case fixture(home: String)
        case refused(String)
    }
    let policy: Policy
    private let queue = DispatchQueue(label: "t3.browser.import", qos: .userInitiated)
    private var staged: [String: Data] = [:]
    private var snapshots: Set<String> = []
    private let lock = NSLock()
    /// The fixture's answers (development builds): Keychain entries by "service/account", TCC-denied relative paths.
    private var fixture: [String: Any] {
        guard case .fixture(let home) = policy, let data = FileManager.default.contents(atPath: (home as NSString).appendingPathComponent(".t3-browser-import.json")),
              let object = try? JSONSerialization.jsonObject(with: data) as? [String: Any] else { return [:] }
        return object
    }

    init(policy: Policy) { self.policy = policy }

    /// The account's home, whatever HOME says.
    static var accountHome: String { getpwuid(getuid()).flatMap { String(validatingUTF8: $0.pointee.pw_dir) } ?? NSHomeDirectory() }

    static func resolve(env: [String: String], packaged: Bool, accountHome: String = accountHome) -> Policy {
        if packaged { return .packaged(home: accountHome) }
        guard let raw = env["T3_BROWSER_IMPORT_HOME"]?.trimmingCharacters(in: .whitespaces), !raw.isEmpty else {
            return .refused("Development build: set T3_BROWSER_IMPORT_HOME to a fixture home to list browsers.")
        }
        let home = URL(fileURLWithPath: raw).standardizedFileURL.resolvingSymlinksInPath().path
        let real = URL(fileURLWithPath: accountHome).standardizedFileURL.resolvingSymlinksInPath().path
        // The fixture may live under the account's home (a worktree's target/) but never be it or hold it.
        if home == real || real.hasPrefix(home.hasSuffix("/") ? home : home + "/") || home == "/" { return .refused("Refusing the account's own home as a cookie-import fixture.") }
        return .fixture(home: home)
    }

    var context: [String: Any] {
        switch policy {
        case .packaged(let home): return ["allowed": true, "home": home, "platform": "darwin", "fixture": false]
        case .fixture(let home): return ["allowed": true, "home": home, "platform": "darwin", "fixture": true]
        case .refused(let reason): return ["allowed": false, "reason": reason, "platform": "darwin"]
        }
    }

    // MARK: Confinement

    private var fixtureHome: String? { if case .fixture(let home) = policy { return home }; return nil }
    /// The path with its parent's links resolved (a dangling link at the end stays a link).
    private static func anchored(_ path: String) -> String {
        let url = URL(fileURLWithPath: path).standardizedFileURL
        return url.deletingLastPathComponent().resolvingSymlinksInPath().appendingPathComponent(url.lastPathComponent).path
    }
    private func inside(_ path: String, _ root: String) -> Bool { path == root || path.hasPrefix(root.hasSuffix("/") ? root : root + "/") }
    /// A development build reads only inside its fixture home (and its own snapshots), the link's target included.
    func permitted(_ path: String, followLinks: Bool = true) -> Bool {
        if case .refused = policy { return false }
        guard let home = fixtureHome else { return true }
        lock.lock(); let ownSnapshots = snapshots; lock.unlock()
        let anchored = Self.anchored(path)
        let roots = [home] + Array(ownSnapshots)
        guard roots.contains(where: { inside(anchored, $0) }) else { return false }
        if followLinks, FileManager.default.fileExists(atPath: anchored) {
            let real = URL(fileURLWithPath: anchored).resolvingSymlinksInPath().path
            return roots.contains(where: { inside(real, $0) })
        }
        return true
    }
    /// The fixture's simulated TCC refusal: an open or a read under a listed path fails with EPERM.
    private func tccDenied(_ path: String) -> Bool {
        guard let home = fixtureHome, let denied = fixture["tccDenied"] as? [String] else { return false }
        let anchored = Self.anchored(path)
        return denied.contains { inside(anchored, (home as NSString).appendingPathComponent($0)) }
    }

    static func errnoName(_ code: Int32) -> String {
        switch code {
        case ENOENT: return "ENOENT"
        case EPERM: return "EPERM"
        case EACCES: return "EACCES"
        case ENOTDIR: return "ENOTDIR"
        case EISDIR: return "EISDIR"
        case EINVAL: return "EINVAL"
        case EBUSY: return "EBUSY"
        case ELOOP: return "ELOOP"
        case ESRCH: return "ESRCH"
        default: return "E\(code)"
        }
    }
    private static func failure(_ code: Int32, _ path: String) -> [String: Any] { ["code": errnoName(code), "message": "\(errnoName(code)): \(path)"] }
    private static let outside: [String: Any] = ["code": "EACCES", "message": "Outside the cookie-import fixture home."]

    // MARK: Ops

    /// `browserImportIO`: one primitive, answered off the main thread (a Keychain prompt may wait on the user).
    func perform(_ request: [String: Any], done: @escaping ([String: Any]) -> Void) {
        queue.async { [self] in done(answer(request)) }
    }

    func answer(_ request: [String: Any]) -> [String: Any] {
        let path = request["path"] as? String ?? ""
        switch request["call"] as? String ?? "" {
        case "stat":
            guard permitted(path) else { return Self.outside }
            var info = stat()
            guard stat(path, &info) == 0 else { return Self.failure(errno, path) }
            let kind = info.st_mode & S_IFMT
            return ["kind": kind == S_IFREG ? "File" : kind == S_IFDIR ? "Directory" : "Other"]
        case "readDirectory":
            guard permitted(path) else { return Self.outside }
            guard let directory = opendir(path) else { return Self.failure(errno, path) }
            defer { closedir(directory) }
            var entries: [String] = []
            while let entry = readdir(directory) {
                let name = withUnsafeBytes(of: entry.pointee.d_name) { String(decoding: $0.prefix(Int(entry.pointee.d_namlen)), as: UTF8.self) }
                if name != "." && name != ".." { entries.append(name) }
            }
            return ["entries": entries]
        case "open":
            guard permitted(path) else { return Self.outside }
            if tccDenied(path) { return Self.failure(EPERM, path) }
            let fd = Darwin.open(path, O_RDONLY)
            guard fd >= 0 else { return Self.failure(errno, path) }
            Darwin.close(fd)
            return [:]
        case "readFile":
            guard permitted(path) else { return Self.outside }
            if tccDenied(path) { return Self.failure(EPERM, path) }
            let fd = Darwin.open(path, O_RDONLY)
            guard fd >= 0 else { return Self.failure(errno, path) }
            let handle = FileHandle(fileDescriptor: fd, closeOnDealloc: true)
            let data = handle.readDataToEndOfFile()
            let id = UUID().uuidString
            lock.lock(); staged[id] = data; lock.unlock()
            return ["id": id, "size": data.count]
        case "readChunk":
            lock.lock(); let data = staged[request["id"] as? String ?? ""]; lock.unlock()
            guard let data else { return ["code": "ENOENT", "message": "The staged file was released."] }
            let offset = max(0, request["offset"] as? Int ?? 0), length = max(0, request["length"] as? Int ?? 0)
            let end = min(data.count, offset + length)
            return ["data": offset < end ? data.subdata(in: offset..<end).base64EncodedString() : ""]
        case "release":
            lock.lock(); defer { lock.unlock() }
            if let id = request["id"] as? String { staged.removeValue(forKey: id) }
            if !path.isEmpty, let directory = snapshots.first(where: { inside(path, $0) }) {
                snapshots.remove(directory)
                try? FileManager.default.removeItem(atPath: directory)
            }
            return [:]
        case "readLink":
            guard permitted(path, followLinks: false) else { return Self.outside }
            var buffer = [CChar](repeating: 0, count: Int(PATH_MAX) + 1)
            let count = readlink(path, &buffer, Int(PATH_MAX))
            guard count >= 0 else { return Self.failure(errno, path) }
            return ["target": String(decoding: buffer.prefix(count).map { UInt8(bitPattern: $0) }, as: UTF8.self)]
        case "query":
            guard permitted(path) else { return Self.outside }
            return Self.query(path, request["sql"] as? String ?? "", request["params"] as? [Any] ?? [])
        case "snapshot":
            guard permitted(path) else { return Self.outside }
            return snapshot(path)
        case "keychain":
            return keychain(service: request["service"] as? String ?? "", account: request["account"] as? String ?? "")
        case "pbkdf2":
            let length = max(1, min(64, request["length"] as? Int ?? 16))
            return ["key": Self.pbkdf2(request["passphrase"] as? String ?? "", salt: request["salt"] as? String ?? "", iterations: max(1, request["iterations"] as? Int ?? 1), length: length).base64EncodedString()]
        case "decrypt":
            return ["results": (request["items"] as? [[String: Any]] ?? []).map { Self.decrypt($0)?.base64EncodedString() ?? NSNull() as Any }]
        case "sha256":
            return ["results": (request["items"] as? [String] ?? []).map { Data(SHA256.hash(data: Data(base64Encoded: $0) ?? Data())).base64EncodedString() }]
        case "signal0":
            let pid = pid_t(request["pid"] as? Int ?? 0)
            guard pid > 0 else { return Self.failure(ESRCH, "pid") }
            return kill(pid, 0) == 0 ? [:] : Self.failure(errno, "pid \(pid)")
        case "hostname":
            var buffer = [CChar](repeating: 0, count: 256)
            return ["hostname": gethostname(&buffer, buffer.count) == 0 ? String(cString: buffer) : ""]
        case "localAddresses":
            return ["addresses": Self.localAddresses()]
        case "lockHeld":
            guard permitted(path) else { return Self.outside }
            return ["held": Self.lockHeld(path)]
        default:
            return ["code": "EINVAL", "message": "Unknown import call."]
        }
    }

    // MARK: SQLite (read-only)

    static func query(_ path: String, _ sql: String, _ params: [Any]) -> [String: Any] {
        var db: OpaquePointer?
        defer { sqlite3_close(db) }
        guard sqlite3_open_v2(path, &db, SQLITE_OPEN_READONLY, nil) == SQLITE_OK else { return sqlFailure(db, "open") }
        var statement: OpaquePointer?
        defer { sqlite3_finalize(statement) }
        guard sqlite3_prepare_v2(db, sql, -1, &statement, nil) == SQLITE_OK else { return sqlFailure(db, "prepare") }
        bind(statement, params)
        var rows: [[String: Any]] = []
        while true {
            let step = sqlite3_step(statement)
            if step == SQLITE_DONE { break }
            guard step == SQLITE_ROW else { return sqlFailure(db, "step") }
            var row: [String: Any] = [:]
            for column in 0..<sqlite3_column_count(statement) {
                let name = String(cString: sqlite3_column_name(statement, column))
                switch sqlite3_column_type(statement, column) {
                case SQLITE_INTEGER: row[name] = sqlite3_column_int64(statement, column)
                case SQLITE_FLOAT: row[name] = sqlite3_column_double(statement, column)
                case SQLITE_TEXT: row[name] = String(cString: sqlite3_column_text(statement, column))
                case SQLITE_BLOB:
                    let count = Int(sqlite3_column_bytes(statement, column))
                    let bytes = count > 0 ? Data(bytes: sqlite3_column_blob(statement, column), count: count) : Data()
                    row[name] = ["$b": bytes.base64EncodedString()]
                default: row[name] = NSNull()
                }
            }
            rows.append(row)
        }
        return ["rows": rows]
    }
    private static let transient = unsafeBitCast(-1, to: sqlite3_destructor_type.self)
    private static func bind(_ statement: OpaquePointer?, _ params: [Any]) {
        for (index, value) in params.enumerated() {
            let at = Int32(index + 1)
            if let blob = value as? [String: Any], let base64 = blob["$b"] as? String, let data = Data(base64Encoded: base64) {
                _ = data.withUnsafeBytes { sqlite3_bind_blob(statement, at, $0.baseAddress, Int32(data.count), transient) }
            } else if let text = value as? String { sqlite3_bind_text(statement, at, text, -1, transient) }
            else if let number = value as? NSNumber { if CFNumberIsFloatType(number) { sqlite3_bind_double(statement, at, number.doubleValue) } else { sqlite3_bind_int64(statement, at, number.int64Value) } }
            else { sqlite3_bind_null(statement, at) }
        }
    }
    private static func sqlFailure(_ db: OpaquePointer?, _ stage: String) -> [String: Any] {
        ["code": "SQLITE", "kind": "sql", "message": "\(stage): \(db.map { String(cString: sqlite3_errmsg($0)) } ?? "unable to open the database")"]
    }

    /// CookieDatabase.snapshotCookieDatabase: `VACUUM INTO` a fresh temporary directory, read-only on the source, so a
    /// live database (its WAL included) is read consistently and the browser's own file is never opened for writing.
    func snapshot(_ path: String) -> [String: Any] {
        var template = Array((NSTemporaryDirectory() as NSString).appendingPathComponent("t3code-cookie-import-XXXXXX").utf8CString)
        guard let made = mkdtemp(&template) else { return Self.failure(errno, "temporary directory") }
        let directory = URL(fileURLWithPath: String(cString: made)).resolvingSymlinksInPath().path
        let target = (directory as NSString).appendingPathComponent((path as NSString).lastPathComponent)
        var db: OpaquePointer?
        defer { sqlite3_close(db) }
        var statement: OpaquePointer?
        defer { sqlite3_finalize(statement) }
        guard sqlite3_open_v2(path, &db, SQLITE_OPEN_READONLY, nil) == SQLITE_OK, sqlite3_prepare_v2(db, "VACUUM INTO ?", -1, &statement, nil) == SQLITE_OK,
              sqlite3_bind_text(statement, 1, target, -1, Self.transient) == SQLITE_OK, sqlite3_step(statement) == SQLITE_DONE else {
            let failure = Self.sqlFailure(db, "snapshot")
            try? FileManager.default.removeItem(atPath: directory)
            return failure
        }
        lock.lock(); snapshots.insert(directory); lock.unlock()
        return ["path": target]
    }

    // MARK: Keychain

    /// The generic password, or a refusal the reader maps (missing → keychainItemMissing; any other status →
    /// needsKeychainApproval). A development build answers from the fixture and never asks the Keychain.
    func keychain(service: String, account: String) -> [String: Any] {
        switch policy {
        case .refused(let reason): return ["code": "KEYCHAIN", "kind": "keychainUnavailable", "message": reason]
        case .fixture:
            let entry = (fixture["keychain"] as? [String: Any])?["\(service)/\(account)"]
            if let secret = entry as? String { return ["secret": secret] }
            if let refusal = entry as? [String: Any] { return ["code": "KEYCHAIN", "kind": refusal["unavailable"] as? Bool == true ? "keychainUnavailable" : "keychain", "message": refusal["message"] as? String ?? "User denied access"] }
            return ["secret": NSNull()]
        case .packaged:
            let query: [String: Any] = [kSecClass as String: kSecClassGenericPassword, kSecAttrService as String: service, kSecAttrAccount as String: account,
                                        kSecReturnData as String: true, kSecMatchLimit as String: kSecMatchLimitOne]
            var item: CFTypeRef?
            let status = SecItemCopyMatching(query as CFDictionary, &item)
            return Self.keychainAnswer(status: status, data: item as? Data)
        }
    }
    static func keychainAnswer(status: OSStatus, data: Data?) -> [String: Any] {
        if status == errSecSuccess { return ["secret": data.map { String(decoding: $0, as: UTF8.self) } ?? NSNull()] }
        if status == errSecItemNotFound { return ["secret": NSNull()] }
        if status == errSecNotAvailable || status == errSecNoSuchKeychain { return ["code": "KEYCHAIN", "kind": "keychainUnavailable", "message": "The Keychain is unavailable (\(status))."] }
        let message = SecCopyErrorMessageString(status, nil) as String? ?? "Keychain error \(status)"
        return ["code": "KEYCHAIN", "kind": "keychain", "message": message]
    }

    // MARK: Crypto

    static func pbkdf2(_ passphrase: String, salt: String, iterations: Int, length: Int) -> Data {
        var derived = Data(count: length)
        let password = Array(passphrase.utf8).map { Int8(bitPattern: $0) }, saltBytes = Array(salt.utf8)
        _ = derived.withUnsafeMutableBytes { out in
            CCKeyDerivationPBKDF(CCPBKDFAlgorithm(kCCPBKDF2), password, password.count, saltBytes, saltBytes.count, CCPseudoRandomAlgorithm(kCCPRFHmacAlgSHA1),
                                 UInt32(iterations), out.bindMemory(to: UInt8.self).baseAddress, length)
        }
        return derived
    }
    static func decrypt(_ item: [String: Any]) -> Data? {
        func bytes(_ key: String) -> Data? { (item[key] as? String).flatMap { Data(base64Encoded: $0) } }
        guard let key = bytes("key"), let data = bytes("data") else { return nil }
        if item["mode"] as? String == "gcm" {
            guard let nonce = bytes("nonce"), let tag = bytes("tag"), let gcmNonce = try? AES.GCM.Nonce(data: nonce),
                  let box = try? AES.GCM.SealedBox(nonce: gcmNonce, ciphertext: data, tag: tag) else { return nil }
            return try? AES.GCM.open(box, using: SymmetricKey(data: key))
        }
        guard let iv = bytes("iv"), [16, 24, 32].contains(key.count), iv.count == kCCBlockSizeAES128, data.count % kCCBlockSizeAES128 == 0, !data.isEmpty else { return nil }
        var out = Data(count: data.count + kCCBlockSizeAES128)
        var written = 0
        let status = out.withUnsafeMutableBytes { output in data.withUnsafeBytes { input in key.withUnsafeBytes { keyBytes in iv.withUnsafeBytes { ivBytes in
            CCCrypt(CCOperation(kCCDecrypt), CCAlgorithm(kCCAlgorithmAES), CCOptions(kCCOptionPKCS7Padding), keyBytes.baseAddress, key.count, ivBytes.baseAddress,
                    input.baseAddress, data.count, output.baseAddress, output.count, &written)
        } } } }
        guard status == kCCSuccess else { return nil }
        return out.prefix(written)
    }

    // MARK: Hosts and locks

    static func localAddresses() -> [String] {
        var addresses: Set<String> = ["127.0.0.1"]
        var list: UnsafeMutablePointer<ifaddrs>?
        guard getifaddrs(&list) == 0, let first = list else { return Array(addresses) }
        defer { freeifaddrs(list) }
        for entry in sequence(first: first, next: { $0.pointee.ifa_next }) {
            guard let address = entry.pointee.ifa_addr, address.pointee.sa_family == UInt8(AF_INET) || address.pointee.sa_family == UInt8(AF_INET6) else { continue }
            var host = [CChar](repeating: 0, count: Int(NI_MAXHOST))
            let length = socklen_t(address.pointee.sa_family == UInt8(AF_INET) ? MemoryLayout<sockaddr_in>.size : MemoryLayout<sockaddr_in6>.size)
            if getnameinfo(address, length, &host, socklen_t(host.count), nil, 0, NI_NUMERICHOST) == 0 { addresses.insert(String(cString: host)) }
        }
        return addresses.sorted()
    }
    /// Whether another process holds a POSIX write lock on the file (`F_GETLK`; the reference asks python3's fcntl).
    static func lockHeld(_ path: String) -> Bool {
        let fd = Darwin.open(path, O_RDONLY)
        guard fd >= 0 else { return false }
        defer { Darwin.close(fd) }
        var probe = flock(l_start: 0, l_len: 0, l_pid: 0, l_type: Int16(F_WRLCK), l_whence: Int16(SEEK_SET))
        guard fcntl(fd, F_GETLK, &probe) == 0 else { return false }
        return probe.l_type != Int16(F_UNLCK)
    }

    // MARK: Full Disk Access

    /// PermissionChecklist's Allow: the packaged build opens System Settings at Full Disk Access; any other build records it.
    func openFullDiskAccessSettings() -> [String: Any] {
        guard case .packaged = policy else {
            FileHandle.standardError.write(Data("t3.browser: full-disk-access settings recorded (not opened outside the packaged build)\n".utf8))
            return ["opened": true, "recorded": true]
        }
        guard let url = URL(string: "x-apple.systempreferences:com.apple.preference.security?Privacy_AllFiles") else { return ["opened": false] }
        return ["opened": NSWorkspace.shared.open(url)]
    }
}
#endif
