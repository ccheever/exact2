import AppKit
import CommonCrypto
import CryptoKit
import Network
import SQLite3
import WebKit
import XCTest

// browser-surface part 4 (profiles): the profile stores, Clear cookies / Clear cache, the cookie write into a profile's
// store (T3BrowserSessions+Profiles.swift), and the import's module primitives (T3BrowserImportIO.swift): the
// development build's fixture home, its confinement and TCC stand-in, the snapshot, SQLite, crypto and fcntl answers.
// Ported reference tests (T3 Code 1e2ecbd975, MIT, see LICENSE-T3), under their own names:
// BrowserImport/CookieDatabase.test.ts (3: the snapshot is the module's here) and Sources.test.ts's "detects a live fcntl
// lock on .parentlock, as macOS Firefox leaves it" (the module asks fcntl where the reference spawns python3).
final class Fixture {
    private let listener: NWListener
    private(set) var cookies: [String] = []
    private(set) var port: UInt16 = 0
    private let queue = DispatchQueue(label: "profiles-fixture")
    init() throws {
        listener = try NWListener(using: .tcp, on: .any)
        let ready = DispatchSemaphore(value: 0)
        listener.stateUpdateHandler = { if case .ready = $0 { ready.signal() } }
        listener.newConnectionHandler = { [weak self] connection in self?.serve(connection) }
        listener.start(queue: queue)
        _ = ready.wait(timeout: .now() + 5)
        port = listener.port?.rawValue ?? 0
    }
    var base: String { "http://127.0.0.1:\(port)" }
    var seen: [String] { queue.sync { cookies } }
    private func serve(_ connection: NWConnection) {
        connection.start(queue: queue)
        connection.receive(minimumIncompleteLength: 1, maximumLength: 65_536) { [weak self] data, _, _, _ in
            guard let self, let data, let text = String(data: data, encoding: .utf8) else { return connection.cancel() }
            let cookie = text.split(separator: "\r\n").first { $0.lowercased().hasPrefix("cookie:") }.map { String($0.dropFirst(7)).trimmingCharacters(in: .whitespaces) } ?? ""
            self.cookies.append(cookie)
            let body = Data("<title>Profile page</title><p id=c></p><script>document.getElementById('c').textContent=document.cookie</script>".utf8)
            var response = Data("HTTP/1.1 200 OK\r\nContent-Type: text/html\r\nContent-Length: \(body.count)\r\nCache-Control: no-store\r\nConnection: close\r\n\r\n".utf8)
            response.append(body)
            connection.send(content: response, completion: .contentProcessed { _ in connection.cancel() })
        }
    }
    deinit { listener.cancel() }
}

func spin(until condition: () -> Bool, timeout: TimeInterval = 10) {
    let end = Date().addingTimeInterval(timeout)
    while !condition() && Date() < end { RunLoop.main.run(until: Date().addingTimeInterval(0.02)) }
}
func temporaryDirectory(_ prefix: String) -> String {
    var template = Array((NSTemporaryDirectory() as NSString).appendingPathComponent("\(prefix)XXXXXX").utf8CString)
    return URL(fileURLWithPath: String(cString: mkdtemp(&template)!)).resolvingSymlinksInPath().path
}
func exec(_ path: String, _ statements: [String], keepOpen: Bool = false) -> OpaquePointer? {
    var db: OpaquePointer?
    sqlite3_open(path, &db)
    for statement in statements { sqlite3_exec(db, statement, nil, nil, nil) }
    if keepOpen { return db }
    sqlite3_close(db)
    return nil
}

final class ProfileStoreTests: XCTestCase {
    private var sessions: T3BrowserSessions!
    private var fixture: Fixture!
    override func setUpWithError() throws { sessions = T3BrowserSessions(agent: true, changed: { _ in }); fixture = try Fixture() }
    override func tearDown() { sessions.sync([]) }

    private func cookies(_ environment: String, _ profile: String) -> [HTTPCookie] {
        var found: [HTTPCookie]?
        sessions.store(environment: environment, profile: profile).httpCookieStore.getAllCookies { found = $0 }
        spin(until: { found != nil })
        return found ?? []
    }
    private func write(_ environment: String, _ profile: String, _ cookies: [[String: Any]]) -> [Bool] {
        var results: [Bool]?
        sessions.importCookies(environment: environment, profile: profile, cookies: cookies) { results = $0 }
        spin(until: { results != nil })
        return results ?? []
    }
    private func clear(_ environment: String, _ profile: String, _ what: String) -> Bool {
        var cleared: Bool?
        sessions.clearData(environment: environment, profile: profile, what: what) { cleared = $0 }
        spin(until: { cleared != nil })
        return cleared ?? false
    }
    private let session = ["url": "http://127.0.0.1/", "name": "sid", "value": "v", "path": "/", "secure": false, "httpOnly": false, "sameSite": "lax"] as [String: Any]

