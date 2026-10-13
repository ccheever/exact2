// The date a session tells its runner (LLP 1027.000.000): at boot, before
// the first settle, and whenever the zone's offset or the drive's clock moves.
import Foundation

extension ExactSession {
    /// @ref LLP 1027.000.000 — the date, against the clock `now()` reads.
    func tellTime() {
        if launchPlace.epoch != nil {
            tellAgentOffset()
        } else {
            toldOffset = nil
            followOffset()
        }
        apply(runtime.setPlace(locale: launchPlace.locale, timeZone: launchPlace.timeZone, seed: launchPlace.seed))
        tellPreferences()
        tellPage()
    }
    func followOffset() {
        guard launchPlace.epoch == nil else { return }
        let date = dateNow()
        guard date.offset != toldOffset else { return }
        toldOffset = date.offset
        apply(runtime.setTime(epochAtZero: date.epoch, utcOffset: date.offset))
    }
    /// The date at the clock's zero and the zone's offset now: the drive's
    /// under the agent (at the virtual instant), the machine's otherwise.
    func dateNow() -> (epoch: Double, offset: Double) {
        guard let epoch = launchPlace.epoch else {
            return (Date().timeIntervalSince1970 * 1000 - now(), Double(TimeZone.autoupdatingCurrent.secondsFromGMT()) / 60)
        }
        let zone = TimeZone(identifier: launchPlace.timeZone) ?? TimeZone(secondsFromGMT: 0)!
        return (epoch, Double(zone.secondsFromGMT(for: Date(timeIntervalSince1970: (epoch + now()) / 1000))) / 60)
    }
    /// Under the agent, the drive's date at the clock's zero and its zone's
    /// offset at the virtual instant the clock reads: told at boot and after
    /// every `clock`, so a move across a DST change re-answers it (LLP
    /// 1069.007 D2). An unchanged offset commits nothing.
    func tellAgentOffset() {
        guard launchPlace.epoch != nil else { return }
        let date = dateNow()
        apply(runtime.setTime(epochAtZero: date.epoch, utcOffset: date.offset))
    }
    /// The date before the first boot: the runner reads it from its first
    /// settle, so an argument reading the zone's offset (a kept answer's
    /// identity) is the real one, not zero until `tellTime`.
    func primeTime() {
        // Before a re-boot too, so it starts from today's date: the running
        // runner hears it as well, and what that commits is presented, as
        // the old host stays if the boot fails.
        guard state != .destroyed else { return }
        let date = dateNow()
        let out = runtime.setTime(epochAtZero: date.epoch, utcOffset: date.offset)
        if booted { apply(out) }
    }
}
