// T3Module's Browser ops (T3Module.swift routes them): what a tab's frame hands the `t3-browser-page` hatch for
// (T3BrowserPage.swift: the recording and its upload, the agent's cursor, the separate window) and the artifact
// directory's files.
import Foundation

extension T3Module {
    func browserOps(_ request: [String: Any], reply: ExactReply, next: () -> Void) {
        if browserPages.perform(request, reply: { reply.send($0) }) { return }
        next()
    }
}