    func testIncognitoIsInMemoryAndNamedProfilesPersistInTheirOwnStore() {
        let persistent = T3BrowserSessions(agent: false, changed: { _ in })
        XCTAssertFalse(persistent.store(environment: "env-test", profile: "incognito").isPersistent, "Incognito keeps nothing on disk")
        XCTAssertTrue(persistent.store(environment: "env-test", profile: "profile-work").isPersistent)
        XCTAssertTrue(persistent.store(environment: "env-test", profile: "incognito") === persistent.store(environment: "env-test", profile: "incognito"), "one Incognito store per environment while the app runs")
        XCTAssertFalse(persistent.store(environment: "env-test", profile: "incognito") === persistent.store(environment: "env-other", profile: "incognito"))
        XCTAssertNotEqual(T3BrowserSessions.storeIdentifier(environment: "env-test", profile: "profile-work"), T3BrowserSessions.storeIdentifier(environment: "env-test", profile: "default"))
        let done = expectation(description: "removed")
        WKWebsiteDataStore.remove(forIdentifier: T3BrowserSessions.storeIdentifier(environment: "env-test", profile: "profile-work")) { _ in done.fulfill() }
        wait(for: [done], timeout: 10)
    }

    func testImportedCookiesKeepHostOnlyAndDomainScopesAndRefusedOnesAreSkipped() {
        let results = write("env-1", "profile-work", [
            ["url": "https://example.test/app", "name": "host", "value": "1", "path": "/app", "secure": true, "httpOnly": true, "expirationDate": Date().timeIntervalSince1970 + 3_600, "sameSite": "strict"],
            ["url": "https://example.test/", "name": "domain", "value": "2", "domain": ".example.test", "path": "/", "secure": true, "httpOnly": false, "sameSite": "no_restriction"],
            ["url": "https://example.test/", "name": "expired", "value": "3", "path": "/", "expirationDate": 1_000_000.0, "sameSite": "lax"],
            ["url": "not a url", "name": "broken", "value": "4", "path": "/"],
        ])
        XCTAssertEqual(results, [true, true, false, false], "an expired or malformed cookie is skipped, as a refused cookies.set")
        let stored = Dictionary(uniqueKeysWithValues: cookies("env-1", "profile-work").map { ($0.name, $0) })
        XCTAssertEqual(stored["host"]?.domain, "example.test", "host-only: no leading dot, never widened to subdomains")
        XCTAssertEqual(stored["host"]?.isSecure, true)
        XCTAssertEqual(stored["host"]?.isHTTPOnly, true)
        XCTAssertEqual(stored["host"]?.sameSitePolicy, .sameSiteStrict)
        XCTAssertNotNil(stored["host"]?.expiresDate)
        XCTAssertEqual(stored["domain"]?.domain, ".example.test")
        XCTAssertTrue(stored["domain"]?.isSessionOnly == true)
        XCTAssertTrue(cookies("env-1", "default").isEmpty, "only the target profile's store")
    }

    func testClearCookiesAndClearCacheActOnTheirEnvironmentsProfileOnly() {
        for (environment, profile) in [("env-1", "profile-work"), ("env-1", "default"), ("env-2", "profile-work")] { XCTAssertEqual(write(environment, profile, [session]), [true]) }
        XCTAssertTrue(clear("env-1", "profile-work", "cache"))
        XCTAssertEqual(cookies("env-1", "profile-work").count, 1, "Clear cache leaves the cookies")
        XCTAssertTrue(clear("env-1", "profile-work", "cookies"))
        XCTAssertTrue(cookies("env-1", "profile-work").isEmpty)
        XCTAssertEqual(cookies("env-1", "default").count, 1, "another profile keeps its cookies")
        XCTAssertEqual(cookies("env-2", "profile-work").count, 1, "the same profile in another environment keeps its cookies")
        XCTAssertFalse(clear("env-1", "profile-work", "everything"))
        XCTAssertEqual(T3BrowserSessions.dataTypes("cookies"), [WKWebsiteDataTypeCookies, WKWebsiteDataTypeLocalStorage, WKWebsiteDataTypeIndexedDBDatabases, WKWebsiteDataTypeServiceWorkerRegistrations])
        XCTAssertEqual(T3BrowserSessions.dataTypes("cache"), [WKWebsiteDataTypeDiskCache, WKWebsiteDataTypeMemoryCache, WKWebsiteDataTypeFetchCache])
    }

