// What runs at an alarm's time and at a boot, with no session (LLP 1133.030 D9): the receivers and the ringing service
// the package's manifest fragment declares, `directBootAware`, reading only the device-protected records.
package exact.alarms

import android.app.AlarmManager
import android.app.Notification
import android.app.NotificationChannel
import android.app.NotificationManager
import android.app.PendingIntent
import android.app.Service
import android.content.BroadcastReceiver
import android.content.Context
import android.content.Intent
import android.content.pm.ServiceInfo
import android.media.AudioAttributes
import android.media.MediaPlayer
import android.media.RingtoneManager
import android.os.Build
import android.os.Handler
import android.os.IBinder
import android.os.Looper
import android.os.PowerManager
import android.os.SystemClock
import android.provider.Settings

/** Setting, moving and ending the system's alarms for the records. */
internal object AlarmRing {
    const val RING = "exact.alarms.ring"
    const val COUNTDOWN = "exact.alarms.countdown"
    const val FIRE = "exact.alarms.FIRE"
    const val STOP = "exact.alarms.STOP"
    const val SNOOZE = "exact.alarms.SNOOZE"

    private fun alarms(c: Context) = c.getSystemService(AlarmManager::class.java)
    private fun notices(c: Context) = c.getSystemService(NotificationManager::class.java)
    private fun boot(c: Context) = Settings.Global.getInt(c.contentResolver, Settings.Global.BOOT_COUNT, -1)

    /** The ring's channel, which the person can block in Settings; its sound is the service's, so it is silent. */
    fun channels(c: Context) {
        val n = notices(c)
        if (n.getNotificationChannel(RING) == null) {
            n.createNotificationChannel(NotificationChannel(RING, "Alarms", NotificationManager.IMPORTANCE_HIGH).apply {
                setSound(null, null)
                enableVibration(false)
                lockscreenVisibility = Notification.VISIBILITY_PUBLIC
            })
        }
        if (n.getNotificationChannel(COUNTDOWN) == null) {
            n.createNotificationChannel(NotificationChannel(COUNTDOWN, "Timers", NotificationManager.IMPORTANCE_LOW).apply {
                setSound(null, null)
                lockscreenVisibility = Notification.VISIBILITY_PUBLIC
            })
        }
    }

    fun channelBlocked(c: Context): Boolean = notices(c).getNotificationChannel(RING)?.importance == NotificationManager.IMPORTANCE_NONE

    private fun fire(c: Context, r: AlarmRecord): PendingIntent = PendingIntent.getBroadcast(c, r.code,
        Intent(c, AlarmReceiver::class.java).setAction(FIRE).putExtra("tag", r.tag), PendingIntent.FLAG_UPDATE_CURRENT or PendingIntent.FLAG_IMMUTABLE)

    /** What the system's next-alarm display opens: the app. */
    private fun show(c: Context, r: AlarmRecord): PendingIntent? = c.packageManager.getLaunchIntentForPackage(c.packageName)?.let {
        PendingIntent.getActivity(c, r.code, it, PendingIntent.FLAG_UPDATE_CURRENT or PendingIntent.FLAG_IMMUTABLE)
    }

    fun action(c: Context, r: AlarmRecord, what: String): PendingIntent = PendingIntent.getBroadcast(c, r.code,
        Intent(c, AlarmReceiver::class.java).setAction(what).putExtra("tag", r.tag), PendingIntent.FLAG_UPDATE_CURRENT or PendingIntent.FLAG_IMMUTABLE)

    /** Sets `r` (a new one, or one that replaces the tag's), anchored now. */
    fun set(c: Context, r: AlarmRecord) {
        AlarmRecords.get(c, r.tag)?.let { end(c, it) }
        r.code = AlarmRecords.nextCode(c)
        arm(c, r)
    }

    /** The system alarm at `r.at`, a fresh anchor, the record, and a countdown's notification. */
    private fun arm(c: Context, r: AlarmRecord) {
        // Revoked since (D7): the system holds no alarm for the app, and the record goes with it.
        if (Build.VERSION.SDK_INT >= 31 && !alarms(c).canScheduleExactAlarms()) return AlarmRecords.remove(c, r.tag)
        val at = r.at!!.toLong()
        r.anchorWall = System.currentTimeMillis()
        r.anchorElapsed = SystemClock.elapsedRealtime()
        r.anchorBoot = boot(c)
        AlarmRecords.put(c, r)
        alarms(c).setAlarmClock(AlarmManager.AlarmClockInfo(at, show(c, r)), fire(c, r))
        if (r.state == "countdown") countdown(c, r)
    }

