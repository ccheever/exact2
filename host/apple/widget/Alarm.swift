// LLP 1133.030 D8 items 1 and 3: the countdown of an alarm a package set
// through AlarmKit, on the Lock Screen and in the Dynamic Island. AlarmKit
// starts, updates and ends this Live Activity from `AlarmPresentationState`;
// without it "the system may unexpectedly dismiss alarms and fail to alert"
// (AlarmKit). It reads nothing of the app: no ExactCore, no app group, no
// cohort, only the alarm's attributes and state. It shows no buttons, since
// a button here runs a `LiveActivityIntent`, which the build does not
// process; the alert's Stop and Repeat are the system's.
import ActivityKit
import AlarmKit
import SwiftUI
import WidgetKit

@available(iOS 26.0, *)
struct ExactAlarmCountdown: Widget {
    var body: some WidgetConfiguration {
        ActivityConfiguration(for: AlarmAttributes<ExactAlarmMetadata>.self) { context in
            HStack(alignment: .center) {
                AlarmTitle(context: context).font(.headline)
                Spacer()
                AlarmTime(state: context.state).font(.system(.title, design: .rounded).monospacedDigit())
            }
            .padding()
            .tint(context.attributes.tintColor)
        } dynamicIsland: { context in
            DynamicIsland {
                DynamicIslandExpandedRegion(.leading) { AlarmTitle(context: context) }
                DynamicIslandExpandedRegion(.trailing) {
                    AlarmTime(state: context.state).font(.title2.monospacedDigit()).foregroundStyle(context.attributes.tintColor)
                }
            } compactLeading: {
                Image(systemName: AlarmTime.paused(context.state) ? "pause.fill" : "timer").foregroundStyle(context.attributes.tintColor)
            } compactTrailing: {
                AlarmTime(state: context.state).monospacedDigit().foregroundStyle(context.attributes.tintColor).frame(maxWidth: 64)
            } minimal: {
                Image(systemName: "timer").foregroundStyle(context.attributes.tintColor)
            }
            .keylineTint(context.attributes.tintColor)
        }
    }
}

/// The title the alarm was set with: its countdown's, else its alert's.
@available(iOS 26.0, *)
private struct AlarmTitle: View {
    let context: ActivityViewContext<AlarmAttributes<ExactAlarmMetadata>>
    var body: some View {
        let presentation = context.attributes.presentation
        Text(presentation.countdown?.title ?? presentation.alert.title).lineLimit(1)
    }
}

/// The time left, counting down; "Paused" with the time left; nothing while
/// the alert shows, which is the system's.
@available(iOS 26.0, *)
private struct AlarmTime: View {
    let state: AlarmPresentationState
    static func paused(_ state: AlarmPresentationState) -> Bool {
        if case .paused = state.mode { return true }
        return false
    }
    var body: some View {
        switch state.mode {
        case .countdown(let c):
            Text(timerInterval: min(Date.now, c.fireDate)...c.fireDate, countsDown: true).multilineTextAlignment(.trailing)
        case .paused(let p):
            let left = Duration.seconds(max(0, p.totalCountdownDuration - p.previouslyElapsedDuration))
            Text("Paused \(left.formatted(.time(pattern: .minuteSecond)))")
        default:
            EmptyView()
        }
    }
}
