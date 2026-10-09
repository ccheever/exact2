// @ref llp/1109.010-mobile-browser-devices.decision.md#native-lifetime
// swiftc modules/apple/T3MobileBrowserFileAccess.swift tests/BrowserFileAccessTests.swift -o /tmp/browser-file-access-tests
import Foundation

@main
struct BrowserFileAccessTests {
    static func main() {
        var checks = 0
        var failures: [String] = []
        func check(_ condition: Bool, _ label: String) { if !condition { failures.append(label) }; checks += 1 }
        let origin = "https://preview.test:8443"
        let source = URL(string: origin + "/api/preview-stream/upload?threadId=t%2Fh&tabId=one&chooser=c%2B1&wsTicket=expired&wsTicket=duplicate")!
        let access: [String: Any] = ["query": ["wsTicket": "fresh+/credential"], "_operate": true]
        let result = T3MobileBrowserFileAccess.url(source, origin: origin, endpoint: "upload", access: access, operate: true)!
        let query = URLComponents(url: result, resolvingAgainstBaseURL: false)!.queryItems!
        check(query.filter { $0.name == "wsTicket" }.map { $0.value! } == ["fresh+/credential"], "replace all expired ticket occurrences")
        check(query.filter { $0.name != "wsTicket" } == URLComponents(url: source, resolvingAgainstBaseURL: false)!.queryItems!.filter { $0.name != "wsTicket" }, "preserve exact target and chooser")
        for bad in ["http://preview.test:8443/api/preview-stream/upload", "https://other.test:8443/api/preview-stream/upload", "https://preview.test/api/preview-stream/upload", "https://user@preview.test:8443/api/preview-stream/upload", origin + "/api/preview-stream/download", origin + "/api/other"] {
         check(T3MobileBrowserFileAccess.url(URL(string: bad)!, origin: origin, endpoint: "upload", access: access, operate: true) == nil, "refuse mismatched origin/endpoint/credentials")
        }
        for invalid: [String: Any] in [[:], ["query": ["wsTicket": ""]], ["query": ["wsTicket": "fresh"], "_operate": false]] {
         check(T3MobileBrowserFileAccess.url(source, origin: origin, endpoint: "upload", access: invalid, operate: true) == nil, "require fresh ticket and current operate grant")
        }
        let download = URL(string: origin + "/api/preview-stream/download?id=file&wsTicket=old")!
        check(T3MobileBrowserFileAccess.url(download, origin: origin, endpoint: "download", access: ["query": ["wsTicket": "fresh"], "_operate": false], operate: false) != nil, "download needs read access only")
        check(T3MobileBrowserFileAccess.url(download, origin: origin, endpoint: "other", access: access, operate: false) == nil, "endpoint allowlist")
        for failure in failures { print("FAIL: \(failure)") }
        if !failures.isEmpty { exit(1) }
        print("Browser file access: \(checks) checks passed")
    }
}