    /** The ongoing notification while a countdown runs (D3): its title and a chronometer counting down to the end. */
    private fun countdown(c: Context, r: AlarmRecord) {
        channels(c)
        val n = Notification.Builder(c, COUNTDOWN).setSmallIcon(android.R.drawable.ic_lock_idle_alarm).setContentTitle(r.title)
            .setOngoing(true).setOnlyAlertOnce(true).setCategory(Notification.CATEGORY_STOPWATCH)
            .setWhen(r.at!!.toLong()).setShowWhen(true).setUsesChronometer(true).setChronometerCountDown(true)
            .setContentIntent(show(c, r)).build()
        if (notices(c).areNotificationsEnabled()) notices(c).notify(r.code, n)
    }

    /** Ends the tag's system alarm, its notification and its ring, keeping the record. */
    private fun end(c: Context, r: AlarmRecord) {
        alarms(c).cancel(fire(c, r))
        notices(c).cancel(r.code)
        AlarmService.silence(c, r.tag)
    }

    fun cancel(c: Context, tag: String) {
        val r = AlarmRecords.get(c, tag) ?: return
        end(c, r)
        AlarmRecords.remove(c, tag)
    }

    /** `setAlarmClock` has no pause: the alarm is cancelled and `remaining` kept (D9). */
    fun pause(c: Context, r: AlarmRecord) {
        end(c, r)
        r.remaining = maxOf(0.0, r.at!! - corrected(c, r))
        r.at = null
        r.state = "paused"
        AlarmRecords.put(c, r)
    }

    fun resume(c: Context, r: AlarmRecord) {
        r.at = System.currentTimeMillis() + (r.remaining ?: 0.0)
        r.remaining = null
        r.state = "countdown"
        arm(c, r)
    }

    /** Stop: the ring, the notification and the record end. */
    fun stop(c: Context, tag: String) = cancel(c, tag)

    /** Snooze, or Repeat for a countdown: a countdown of `snooze` from now. */
    fun snooze(c: Context, tag: String) {
        val r = AlarmRecords.get(c, tag) ?: return
        val snooze = r.snooze ?: return stop(c, tag)
        end(c, r)
        r.at = System.currentTimeMillis() + snooze
        r.state = "countdown"
        arm(c, r)
    }

    /** Revocation, or a denial: every alarm the system holds ends; a paused countdown holds none and stays (D7). */
    fun dropHeld(c: Context) {
        for (r in AlarmRecords.all(c)) if (r.state != "paused") cancel(c, r.tag)
    }

    /**
     * Now on a countdown's own clock: the wall clock at the anchor plus the real time since, while the boot is the
     * anchor's. A wall clock set since then moves a countdown's end by nothing (D9); after a boot there is no anchor
     * and the wall clock is all there is.
     */
    fun corrected(c: Context, r: AlarmRecord): Double {
        val measured = r.state == "countdown" || r.state == "paused"
        if (!measured || r.anchorBoot < 0 || r.anchorBoot != boot(c)) return System.currentTimeMillis().toDouble()
        return (r.anchorWall + (SystemClock.elapsedRealtime() - r.anchorElapsed)).toDouble()
    }

    /**
     * The wall clock moved (`TIME_SET`): each countdown's end moves by the change, so it still ends after the same real
     * time; an `at` alarm keeps its instant (D9).
     */
    fun timeSet(c: Context) {
        for (r in AlarmRecords.all(c)) {
            if (r.state != "countdown" || r.anchorBoot != boot(c)) continue
            val shift = System.currentTimeMillis() - corrected(c, r)
            if (kotlin.math.abs(shift) < 1000) continue
            r.at = r.at!! + shift
            alarms(c).cancel(fire(c, r))
            arm(c, r)
        }
        AlarmsModule.changed()
    }

    /**
     * A boot (or the package replaced): the system dropped every alarm, so each is set again from its record with a
     * fresh anchor; one that fell due while the phone was off rings at once (D9).
     */
    fun restore(c: Context) {
        val now = System.currentTimeMillis()
        for (r in AlarmRecords.all(c)) when (r.state) {
            "countdown", "scheduled" -> if (r.at!! <= now) ring(c, r) else arm(c, r)
            // The service did not survive the restart: ring again.
            "alerting" -> ring(c, r)
        }
        AlarmsModule.changed()
    }

