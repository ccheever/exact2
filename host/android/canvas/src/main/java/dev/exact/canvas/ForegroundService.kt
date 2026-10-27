package dev.exact.canvas

import android.app.Notification
import android.app.NotificationChannel
import android.app.NotificationManager
import android.app.PendingIntent
import android.app.Service
import android.content.ComponentName
import android.content.Context
import android.content.Intent
import android.content.pm.PackageManager
import android.content.pm.ServiceInfo
import android.os.Build
import android.os.IBinder
import android.util.Log
import org.json.JSONArray
import org.json.JSONObject

/**
 * The host's foreground services (LLP 1133.033 D2): one service class per kind of claim, declared in the app's
 * manifest only when a granted line derives it (`device.microphone-background` declares [Capture]), with every type it
 * promotes as and its purpose's text as `dev.exact.purpose` meta-data. Packages never start one: they claim it through
 * [ExtensionContext.foregroundService], and the service runs while any claim of its kind is held.
 */
open class ForegroundService : Service() {
    /** A capture that outlives the screen: type `microphone`. */
    class Capture : ForegroundService()

    /** A call: types `phoneCall` and `microphone` (D11). */
    class Call : ForegroundService()

    internal val kind: String get() = if (this is Call) "call" else "capture"

    override fun onBind(intent: Intent?): IBinder? = null

    override fun onStartCommand(intent: Intent?, flags: Int, startId: Int): Int {
        Claims.started(this)
        // Not sticky: a process the system makes again holds no claim, and a service it would restart has none to show.
        return START_NOT_STICKY
    }

    override fun onDestroy() {
        Claims.destroyed(this)
        super.onDestroy()
    }
}

/**
 * A package's hold on a foreground service of [kind] ([ForegroundServices.claim]). Released by [release] or with the
 * process; the service stops with the last claim of its kind.
 */
class ForegroundClaim internal constructor(
    internal val application: Context,
    val kind: String,
    types: Set<String>,
    val origin: String,
    internal val extension: String,
    internal var notification: (Notification.Builder) -> Unit,
    internal val lost: (String) -> Unit,
    internal val agent: Boolean,
) {
    /** When it was made, ms since the epoch: the notification's chronometer counts from it (D4). */
    val since: Long = System.currentTimeMillis()

    var types: Set<String> = types
        private set

    internal var released = false

    /** Promote the service again with [types], which Android allows to add types (D2). */
    fun types(types: Set<String>) {
        if (released || types == this.types) return
        this.types = types
        Claims.changed(this)
    }

    /** The notification changed: [build] sets its parts again over the module's. */
    fun update(build: (Notification.Builder) -> Unit) {
        if (released) return
        notification = build
        Claims.changed(this)
    }

    fun release() {
        if (released) return
        released = true
        Claims.release(this)
    }
}

/** What [ExtensionContext.foregroundService] gives a package: claims on the host's services (LLP 1133.033 D2). */
class ForegroundServices internal constructor(private val application: Context, private val extension: String, private val agent: () -> Boolean) {
    /** Whether the app's lines declare a service of [kind]: the manifest has its class. */
    fun declared(kind: String): Boolean = Claims.declared(application, kind)

    /**
     * Under the agent, the platform's refusal every claim meets, as the drive prefers it (`prefer extension
     * <extension>.foregroundService={"refused": "…"}`, D21); null for none. The implementation sets it from its
     * preferred values.
     */
    var refusing: String? = null