    func testATabSeesTheCookiesImportedIntoItsProfileAndNoOther() throws {
        XCTAssertEqual(write("env-1", "profile-work", [["url": "\(fixture.base)/", "name": "imported", "value": "yes", "path": "/", "sameSite": "lax"]]), [true])
        sessions.sync([["id": "work-tab", "url": "\(fixture.base)/", "profile": "profile-work", "environment": "env-1"], ["id": "default-tab", "url": "\(fixture.base)/", "profile": "default", "environment": "env-1"]])
        spin(until: { self.fixture.seen.count >= 2 && self.sessions.sessions.values.allSatisfy { $0.navigation.kind == "Success" } })
        XCTAssertEqual(Set(fixture.seen), ["imported=yes", ""], "the request from the work tab carries the cookie; the default tab's does not")
        var text: String?
        sessions.sessions["work-tab"]?.web.evaluateJavaScript("document.cookie") { value, _ in text = value as? String }
        spin(until: { text != nil })
        XCTAssertEqual(text, "imported=yes")
    }

    func testTheModuleOpsAnswerThroughTheReply() {
        XCTAssertEqual(T3BrowserSessions.httpCookie(["url": "https://example.test/", "name": "n", "value": "v", "path": "/", "sameSite": "unspecified"])?.sameSitePolicy, nil, "WebKit's default (None) for an unspecified SameSite")
        XCTAssertNil(T3BrowserSessions.httpCookie(["url": "https://example.test/", "name": "", "value": "v"]))
    }
}

final class ImportIOTests: XCTestCase {
    private var home: String!
    private var io: T3BrowserImportIO!
    override func setUp() { home = temporaryDirectory("t3-import-fixture-"); io = T3BrowserImportIO(policy: .fixture(home: home)) }
    override func tearDown() { try? FileManager.default.removeItem(atPath: home) }
    private func call(_ request: [String: Any]) -> [String: Any] { io.answer(request) }

    func testADevelopmentBuildReadsOnlyAFixtureHome() {
        XCTAssertEqual(T3BrowserImportIO.resolve(env: [:], packaged: false), .refused("Development build: set T3_BROWSER_IMPORT_HOME to a fixture home to list browsers."))
        XCTAssertEqual(T3BrowserImportIO.resolve(env: ["T3_BROWSER_IMPORT_HOME": "/Users/someone"], packaged: false, accountHome: "/Users/someone"), .refused("Refusing the account's own home as a cookie-import fixture."))
        XCTAssertEqual(T3BrowserImportIO.resolve(env: ["T3_BROWSER_IMPORT_HOME": "/"], packaged: false, accountHome: "/Users/someone"), .refused("Refusing the account's own home as a cookie-import fixture."))
        XCTAssertEqual(T3BrowserImportIO.resolve(env: ["T3_BROWSER_IMPORT_HOME": home], packaged: false), .fixture(home: home))
        XCTAssertEqual(T3BrowserImportIO.resolve(env: ["T3_BROWSER_IMPORT_HOME": home], packaged: true, accountHome: "/Users/someone"), .packaged(home: "/Users/someone"), "only the packaged build reads the user's browsers")
        XCTAssertEqual(T3BrowserImportIO(policy: .refused("no")).answer(["call": "stat", "path": home!])["code"] as? String, "EACCES")
        XCTAssertEqual(T3BrowserImportIO(policy: .refused("no")).context["allowed"] as? Bool, false)
    }

