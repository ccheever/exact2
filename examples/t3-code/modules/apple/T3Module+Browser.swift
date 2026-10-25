// T3Module's Browser ops (T3Module.swift routes them): what a tab's frame hands the `t3-browser-page` hatch for
// (T3BrowserPage.swift: the recording, the agent's cursor, the separate window) and the artifact directory's files. A
// saved recording's upload goes out on the module's link to the environment the request names (T3Link.swift).
import Foundation

extension T3Module {
    func browserOps(_ request: [String: Any], reply: ExactReply, next: () -> Void) {
        if browserPages.perform(request, reply: { reply.send($0) }) { return }
        next()
    }

    /// The upload's connection: a response goes out on the module's link to the environment the request came from,
    /// focused or in the background.
    func attachBrowserPages() {
        let reply = { (done: @escaping ([String: Any]) -> Void) -> T3Link.Done in
            { failure, value in done(failure.map { ["ok": false, "error": $0.json] } ?? ["ok": true, "value": value ?? NSNull()]) }
        }
        browserPages.send = { [links] request, _, key, done in
            DispatchQueue.main.async {
                guard request["op"] as? String == "uploadAttachment", let link = links.link(fleet: key) else { return done(["ok": false]) }
                let data = Data(base64Encoded: request["base64"] as? String ?? "") ?? Data()
                link.upload(path: request["path"] as? String ?? "", data: data, contentType: request["contentType"] as? String ?? "application/octet-stream", done: reply(done))
            }
        }
        browserPages.rpc = { [links] method, payload, _, key, done in
            DispatchQueue.main.async {
                guard let link = links.link(fleet: key) else { return done(["ok": false, "error": T3Failure(kind: "Disconnected", message: "The server is not connected.").json]) }
                link.request(method, payload: payload, done: reply(done))
            }
        }
    }
}
