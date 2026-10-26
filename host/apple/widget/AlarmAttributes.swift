// LLP 1133.030 D8 item 1: the metadata type of every alarm a package
// schedules through AlarmKit. The build compiles this file into the module
// of a package whose Apple implementation declares `"apple": {"activity":
// "alarmkit"}`, on iOS only, and into the widget extension beside
// `Alarm.swift`; ActivityKit matches the alarm's Live Activity to the
// extension's configuration by this type, as Apple's sample shares one
// attributes file between its two targets. It carries nothing: the title,
// the tint and the time are AlarmKit's own attributes and state.
#if os(iOS)
import AlarmKit

@available(iOS 26.0, *)
public struct ExactAlarmMetadata: AlarmMetadata {
    public init() {}
}
#endif
