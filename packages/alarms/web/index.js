// @exact/alarms on the web (LLP 1133.030 D14): absent. No web API rings
// through a device's silence or rings while the page is closed, and
// Notification Triggers was abandoned. So the status says alarms are not
// available here, the list is empty, and every send is `NotSupportedError`.
// The page runtime (host/web-js/modules.js) calls this outside the agent;
// under the agent the declared substitute answers the same (`web`,
// `unsupported`).

const STATUS = { available: false, state: 'unknown', countdown: false, apple: null, android: null };
const absent = async () => ({ error: 'NotSupportedError' });

export function implementation() {
  return {
    async alarmStatus() { return STATUS; },
    async alarms() { return []; },
    scheduleAlarm: absent,
    cancelAlarm: absent,
    pauseAlarm: absent,
    resumeAlarm: absent,
    requestAlarmPermission: absent,
  };
}
