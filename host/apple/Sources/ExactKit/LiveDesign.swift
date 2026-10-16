// The live table (LLP 1121 D13, stage 4): a value-only design edit applied to
// a running app, every session in place, no plan and no restart.
import Foundation

extension ExactApp {
    /// A live design batch or snapshot from the dev stream (LLP 1121 stage
    /// 4), applied to every booted session in place, no plan, no restart:
    /// stale when a session does not run the batch's base (the connection
    /// then fetches the snapshot), else applied or why not. The build
    /// channel (LLP 1122 D4) hears of a refused colour and of a new design.
    func design(_ frame: String) -> String {
        var answer = "applied", refused = false, design: String?
        for session in sessions where session.state != .destroyed && session.booted {
            let batch = session.runtime.designRows(frame)
            // Its references reported again: a value may name a platform colour no report resolved yet.
            if batch.live == "applied" { session.apply(batch); session.reportColors() }
            if batch.live == "stale" { answer = "stale" } else if answer == "applied", let live = batch.live, live != "applied" { answer = live }
            if batch.unsupported { refused = true }
            design = design ?? batch.design
        }
        if refused {
            fputs("exact url: live: a colour was refused (the wide colour table is full)\n", stderr)
            connection?.report(["kind": "unsupported", "what": "batch", "detail": "the wide colour table is full: a colour was refused"])
        }
        if let design, design != BuildChannel.design { BuildChannel.design = design; scheduleStatus() }
        return answer
    }

    /// A session booted from the published plan, which carries that plan's
    /// own design: that design is the one reported until the server's
    /// snapshot says otherwise (`/__dev/design` answers 204 while the
    /// server's is the plan's), fetched again here; a pin held is set on
    /// it, and the generation reported (LLP 1122 D4).
    func liveBooted(_ session: ExactSession) {
        if BuildChannel.enabled, let design = session.runtime.designDigest() { BuildChannel.design = design }
        connection?.syncDesign(); repinBooted()
    }
}
