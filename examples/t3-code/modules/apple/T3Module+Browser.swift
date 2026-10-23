// T3Module's Browser ops (T3Module.swift routes them): the data module's live sessions,
// a tab's navigation and its chrome row's commands, answered by the page registry (T3BrowserSessions.swift).
// The previewAutomation host's plans (`browserAutomation`, T3BrowserAutomation.swift), which answer
// `previewAutomation.respond` on the focused connection, and a tab's Mute (`browserMute`). A recording the agent stops is
// uploaded to its environment natively (`browserRecordingUpload`, T3BrowserAutomationRecording.swift).
import Foundation

extension T3Module: T3BrowserSessionOwner {
    func browserOps(_ request: [String: Any], reply: ExactReply, next: () -> Void) {
        if browserSessions.performProfiles(request, reply: reply) { return } // clearing, the cookie import (T3BrowserSessions+Profiles.swift)
        let generation = request["generation"] as? Int ?? 0
        switch request["op"] as? String {
        case "browserAutomation":
            let plan = request["plan"] as? [String: Any] ?? [:]
            return DispatchQueue.main.async { [browserAutomation] in reply.send(["ok": true, "generation": generation, "value": browserAutomation.perform(plan)]) }
        case "browserRecordingUpload":
            return DispatchQueue.main.async { [browserAutomation] in
                browserAutomation.recordingUpload(request) { value in reply.send(["ok": true, "generation": generation, "value": value]) }
            }
        case "browserMute":
            let tab = request["tab"] as? String ?? "", muted = request["muted"] as? Bool == true
            return DispatchQueue.main.async { [browserAutomation] in reply.send(["ok": true, "generation": generation, "value": ["done": browserAutomation.setMuted(tab, muted)]]) }
        default:
            // browserSet; browserOpenLinks (T3BrowserSession+Links.swift)
            if let answer = browserSessions.perform(request) ?? browserSessions.performNavigation(request) ?? browserSessions.performLinks(request) { return reply.send(answer) }
            if browserSessions.performCapture(request, reply: { reply.send($0) }) { return } // T3BrowserCapture.swift
            next()
        }
    }

    /// The executor's pages and its connection: each new page gets the host's scripts; a response goes out on the
    /// module's link to the environment the request came from (T3Link.swift), focused or in the background.
    func attachBrowserAutomation() {
        browserSessions.created = { [weak browserAutomation] session in browserAutomation?.prepare(session) }
        browserSessions.ended = { [weak browserAutomation] id in browserAutomation?.forget(id) }
        browserAutomation.changed = { [weak browserSessions] in browserSessions?.publish() }
        let reply = { (done: @escaping ([String: Any]) -> Void) -> T3Link.Done in
            { failure, value in done(failure.map { ["ok": false, "error": $0.json] } ?? ["ok": true, "value": value ?? NSNull()]) }
        }
        browserAutomation.send = { [links] request, _, key, done in
            DispatchQueue.main.async {
                guard request["op"] as? String == "uploadAttachment", let link = links.link(fleet: key) else { return done(["ok": false]) }
                let data = Data(base64Encoded: request["base64"] as? String ?? "") ?? Data()
                link.upload(path: request["path"] as? String ?? "", data: data, contentType: request["contentType"] as? String ?? "application/octet-stream", done: reply(done))
            }
        }
        browserAutomation.rpc = { [links] method, payload, _, key, done in
            DispatchQueue.main.async {
                guard let link = links.link(fleet: key) else { return done(["ok": false, "error": T3Failure(kind: "Disconnected", message: "The server is not connected.").json]) }
                link.request(method, payload: payload, done: reply(done))
            }
        }
    }
}
