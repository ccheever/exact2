#if os(iOS)
import UIKit

extension DevMenu {
    /// The same compact JSON an agent reads, shared as text without creating an app file.
    static func shareFrameHealth() {
        guard let session else { return }
        let host = controller
        FrameHealthReport.prepare(health: session.frameHealth.reportSource, session: session) { [weak host, weak session] data in
            guard let session, session.state != .destroyed, controller === host else { return }
            let text = String(decoding: data, as: UTF8.self)
            let alert = UIAlertController(title: "Frame Health Report",
                message: "A compact timing report (\(data.count) bytes). Share it with an agent or copy it. App text, screenshots and logs are excluded.", preferredStyle: .alert)
            alert.addAction(UIAlertAction(title: "Share…", style: .default) { _ in
                present(UIActivityViewController(activityItems: [text], applicationActivities: nil))
            })
            alert.addAction(UIAlertAction(title: "Copy", style: .default) { _ in UIPasteboard.general.string = text })
            alert.addAction(UIAlertAction(title: "Cancel", style: .cancel))
            present(alert)
        }
    }
}
#endif