    /** The receiver's work at the alarm's time: the Stop invariant, the early-delivery check, then the ring. */
    fun due(c: Context, tag: String) {
        val r = AlarmRecords.get(c, tag) ?: return
        if (r.state != "countdown" && r.state != "scheduled") return
        // A forward jump of the wall clock delivers an RTC alarm early: a countdown not yet at its real end is set again.
        val early = r.at!! - corrected(c, r)
        if (r.state == "countdown" && early >= 1000) {
            r.at = System.currentTimeMillis() + early
            arm(c, r)
            return
        }
        // No ring that cannot be stopped (D7): with notifications off or the channel blocked there is no Stop.
        if (!notices(c).areNotificationsEnabled() || channelBlocked(c)) {
            cancel(c, tag)
            AlarmsModule.changed()
            return
        }
        ring(c, r)
    }

    private fun ring(c: Context, r: AlarmRecord) {
        notices(c).cancel(r.code)
        r.state = "alerting"
        AlarmRecords.put(c, r)
        val start = Intent(c, AlarmService::class.java).putExtra("tag", r.tag)
        c.startForegroundService(start)
        AlarmsModule.changed()
    }
}

/**
 * The alarm's own broadcast (explicit, so not in the fragment's action list), and the notification's Stop, Snooze and
 * dismissal. It holds a wake lock until the service has started.
 */
class AlarmReceiver : BroadcastReceiver() {
    override fun onReceive(c: Context, intent: Intent) {
        val tag = intent.getStringExtra("tag") ?: return
        when (intent.action) {
            AlarmRing.FIRE -> {
                val lock = c.getSystemService(PowerManager::class.java).newWakeLock(PowerManager.PARTIAL_WAKE_LOCK, "exact:alarm")
                lock.acquire(10_000)
                AlarmService.waking = lock
                AlarmRing.due(c, tag)
            }
            AlarmRing.STOP -> AlarmRing.stop(c, tag)
            AlarmRing.SNOOZE -> AlarmRing.snooze(c, tag)
        }
        AlarmsModule.changed()
    }
}

/** A boot, the package replaced, the wall clock or the zone changed, the exact-alarm switch changed (D9). */
class AlarmSystemReceiver : BroadcastReceiver() {
    override fun onReceive(c: Context, intent: Intent) {
        when (intent.action) {
            Intent.ACTION_LOCKED_BOOT_COMPLETED, Intent.ACTION_BOOT_COMPLETED, Intent.ACTION_MY_PACKAGE_REPLACED -> AlarmRing.restore(c)
            Intent.ACTION_TIME_CHANGED -> AlarmRing.timeSet(c)
            // An epoch instant has no zone: nothing moves, the app reads again.
            Intent.ACTION_TIMEZONE_CHANGED -> AlarmsModule.changed()
            AlarmManager.ACTION_SCHEDULE_EXACT_ALARM_PERMISSION_STATE_CHANGED -> {
                if (c.getSystemService(AlarmManager::class.java).canScheduleExactAlarms()) AlarmRing.restore(c) else AlarmRing.dropHeld(c)
                AlarmsModule.changed()
            }
        }
    }
}

/**
 * The ring: a foreground service (`systemExempted` from Android 14) that posts the alarm's notification, with Stop and
 * Snooze, and plays the alarm sound on the alarm stream, looping, until stopped or for [LIMIT], the Clock app's default
 * "silence after"; then the alarm ends as a Stop would.
 */
class AlarmService : Service() {
    companion object {
        /** Ten minutes: the Clock app's default "Silence after". */
        const val LIMIT = 10 * 60_000L
        @Volatile internal var waking: PowerManager.WakeLock? = null
        @Volatile private var running: AlarmService? = null

        fun silence(c: Context, tag: String) { running?.let { s -> Handler(Looper.getMainLooper()).post { s.end(tag) } } }
    }

    private val ringing = LinkedHashMap<String, Int>()
    private var player: MediaPlayer? = null
    private var foregroundCode = 0
    private val main = Handler(Looper.getMainLooper())

    override fun onBind(intent: Intent?): IBinder? = null

    override fun onCreate() {
        super.onCreate()
        running = this
    }