    func testEveryPathMustResolveInsideTheFixtureHome() throws {
        let outside = temporaryDirectory("t3-import-outside-")
        defer { try? FileManager.default.removeItem(atPath: outside) }
        FileManager.default.createFile(atPath: "\(outside)/Cookies", contents: Data("db".utf8))
        FileManager.default.createFile(atPath: "\(home!)/Cookies", contents: Data("db".utf8))
        try FileManager.default.createSymbolicLink(atPath: "\(home!)/escape", withDestinationPath: "\(outside)/Cookies")
        try FileManager.default.createSymbolicLink(atPath: "\(home!)/SingletonLock", withDestinationPath: "host-1234")
        XCTAssertEqual(call(["call": "stat", "path": "\(home!)/Cookies"])["kind"] as? String, "File")
        XCTAssertEqual(call(["call": "stat", "path": "\(outside)/Cookies"])["message"] as? String, "Outside the cookie-import fixture home.")
        XCTAssertEqual(call(["call": "stat", "path": "\(home!)/../\((outside as NSString).lastPathComponent)/Cookies"])["code"] as? String, "EACCES")
        XCTAssertEqual(call(["call": "readFile", "path": "\(home!)/escape"])["code"] as? String, "EACCES", "a link out of the fixture is refused")
        XCTAssertEqual(call(["call": "readLink", "path": "\(home!)/SingletonLock"])["target"] as? String, "host-1234", "a dangling lock link reads")
        XCTAssertEqual(call(["call": "readLink", "path": "\(home!)/Cookies"])["code"] as? String, "EINVAL")
        XCTAssertEqual(call(["call": "stat", "path": "\(home!)/missing"])["code"] as? String, "ENOENT")
    }

