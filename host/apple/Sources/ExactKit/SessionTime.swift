// The date a session tells its runner (LLP 1027.000.000): at boot, before
// the first settle, and whenever the zone's offset or the drive's clock moves.
import Foundation
import ExactCore

extension ExactSession {
    /// @ref LLP 1027.000.000 — the date, against the clock `now()` reads.
    func tellTime() {
        if launchPlace.epoch != nil {
            tellAgentOffset()
        } else {
            toldOffset = nil
            offsetHeld = nil
            followOffset()
        }
        apply(runtime.setPlace(locale: launchPlace.locale, timeZone: launchPlace.timeZone, seed: launchPlace.seed, formatLocale: launchPlace.formatLocale))
        tellPreferences()
        tellPage()
    }
    /// The machine zone's offset, told when it moved. `offsetHeld` is the
    /// zone it was read in and when to read it again: until then, in that
    /// zone, a frame's check reads no calendar.
    func followOffset() {
        guard launchPlace.epoch == nil else { return }
        let zone = TimeZone.autoupdatingCurrent, wall = Date()
        if let held = offsetHeld, held.zone == zone.identifier, wall < held.until { return }
        // An hour at most: a zone's rules can move its offset at a time that
        // is no daylight-saving transition.
        let hour = wall.addingTimeInterval(3600)
        offsetHeld = (zone.identifier, min(zone.nextDaylightSavingTimeTransition(after: wall) ?? hour, hour))
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
    /// The date and where the viewer is (locale, zone, seed) before each
    /// boot or candidate: the runner reads them from its first settle, so
    /// an argument reading the zone's offset or the locale (a kept answer's
    /// identity) is the real one, not a default (or a reload's stale date)
    /// until `tellTime`. A running runner is not told.
    func primeTime() {
        guard state != .destroyed else { return }
        let date = dateNow()
        runtime.setLaunchTime(epochAtZero: date.epoch, utcOffset: date.offset)
        runtime.setLaunchPlace(locale: launchPlace.locale, timeZone: launchPlace.timeZone, seed: launchPlace.seed, formatLocale: launchPlace.formatLocale)
        runtime.liveClock(clock == nil)
    }
    /// The Region or the hour cycle the user forces changed (Foundation
    /// posts `NSLocale.currentLocaleDidChangeNotification` for either):
    /// `formatLocale` is read again and told, as a notice. The agent's is the drive's.
    func observeFormatLocale() -> NSObjectProtocol {
        NotificationCenter.default.addObserver(forName: NSLocale.currentLocaleDidChangeNotification, object: nil, queue: .main) { [weak self] _ in
            self?.formatLocaleChanged()
        }
    }
    func formatLocaleChanged() {
        guard launchPlace.epoch == nil, state != .destroyed else { return }
        let read = SessionFacts.currentFormatLocale
        guard read != launchPlace.formatLocale else { return }
        launchPlace.formatLocale = read
        guard booted else { return } // `primeTime` tells the first boot
        let place = launchPlace
        let tell: (Runtime) -> Batch = { $0.setPlace(locale: place.locale, timeZone: place.timeZone, seed: place.seed, formatLocale: place.formatLocale) }
        if dataPreparing, !Owner.shared.isServingMain { tellSoon([tell]) } else { apply(tell(runtime)) }
    }
}
