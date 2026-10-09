// @ref llp/1109.010-mobile-browser-devices.decision.md#native-lifetime
// Media tickets expire after five minutes; an open stream can outlive its ticket.
import Foundation

enum T3MobileBrowserFileAccess {
    static func url(_ original: URL, origin: String, endpoint: String, access: [String: Any], operate: Bool) -> URL? {
        guard ["upload", "download"].contains(endpoint),
              let expected = URL(string: origin), original.scheme == expected.scheme,
              original.host == expected.host, original.port == expected.port,
              original.user == nil, original.password == nil,
              original.path == "/api/preview-stream/\(endpoint)",
              !operate || access["_operate"] as? Bool == true,
              let query = access["query"] as? [String: String], let ticket = query["wsTicket"], !ticket.isEmpty,
              var parts = URLComponents(url: original, resolvingAgainstBaseURL: false) else { return nil }
        // Keep the exact tab/chooser/download coordinates, replace only the expired credential.
        parts.queryItems = (parts.queryItems ?? []).filter { $0.name != "wsTicket" } + [URLQueryItem(name: "wsTicket", value: ticket)]
        return parts.url
    }
}
