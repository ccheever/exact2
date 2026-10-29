// The emulator half of android.mjs (LLP 1133.030 §14 stage 2): the fixture under the agent with the implementation
// bound, on the wall clock. The system's own screens (the notification dialog, the notification shade's Stop and
// Repeat) are pressed through `uiautomator`; the exact-alarm switch is set with `appops`, as its Settings page does.
import { resolve } from 'node:path';
import { texts } from '../../photos/test/fixture.mjs';

const wait = (ms) => new Promise((done) => setTimeout(done, ms));

export async function drive({ root, env, name, emulator, check, run, out }) {
  const adb = (...a) => run('adb', ['-s', emulator, ...a], root, process.env, true);
  const sdk = Number(adb('shell', 'getprop', 'ro.build.version.sdk').trim());
  Object.assign(process.env, env, { ANDROID_SERIAL: emulator, EXACT_ANDROID_READER: '1' });
  const { open } = await import(resolve(root, 'scripts/agent.mjs'));
  const launch = () => open({ host: 'android', app: name, bind: 'alarms', epoch: 'now', storage: `alarmfixture-${process.pid}` });
  const shot = (file) => out && run('sh', ['-c', `adb -s ${emulator} exec-out screencap -p > '${resolve(out, file)}'`], root, process.env, true);

  /** The system UI's node whose text or description is `label`, pressed at its centre. */
  const press = async (label, ms = 15000) => {
    const end = Date.now() + ms;
    while (Date.now() < end) {
      adb('shell', 'uiautomator', 'dump', '/sdcard/alarms-ui.xml');
      const xml = adb('shell', 'cat', '/sdcard/alarms-ui.xml');
      const node = [...xml.matchAll(/<node [^>]*>/g)].map((m) => m[0])
        .find((n) => new RegExp(`(text|content-desc)="${label}"`, 'i').test(n) && /clickable="true"|enabled="true"/.test(n));
      const b = node?.match(/bounds="\[(\d+),(\d+)\]\[(\d+),(\d+)\]"/);
      if (b) {
        adb('shell', 'input', 'tap', String((+b[1] + +b[3]) >> 1), String((+b[2] + +b[4]) >> 1));
        return true;
      }
      await wait(500);
    }
    return false;
  };
  /** `testId`'s text once `want` holds, reading on the wall clock; null while the app is away. */
  const read = async (s, testId, want, ms = 15000) => {
    const end = Date.now() + ms;
    let got = null;
    while (Date.now() < end) {
      try { if (want(got = (await texts(s))[testId])) return got; } catch { got = null; }
      await wait(500);
    }
    return got;
  };

  let s = await launch();
  const pkg = adb('shell', 'pm', 'list', 'packages').split('\n').map((l) => l.replace('package:', '').trim()).find((p) => p.includes(name)) ?? '';
  const dumpsys = (what) => adb('shell', 'dumpsys', ...what);
  try {
    // 1. Before asking: exact alarms are off by default from Android 14 for a new install, and notifications `prompt`.
    adb('shell', 'cmd', 'appops', 'set', pkg, 'SCHEDULE_EXACT_ALARM', 'default');
    const fresh = await read(s, 'status', (t) => /^true /.test(t ?? ''));
    check('android', 'before asking: available and prompt', fresh === 'true prompt true', fresh);
    check('android', 'the platform half reads both switches', await read(s, 'android', (t) => t && t !== 'none') === `false ${sdk >= 33 ? 'prompt' : 'granted'}`, (await texts(s)).android);

    // 2. Exact alarms held and notifications still `prompt`: `prompt`, and the request asks for notifications only.
    adb('shell', 'cmd', 'appops', 'set', pkg, 'SCHEDULE_EXACT_ALARM', 'allow');
    await s.tap('request');
    if (sdk >= 33) {
      const asked = await press('Allow');
      check('android', "the request shows the system's notification dialog, and Allow answers it", asked, null);
      shot('android-1-request.png');
    }
    const granted = await read(s, 'status', (t) => t === 'true granted true', 20000);
    check('android', 'allowed: granted, with no Settings page (exact alarms were held)', granted === 'true granted true', granted);

    // 3. A one-minute countdown: the system's alarm clock, and the ongoing countdown notification.
    await s.tap('countdown');
    const counting = await read(s, 'alarms', (t) => t === 'timer countdown');
    check('android', 'the countdown is listed', counting === 'timer countdown', counting);
    const alarms = dumpsys(['alarm']);
    check('android', '`dumpsys alarm` lists it as an alarm clock', alarms.split('\n').some((l) => l.includes(pkg) && /ALARM_CLOCK|alarmClock|AlarmClock/i.test(l))
      || new RegExp(`${pkg}[\\s\\S]{0,400}alarmClock=`, 'i').test(alarms), alarms.split('\n').filter((l) => l.includes(pkg)).slice(0, 6));
    const shade = dumpsys(['notification', '--noredact']);
    check('android', 'the countdown has its ongoing notification', new RegExp(`pkg=${pkg}[\\s\\S]{0,600}exact\\.alarms\\.countdown`).test(shade), null);
    shot('android-2-countdown.png');

    // 4. Pause and resume.
    await s.tap('pause');
    check('android', 'paused', await read(s, 'alarms', (t) => t === 'timer paused') === 'timer paused', (await texts(s)).alarms);
    await s.tap('resume');
    check('android', 'resumed', await read(s, 'alarms', (t) => t === 'timer countdown') === 'timer countdown', (await texts(s)).alarms);

    // 5. The ring: the service in the foreground, its notification with Stop and Repeat; Repeat counts down again.
    const rang = await read(s, 'alarms', (t) => t === 'timer alerting', 90000);
    check('android', 'it rings at its end', rang === 'timer alerting', rang);
    const services = dumpsys(['activity', 'services', pkg]);
    check('android', 'the ringing service runs in the foreground', /AlarmService/.test(services) && /isForeground=true/.test(services), services.slice(0, 600));
    const ringing = dumpsys(['notification', '--noredact']);
    check('android', 'the ring posts its alarm notification', new RegExp(`pkg=${pkg}[\\s\\S]{0,800}exact\\.alarms\\.ring`).test(ringing), null);
    const audio = dumpsys(['audio']);
    check('android', 'it plays on the alarm stream', /usage=USAGE_ALARM[^\n]*state:started|USAGE_ALARM[\s\S]{0,200}started/.test(audio), audio.split('\n').filter((l) => /USAGE_ALARM/.test(l)).slice(0, 4));
    adb('shell', 'cmd', 'statusbar', 'expand-notifications');
    shot('android-3-ring.png');
    const repeated = await press('Repeat');
    adb('shell', 'cmd', 'statusbar', 'collapse');
    check('android', "the notification's Repeat pressed", repeated, null);
    check('android', 'Repeat counts down again', await read(s, 'alarms', (t) => t === 'timer countdown', 15000) === 'timer countdown', (await texts(s)).alarms);
    check('android', 'the ring stopped', !/AlarmService/.test(dumpsys(['activity', 'services', pkg])), null);

    // 6. Cancel, and an alarm at an instant.
    await s.tap('cancel-timer');
    check('android', 'cancelled', await read(s, 'alarms', (t) => t === '') === '', (await texts(s)).alarms);
    await s.tap('at');
    check('android', 'an alarm at an instant is listed', await read(s, 'alarms', (t) => t === 'at scheduled') === 'at scheduled', (await texts(s)).alarms);
    await s.tap('cancel-at');
    await read(s, 'alarms', (t) => t === '');

    // 7. Revocation ends the alarm: the system stops the app; the next session reads it gone and `prompt`.
    await s.tap('countdown');
    await read(s, 'alarms', (t) => t === 'timer countdown');
    await s.close().catch(() => {});
    adb('shell', 'cmd', 'appops', 'set', pkg, 'SCHEDULE_EXACT_ALARM', 'deny');
    await wait(2000);
    s = await launch();
    const revoked = await read(s, 'status', (t) => t === 'true prompt true');
    check('android', 'revoked: prompt', revoked === 'true prompt true', revoked);
    check('android', 'revoked: the alarm is gone', await read(s, 'alarms', (t) => t === '') === '', (await texts(s)).alarms);
    check('android', 'revoked: the system holds no alarm clock for it', !dumpsys(['alarm']).split('\n').some((l) => l.includes(pkg) && /alarmClock/i.test(l)), null);

    // 8. Notifications off after scheduling: at the due time nothing sounds, and the next read is `denied`, the alarm gone.
    adb('shell', 'cmd', 'appops', 'set', pkg, 'SCHEDULE_EXACT_ALARM', 'allow');
    await s.close().catch(() => {});
    s = await launch();
    await read(s, 'status', (t) => t === 'true granted true');
    await s.tap('countdown');
    await read(s, 'alarms', (t) => t === 'timer countdown');
    await s.close().catch(() => {});
    if (sdk >= 33) adb('shell', 'pm', 'revoke', pkg, 'android.permission.POST_NOTIFICATIONS');
    else adb('shell', 'cmd', 'notification', 'disable_notifications', pkg);
    await wait(70000);
    check('android', 'notifications off: nothing rang', !/AlarmService/.test(dumpsys(['activity', 'services', pkg])), null);
    s = await launch();
    const off = await read(s, 'status', (t) => /denied/.test(t ?? ''));
    check('android', 'notifications off: denied', off === 'true denied true', off);
    check('android', 'notifications off: the alarm is gone', await read(s, 'alarms', (t) => t === '') === '', (await texts(s)).alarms);
    if (sdk >= 33) adb('shell', 'pm', 'grant', pkg, 'android.permission.POST_NOTIFICATIONS');
  } finally {
    await s.close().catch(() => {});
  }
  return pkg;
}