    override fun onStartCommand(intent: Intent?, flags: Int, startId: Int): Int {
        val tag = intent?.getStringExtra("tag")
        val r = tag?.let { AlarmRecords.get(this, it) }
        AlarmRing.channels(this)
        if (r == null || r.state != "alerting" || r.tag in ringing) {
            // Stopped before the service started, or already ringing. A service started with
            // `startForegroundService` must still enter the foreground once, or the system ends the app.
            if (ringing.isEmpty()) {
                foreground(Int.MAX_VALUE, Notification.Builder(this, AlarmRing.RING).setSmallIcon(android.R.drawable.ic_lock_idle_alarm).build())
                stopForeground(STOP_FOREGROUND_REMOVE)
                stopSelf()
            }
            return START_NOT_STICKY
        }
        val n = notification(r)
        if (ringing.isEmpty()) {
            foreground(r.code, n)
            foregroundCode = r.code
        } else {
            getSystemService(NotificationManager::class.java).notify(r.code, n)
        }
        ringing[r.tag] = r.code
        waking?.let { if (it.isHeld) it.release() }
        waking = null
        play()
        main.postDelayed({ if (tag in ringing) AlarmRing.stop(this, tag).also { AlarmsModule.changed() } }, LIMIT)
        return START_NOT_STICKY
    }

    private fun notification(r: AlarmRecord): Notification {
        val second = if (r.snooze != null) Notification.Action.Builder(null, if (r.duration != null) "Repeat" else "Snooze",
            AlarmRing.action(this, r, AlarmRing.SNOOZE)).build() else null
        val b = Notification.Builder(this, AlarmRing.RING).setSmallIcon(android.R.drawable.ic_lock_idle_alarm)
            .setContentTitle(r.title).setCategory(Notification.CATEGORY_ALARM).setOngoing(true)
            .setVisibility(Notification.VISIBILITY_PUBLIC)
            .addAction(Notification.Action.Builder(null, "Stop", AlarmRing.action(this, r, AlarmRing.STOP)).build())
            // From Android 14 a foreground service's notification can be swiped away while unlocked: that is Stop.
            .setDeleteIntent(AlarmRing.action(this, r, AlarmRing.STOP))
        second?.let { b.addAction(it) }
        packageManager.getLaunchIntentForPackage(packageName)?.let {
            b.setContentIntent(PendingIntent.getActivity(this, r.code, it, PendingIntent.FLAG_UPDATE_CURRENT or PendingIntent.FLAG_IMMUTABLE))
        }
        if (Build.VERSION.SDK_INT >= 34) b.setForegroundServiceBehavior(Notification.FOREGROUND_SERVICE_IMMEDIATE)
        return b.build()
    }

    /** The alarm sound the person chose, on the alarm stream, which Do Not Disturb lets through by default. */
    private fun play() {
        if (player != null) return
        val uri = RingtoneManager.getActualDefaultRingtoneUri(this, RingtoneManager.TYPE_ALARM)
            ?: RingtoneManager.getDefaultUri(RingtoneManager.TYPE_ALARM) ?: RingtoneManager.getDefaultUri(RingtoneManager.TYPE_RINGTONE)
        player = runCatching {
            MediaPlayer().apply {
                setAudioAttributes(AudioAttributes.Builder().setUsage(AudioAttributes.USAGE_ALARM).setContentType(AudioAttributes.CONTENT_TYPE_SONIFICATION).build())
                setDataSource(this@AlarmService, uri)
                isLooping = true
                prepare()
                start()
            }
        }.onFailure { android.util.Log.w("exact", "alarms: no alarm sound: $it") }.getOrNull()
    }

    /** One tag's ring ends; the service ends with the last. */
    fun end(tag: String) {
        val code = ringing.remove(tag) ?: return
        getSystemService(NotificationManager::class.java).cancel(code)
        if (ringing.isEmpty()) {
            stopForeground(STOP_FOREGROUND_REMOVE)
            stopSelf()
            return
        }
        if (code == foregroundCode) {
            val (next, nextCode) = ringing.entries.first().toPair()
            AlarmRecords.get(this, next)?.let { r ->
                foreground(nextCode, notification(r))
                foregroundCode = nextCode
            }
        }
    }

    /** `systemExempted` from Android 14; below it the service starts with no type, as `setAlarmClock` allows (D9). */
    private fun foreground(code: Int, n: Notification) {
        if (Build.VERSION.SDK_INT >= 34) startForeground(code, n, ServiceInfo.FOREGROUND_SERVICE_TYPE_SYSTEM_EXEMPTED)
        else startForeground(code, n)
    }

    override fun onDestroy() {
        player?.runCatching { stop(); release() }
        player = null
        main.removeCallbacksAndMessages(null)
        if (running === this) running = null
        super.onDestroy()
    }
}
