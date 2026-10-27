// @ref LLP 1143 D2 — what the host did, as lines in the runner's journal (the
// agent's `logs`) and in the unified log (`log stream --predicate
// 'subsystem == "dev.exact" AND category == "host"'`), for a run a person
// drives by hand. A fixed set of events: navigation, presentation, focus, the
// keyboard and the status bar. Events only, never the edges between them.
// A production bake writes nothing.
import Foundation
import os

enum HostLog {
    private static let logger = Logger(subsystem: "dev.exact", category: "host")

    static func note(_ session: ExactSession?, _ line: @autoclosure () -> String) {
        guard HatchDiagnostics.available, let session else { return }
        let line = line()
        session.log("host: " + line)
        logger.debug("\(line, privacy: .public)")
    }
}

#if os(iOS) || os(tvOS)
import UIKit

extension Presenter {
    /// What holds the focus in `window`, as a host line names it: a node by
    /// its id and testId, else the responder's class.
    static func focusHolder(in window: UIWindow) -> String {
        // Not `subviews.lazy.compactMap(first).first`: it evaluates the matching
        // child twice, which doubles the search at each level of depth.
        func first(_ view: UIView) -> UIView? {
            if view.isFirstResponder { return view }
            for child in view.subviews { if let found = first(child) { return found } }
            return nil
        }
        guard let held = first(window) else { return "nothing" }
        var up: UIView? = held
        while let view = up {
            if let node = view as? NodeView { return "#\(node.id)\(node.props["testId"].map { " [\($0)]" } ?? "")" }
            up = view.superview
        }
        return String(describing: type(of: held))
    }
}
#endif