    /**
     * Claim a service of [kind] with [types] for a capability started by [origin]. The service starts with the first
     * claim of its kind and promotes itself with the union of its claims' types. A start needs the reader resumed,
     * unless [permitted] (the platform event that allows it: a notification's action, a high-priority push); a claim
     * joining a running service needs nothing. Null, with the reason in the log, when the line was not granted or the
     * platform refuses the start; [lost] hears a refusal that comes after (the service's `startForeground` threw).
     * [notification] sets the claimant's parts of the notification after the module's own (D4). Under the agent no
     * service starts and nothing posts: the claim is told the runner, which lists it in `state.extensions`.
     */
    fun claim(kind: String, types: Set<String>, origin: String, permitted: Boolean = false,
              notification: (Notification.Builder) -> Unit = {}, lost: (String) -> Unit = {}): ForegroundClaim? {
        val agent = agent()
        val refuse = { why: String -> Log.i("exact", "foreground service ${types.joinToString("|")}: refused: $why"); null }
        if (!declared(kind)) return refuse("the app's lines declare no $kind service")
        // The runner prefers a refusal under the agent: `prefer extension <extension>.foregroundService={"refused":…}`.
        if (agent) refusing?.let { why -> Claims.told(application, extension, kind, types, false, why); return refuse(why) }
        val claim = ForegroundClaim(application, kind, types, origin, extension, notification, lost, agent)
        if (agent) {
            Claims.told(application, extension, kind, types, true, null)
            return claim.also { Claims.held += it }
        }
        if ("microphone" in types && application.checkSelfPermission(android.Manifest.permission.RECORD_AUDIO) != PackageManager.PERMISSION_GRANTED) {
            return refuse("SecurityException: RECORD_AUDIO is not granted")
        }
        val running = Claims.held.any { it.kind == kind && !it.agent }
        if (!running && !Extensions.resumedNow && !permitted) return refuse("ForegroundServiceStartNotAllowedException: the app is not in front")
        Claims.held += claim
        if (running) {
            Claims.changed(claim)
            Log.i("exact", "foreground service ${types.joinToString("|")}: claimed")
            return claim
        }
        return try {
            application.startForegroundService(Intent(application, Claims.service(kind)))
            Log.i("exact", "foreground service ${types.joinToString("|")}: claimed")
            claim
        } catch (e: Exception) {
            // `ForegroundServiceStartNotAllowedException` (an `IllegalStateException`) or a `SecurityException`.
            Claims.held -= claim
            refuse("${e.javaClass.simpleName}: ${e.message}")
        }
    }
}

/** Every claim in the process, and the services that carry them. */
internal object Claims {
    val held = ArrayList<ForegroundClaim>()
    private val services = HashMap<String, ForegroundService>()
    private val ids = mapOf("capture" to 0x45584301, "call" to 0x45584302)

    fun service(kind: String): Class<out ForegroundService> = if (kind == "call") ForegroundService.Call::class.java else ForegroundService.Capture::class.java

    fun declared(application: Context, kind: String): Boolean = try {
        application.packageManager.getServiceInfo(ComponentName(application, service(kind)), 0); true
    } catch (e: PackageManager.NameNotFoundException) { false }

    /** The purpose's text the build wrote beside the service: the notification's title and channel's name. */
    fun purpose(application: Context, kind: String): String = try {
        application.packageManager.getServiceInfo(ComponentName(application, service(kind)), PackageManager.GET_META_DATA)
            .metaData?.getString("dev.exact.purpose")
    } catch (e: PackageManager.NameNotFoundException) { null } ?: application.applicationInfo.loadLabel(application.packageManager).toString()

    /** A claim under the agent, made or released, said to the runner through the extension's `changed`. */
    fun told(application: Context, extension: String, kind: String, types: Set<String>, claimed: Boolean, refused: String?) {
        val body = JSONObject().put("extension", extension).put("kind", kind).put("types", JSONArray(types.toList()))
            .put("claimed", claimed).put("text", purpose(application, kind))
        refused?.let { body.put("refused", it) }
        Exact.changed("exact-foreground-service:$body")
    }

    /** The union of the live claims' types for [kind], as `startForeground`'s mask. */
    private fun mask(kind: String): Int {
        val types = held.filter { it.kind == kind && !it.agent }.flatMap { it.types }.toSet()
        var mask = 0
        if ("phoneCall" in types) mask = mask or ServiceInfo.FOREGROUND_SERVICE_TYPE_PHONE_CALL
        if ("microphone" in types && Build.VERSION.SDK_INT >= 30) mask = mask or ServiceInfo.FOREGROUND_SERVICE_TYPE_MICROPHONE
        return mask
    }

