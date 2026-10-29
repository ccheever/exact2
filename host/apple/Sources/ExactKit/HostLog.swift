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
        guard let held = window.firstInTree({ $0.isFirstResponder ? $0 : nil }) else { return "nothing" }
        var up: UIView? = held
        while let view = up {
            if let node = view as? NodeView { return "#\(node.id)\(node.props["testId"].map { " [\($0)]" } ?? "")" }
            up = view.superview
        }
        return String(describing: type(of: held))
    }
}

extension UIView {
    /// The first of this view and its descendants, depth first, that `match`
    /// gives a value for, each view asked once; a view `prune` is true of is
    /// passed over with its subtree. Not `subviews.lazy.compactMap(…).first`:
    /// `first` asks the matching child twice, which doubles a recursive
    /// search at each level of depth.
    package func firstInTree<T>(_ match: (UIView) -> T?, prune: (UIView) -> Bool = { _ in false }) -> T? {
        if prune(self) { return nil }
        if let found = match(self) { return found }
        for child in subviews { if let found = child.firstInTree(match, prune: prune) { return found } }
        return nil
    }
}
#endif
