// The live table (LLP 1121 D13, stage 4): a value-only design edit applied to
// a running app, every session in place, no plan and no restart.
import Foundation

extension ExactApp {
    /// A live design batch or snapshot from the dev stream (LLP 1121 stage
    /// 4), applied to every booted session in place, no plan, no restart:
    /// stale when a session does not run the batch's base (the connection
    /// then fetches the snapshot), else applied or why not.
    func design(_ frame: String) -> String {
        var answer = "applied"
        for session in sessions where session.state != .destroyed && session.booted {
            let batch = session.runtime.designRows(frame)
            // Its references reported again: a value may name a platform colour no report resolved yet.
            if batch.live == "applied" { session.apply(batch); session.reportColors() }
            if batch.live == "stale" { answer = "stale" } else if answer == "applied", let live = batch.live, live != "applied" { answer = live }
            if batch.unsupported { fputs("exact url: live: a colour was refused (the wide colour table is full)\n", stderr) }
        }
        return answer
    }

    /// A session booted from the published plan, which carries that plan's
    /// own design: the one the server's builds run is fetched again.
    func liveBooted() { connection?.syncDesign() }
}