    /** The service's notification: the oldest live claim's (D2), the module's parts first, then the claimant's (D4). */
    private fun notification(service: ForegroundService): Notification {
        val kind = service.kind
        val title = purpose(service, kind)
        val manager = service.getSystemService(NotificationManager::class.java)
        val channel = "exact.$kind"
        // A channel's importance cannot be raised once made: a call's rings (D11), a capture's is silent.
        if (manager.getNotificationChannel(channel) == null) {
            val importance = if (kind == "call") NotificationManager.IMPORTANCE_HIGH else NotificationManager.IMPORTANCE_LOW
            manager.createNotificationChannel(NotificationChannel(channel, title, importance))
        }
        val launch = service.packageManager.getLaunchIntentForPackage(service.packageName)
        val tap = launch?.let { PendingIntent.getActivity(service, 0, it, PendingIntent.FLAG_IMMUTABLE or PendingIntent.FLAG_UPDATE_CURRENT) }
        val icon = service.applicationInfo.icon.takeIf { it != 0 } ?: android.R.drawable.ic_btn_speak_now
        val builder = Notification.Builder(service, channel).setSmallIcon(icon).setContentTitle(title)
            .setCategory(Notification.CATEGORY_SERVICE).setOngoing(true).setContentIntent(tap)
        if (Build.VERSION.SDK_INT >= 31) builder.setForegroundServiceBehavior(Notification.FOREGROUND_SERVICE_IMMEDIATE)
        held.firstOrNull { it.kind == kind && !it.agent }?.let { claim ->
            builder.setWhen(claim.since).setUsesChronometer(true).setShowWhen(true)
            claim.notification(builder)
        }
        return builder.build()
    }

    /** The service of [service]'s kind was started: it promotes itself at once, as Android requires. */
    fun started(service: ForegroundService) {
        services[service.kind] = service
        promote(service)
        // A start with no claim (a process made again, or every claim released before it ran) promotes and stops.
        if (held.none { it.kind == service.kind && !it.agent }) stop(service)
    }

    private fun promote(service: ForegroundService) {
        try {
            if (Build.VERSION.SDK_INT >= 29) service.startForeground(ids.getValue(service.kind), notification(service), mask(service.kind))
            else service.startForeground(ids.getValue(service.kind), notification(service))
        } catch (e: Exception) {
            // A `SecurityException` for a type whose permission or runtime grant is missing, or Android 12's
            // `ForegroundServiceStartNotAllowedException`: every claim of the kind is lost, and the capability degrades.
            val why = "${e.javaClass.simpleName}: ${e.message}"
            Log.i("exact", "foreground service ${service.kind}: refused: $why")
            val lost = held.filter { it.kind == service.kind && !it.agent }
            held.removeAll(lost.toSet())
            lost.forEach { it.released = true; it.lost(why) }
            stop(service)
        }
    }

    /** A claim's types or notification changed. */
    fun changed(claim: ForegroundClaim) {
        if (claim.agent) return
        services[claim.kind]?.let { promote(it) }
    }

    fun release(claim: ForegroundClaim) {
        held -= claim
        if (claim.agent) return told(claim.application, claim.extension, claim.kind, claim.types, false, null)
        Log.i("exact", "foreground service ${claim.types.joinToString("|")}: released")
        val service = services[claim.kind] ?: return
        if (held.none { it.kind == claim.kind && !it.agent }) stop(service) else promote(service)
    }

    private fun stop(service: ForegroundService) {
        service.stopForeground(Service.STOP_FOREGROUND_REMOVE)
        service.stopSelf()
    }

    fun destroyed(service: ForegroundService) {
        if (services[service.kind] === service) services.remove(service.kind)
    }
}