    func testTheFixturesTccStandInRefusesTheOpenButNotTheStat() throws {
        let jar = "\(home!)/Library/Containers/com.apple.Safari/Data/Library/Cookies"
        try FileManager.default.createDirectory(atPath: jar, withIntermediateDirectories: true)
        FileManager.default.createFile(atPath: "\(jar)/Cookies.binarycookies", contents: Data("cook".utf8))
        try Data(#"{"tccDenied":["Library/Containers/com.apple.Safari"]}"#.utf8).write(to: URL(fileURLWithPath: "\(home!)/.t3-browser-import.json"))
        XCTAssertEqual(call(["call": "stat", "path": "\(jar)/Cookies.binarycookies"])["kind"] as? String, "File", "stat sees the jar, as TCC permits")
        XCTAssertEqual(call(["call": "open", "path": "\(jar)/Cookies.binarycookies"])["code"] as? String, "EPERM")
        XCTAssertEqual(call(["call": "readFile", "path": "\(jar)/Cookies.binarycookies"])["code"] as? String, "EPERM")
        try FileManager.default.removeItem(atPath: "\(home!)/.t3-browser-import.json")
        let staged = call(["call": "readFile", "path": "\(jar)/Cookies.binarycookies"])
        XCTAssertEqual(staged["size"] as? Int, 4)
        XCTAssertEqual(call(["call": "readChunk", "id": staged["id"]!, "offset": 1, "length": 2])["data"] as? String, Data("oo".utf8).base64EncodedString())
        _ = call(["call": "release", "id": staged["id"]!])
        XCTAssertEqual(call(["call": "readChunk", "id": staged["id"]!, "offset": 0, "length": 4])["code"] as? String, "ENOENT")
    }

    func testTheKeychainIsAnsweredByTheFixtureAndStatusesMapToTheReadersReasons() throws {
        try Data(#"{"keychain":{"Chrome Safe Storage/Chrome":"fixture-secret","Arc Safe Storage/Arc":{"message":"User denied access"},"Brave Safe Storage/Brave":{"unavailable":true}}}"#.utf8).write(to: URL(fileURLWithPath: "\(home!)/.t3-browser-import.json"))
        XCTAssertEqual(call(["call": "keychain", "service": "Chrome Safe Storage", "account": "Chrome"])["secret"] as? String, "fixture-secret")
        XCTAssertEqual(call(["call": "keychain", "service": "Arc Safe Storage", "account": "Arc"])["kind"] as? String, "keychain")
        XCTAssertEqual(call(["call": "keychain", "service": "Brave Safe Storage", "account": "Brave"])["kind"] as? String, "keychainUnavailable")
        XCTAssertTrue(call(["call": "keychain", "service": "Edge", "account": "Edge"])["secret"] is NSNull, "missing")
        XCTAssertEqual(T3BrowserImportIO.keychainAnswer(status: errSecSuccess, data: Data("s".utf8))["secret"] as? String, "s")
        XCTAssertTrue(T3BrowserImportIO.keychainAnswer(status: errSecItemNotFound, data: nil)["secret"] is NSNull)
        XCTAssertEqual(T3BrowserImportIO.keychainAnswer(status: errSecUserCanceled, data: nil)["kind"] as? String, "keychain")
        XCTAssertEqual(T3BrowserImportIO.keychainAnswer(status: errSecNotAvailable, data: nil)["kind"] as? String, "keychainUnavailable")
    }

    func testCryptoMatchesOSCrypt() throws {
        XCTAssertEqual(T3BrowserImportIO.pbkdf2("macos-secret", salt: "saltysalt", iterations: 1003, length: 16).map { String(format: "%02x", $0) }.joined(), "3df7306fb1eac353289565a2f6b64f74")
        let key = Data("0123456789abcdef".utf8), iv = Data(repeating: 0x20, count: 16), plain = Data("stored encrypted".utf8)
        var cipher = Data(count: plain.count + 16), written = 0
        _ = cipher.withUnsafeMutableBytes { out in plain.withUnsafeBytes { input in key.withUnsafeBytes { k in iv.withUnsafeBytes { v in
            CCCrypt(CCOperation(kCCEncrypt), CCAlgorithm(kCCAlgorithmAES), CCOptions(kCCOptionPKCS7Padding), k.baseAddress, 16, v.baseAddress, input.baseAddress, plain.count, out.baseAddress, out.count, &written)
        } } } }
        cipher = cipher.prefix(written)
        XCTAssertEqual(T3BrowserImportIO.decrypt(["mode": "cbc", "key": key.base64EncodedString(), "iv": iv.base64EncodedString(), "data": cipher.base64EncodedString()]), plain)
        XCTAssertNil(T3BrowserImportIO.decrypt(["mode": "cbc", "key": Data("fedcba9876543210".utf8).base64EncodedString(), "iv": iv.base64EncodedString(), "data": cipher.base64EncodedString()]).flatMap { $0 == plain ? $0 : nil })
        let gcmKey = SymmetricKey(data: Data("0123456789abcdef0123456789abcdef".utf8)), nonce = try AES.GCM.Nonce(data: Data("0123456789ab".utf8))
        let sealed = try AES.GCM.seal(Data("windows value".utf8), using: gcmKey, nonce: nonce)
        let item: [String: Any] = ["mode": "gcm", "key": Data("0123456789abcdef0123456789abcdef".utf8).base64EncodedString(), "nonce": Data("0123456789ab".utf8).base64EncodedString(),
                                   "data": sealed.ciphertext.base64EncodedString(), "tag": sealed.tag.base64EncodedString()]
        XCTAssertEqual(T3BrowserImportIO.decrypt(item), Data("windows value".utf8))
        var tampered = item; tampered["tag"] = Data(repeating: 1, count: 16).base64EncodedString()
        XCTAssertNil(T3BrowserImportIO.decrypt(tampered))
        let digest = call(["call": "sha256", "items": [Data("a".utf8).base64EncodedString()]])["results"] as? [String]
        XCTAssertEqual(digest, [Data(SHA256.hash(data: Data("a".utf8))).base64EncodedString()])
    }

    func testQueriesAreReadOnlyAndTypedAsTheReadersDecodeThem() {
        let db = "\(home!)/Cookies"
        _ = exec(db, ["create table cookies (host_key text, value text, encrypted_value blob, expires_utc integer, is_secure integer, top text)",
                      "insert into cookies values ('a.test', 'v', x'763130', 13300000000000000, 1, null)"])
        let rows = call(["call": "query", "path": db, "sql": "select host_key, value, encrypted_value, expires_utc / 1000000 as expires_seconds, is_secure, top from cookies where host_key = ?", "params": ["a.test"]])["rows"] as? [[String: Any]]
        XCTAssertEqual(rows?.count, 1)
        XCTAssertEqual(rows?.first?["host_key"] as? String, "a.test")
        XCTAssertEqual((rows?.first?["encrypted_value"] as? [String: Any])?["$b"] as? String, Data("v10".utf8).base64EncodedString())
        XCTAssertEqual(rows?.first?["expires_seconds"] as? Int64, 13_300_000_000)
        XCTAssertTrue(rows?.first?["top"] is NSNull)
        XCTAssertEqual(call(["call": "query", "path": db, "sql": "delete from cookies"])["kind"] as? String, "sql", "the database is opened read-only")
        FileManager.default.createFile(atPath: "\(home!)/NotADatabase", contents: Data("not a sqlite database".utf8))
        XCTAssertEqual(call(["call": "query", "path": "\(home!)/NotADatabase", "sql": "select 1"])["kind"] as? String, "sql")
    }

    // CookieDatabase.test.ts snapshotCookieDatabase
    func testIncludesCommittedWALDataInOneConsistentDatabase() {
        let source = "\(home!)/Cookies"
        let open = exec(source, ["PRAGMA journal_mode = WAL", "PRAGMA wal_autocheckpoint = 0", "CREATE TABLE cookies(name TEXT NOT NULL)", "INSERT INTO cookies(name) VALUES ('committed-in-wal')"], keepOpen: true)
        defer { sqlite3_close(open) }
        XCTAssertTrue(FileManager.default.fileExists(atPath: "\(source)-wal"))
        let snapshot = call(["call": "snapshot", "path": source])["path"] as? String ?? ""
        let rows = call(["call": "query", "path": snapshot, "sql": "SELECT name FROM cookies"])["rows"] as? [[String: Any]]
        XCTAssertEqual(rows?.compactMap { $0["name"] as? String }, ["committed-in-wal"])
        _ = call(["call": "release", "path": snapshot])
    }

    func testPropagatesSnapshotFailuresAndRemovesItsTemporaryDirectory() throws {
        let source = "\(home!)/Cookies"
        FileManager.default.createFile(atPath: source, contents: Data("not a sqlite database".utf8))
        let before = try FileManager.default.contentsOfDirectory(atPath: NSTemporaryDirectory()).filter { $0.hasPrefix("t3code-cookie-import-") }
        XCTAssertEqual(call(["call": "snapshot", "path": source])["kind"] as? String, "sql")
        let after = try FileManager.default.contentsOfDirectory(atPath: NSTemporaryDirectory()).filter { $0.hasPrefix("t3code-cookie-import-") }
        XCTAssertEqual(Set(after), Set(before))
    }

    func testRemovesASuccessfulSnapshotWhenItsScopeCloses() {
        let source = "\(home!)/Cookies"
        _ = exec(source, ["CREATE TABLE cookies(name TEXT NOT NULL)"])
        let snapshot = call(["call": "snapshot", "path": source])["path"] as? String ?? ""
        XCTAssertTrue(FileManager.default.fileExists(atPath: snapshot))
        _ = call(["call": "release", "path": snapshot])
        XCTAssertFalse(FileManager.default.fileExists(atPath: snapshot))
        XCTAssertEqual(call(["call": "query", "path": snapshot, "sql": "select 1"])["code"] as? String, "EACCES", "a released snapshot is outside the fixture again")
    }

    // Sources.test.ts "detects a live fcntl lock on .parentlock, as macOS Firefox leaves it"
    func testDetectsALiveFcntlLockOnParentlockAsMacOSFirefoxLeavesIt() throws {
        let lock = "\(home!)/.parentlock"
        FileManager.default.createFile(atPath: lock, contents: Data())
        XCTAssertEqual(call(["call": "lockHeld", "path": lock])["held"] as? Bool, false, "a lock left on disk after a clean exit is not held")
        let holder = Process()
        holder.executableURL = URL(fileURLWithPath: "/usr/bin/python3")
        holder.arguments = ["-c", "import fcntl,os,sys,time\nfd=os.open(sys.argv[1],os.O_WRONLY)\nfcntl.lockf(fd,fcntl.LOCK_EX|fcntl.LOCK_NB)\nprint('locked',flush=True)\ntime.sleep(30)", lock]
        let out = Pipe()
        holder.standardOutput = out
        try holder.run()
        defer { holder.terminate() }
        _ = out.fileHandleForReading.availableData // "locked"
        XCTAssertEqual(call(["call": "lockHeld", "path": lock])["held"] as? Bool, true)
    }

    func testProcessesHostsAndAddresses() {
        XCTAssertNotNil(call(["call": "signal0", "pid": Int(getpid())])["code"] == nil ? true : nil)
        XCTAssertEqual(call(["call": "signal0", "pid": 999_999])["code"] as? String, "ESRCH")
        XCTAssertFalse((call(["call": "hostname"])["hostname"] as? String ?? "").isEmpty)
        XCTAssertTrue((call(["call": "localAddresses"])["addresses"] as? [String] ?? []).contains("127.0.0.1"))
    }
}

_ = NSApplication.shared
NSApp.setActivationPolicy(.accessory)
let suite = XCTestSuite(name: "browser-profiles")
suite.addTest(XCTestSuite(forTestCaseClass: ProfileStoreTests.self))
suite.addTest(XCTestSuite(forTestCaseClass: ImportIOTests.self))
suite.run()
let run = suite.testRun!
print("Executed \(run.executionCount) tests, with \(run.totalFailureCount) failures")
exit(run.totalFailureCount == 0 ? 0 : 1)
