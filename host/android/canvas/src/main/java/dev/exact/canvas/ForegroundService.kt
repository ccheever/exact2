package dev.exact.canvas

import android.app.Activity
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
import android.os.Handler
import android.os.IBinder
import android.os.Looper
import android.util.Log
import org.json.JSONArray
import org.json.JSONObject

/**
 * The host's foreground services (LLP 1133.033 D2): one service class per kind of claim, declared in the app's
 * manifest only when a granted line derives it (`device.microphone-background` declares [Capture] with `microphone`),
 * with every type its lines give it and its purpose's text as `dev.exact.purpose` meta-data. Packages never start one: they claim it through
 * [ExtensionContext.foregroundService], and the service runs while any claim of its kind is held.
 */
open class ForegroundService : Service() {
    /** A capture that outlives the screen: `microphone`, and `mediaProjection` for a screen (LLP 1133.032 D11 item 3). */
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
 * The platform event that permits a claim to start its service while the reader is not in front (LLP 1133.033 D2).
 * Android 12 and later refuse a start from the background except after such events
 * ([restrictions](https://developer.android.com/develop/background-work/services/fgs/restrictions-bg-start)).
 */
sealed class ForegroundPermit {
    /** The person acted on a notification's action (D11). */
    object NotificationAction : ForegroundPermit()

    /** A high-priority push arrived (D12). */
    object Push : ForegroundPermit()

    /**
     * MediaProjection's consent, as `createScreenCaptureIntent()`'s result delivered it (LLP 1133.032 D11 item 3). A
     * claim with the `mediaProjection` type needs it, since Android forbids that service before consent
     * ([service types](https://developer.android.com/develop/background-work/services/fgs/service-types#media-projection));
     * [ForegroundClaim.permit] keeps it for `getMediaProjection` in [ForegroundServices.claim]'s `promoted`.
     */
    class Consent(val resultCode: Int, val data: Intent) : ForegroundPermit()
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
    /** The event that permitted the start, kept for the caller (a [ForegroundPermit.Consent]'s result). */
    val permit: ForegroundPermit?,
    internal val extension: String,
    internal var notification: (Notification.Builder) -> Unit,
    internal val lost: (String) -> Unit,
    internal val promoted: (ForegroundClaim) -> Unit,
    internal val agent: Boolean,
) {
    /** When it was made, ms since the epoch: the notification's chronometer counts from it (D4). */
    val since: Long = System.currentTimeMillis()

    var types: Set<String> = types
        private set

    internal var released = false

    /** The types the service last promoted with that [promoted] was told of: it hears each new set once. */
    internal var told: Set<String> = emptySet()

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

    /** Whether the app's lines declare a service of [kind] with [type] among its types (`microphone`, …). */
    fun declared(kind: String, type: String): Boolean = Claims.declared(application, kind, type)

    /**
     * Under the agent, the platform's refusal every claim meets, as the drive prefers it (`prefer extension
     * <extension>.foregroundService={"refused": "…"}`, D21); null for none. The implementation sets it from its
     * preferred values.
     */
    var refusing: String? = null

    /**
     * Claim a service of [kind] with [types] for a capability started by [origin]. The service starts with the first
     * claim of its kind and promotes itself with the union of its claims' types. A start needs the reader resumed,
     * unless [permit] names the platform event that allows it; a claim joining a running service needs nothing. A
     * `mediaProjection` claim needs a [ForegroundPermit.Consent] whatever the reader's state. Null, with the reason in
     * the log, when the lines do not declare the kind with every type or the platform refuses the start; [lost] hears a
     * refusal that comes after (the service's `startForeground` threw). [promoted] runs on the main thread once the
     * service has called `startForeground` with every one of [types] (at once when it joins a service promoted with
     * them), never inside this call: work the platform allows only in a promoted service, such as
     * `getMediaProjection`, starts there. [notification] sets the claimant's parts of the notification after the
     * module's own (D4). Under the agent no service starts and nothing posts: the claim is told the runner, which lists
     * it in `state.extensions`, and [promoted] runs as soon as the main thread takes it.
     */
    fun claim(kind: String, types: Set<String>, origin: String, permit: ForegroundPermit? = null,
              notification: (Notification.Builder) -> Unit = {}, lost: (String) -> Unit = {},
              promoted: (ForegroundClaim) -> Unit = {}): ForegroundClaim? {
        val agent = agent()
        val refuse = { why: String -> Log.i("exact", "foreground service ${types.joinToString("|")}: refused: $why"); null }
        if (!declared(kind)) return refuse("the app's lines declare no $kind service")
        types.firstOrNull { !declared(kind, it) }?.let { return refuse("the app's lines declare no $it type on the $kind service") }
        // The runner prefers a refusal under the agent: `prefer extension <extension>.foregroundService={"refused":…}`.
        if (agent) refusing?.let { why -> Claims.told(application, extension, kind, types, false, why); return refuse(why) }
        if ("mediaProjection" in types && (permit as? ForegroundPermit.Consent)?.resultCode != Activity.RESULT_OK) {
            return refuse("SecurityException: mediaProjection needs the consent's result")
        }
        val claim = ForegroundClaim(application, kind, types, origin, permit, extension, notification, lost, promoted, agent)
        if (agent) {
            Claims.told(application, extension, kind, types, true, null)
            Claims.held += claim
            Claims.promoted(listOf(claim), types)
            return claim
        }
        if ("microphone" in types && application.checkSelfPermission(android.Manifest.permission.RECORD_AUDIO) != PackageManager.PERMISSION_GRANTED) {
            return refuse("SecurityException: RECORD_AUDIO is not granted")
        }
        val running = Claims.held.any { it.kind == kind && !it.agent }
        if (!running && !Extensions.resumedNow && permit == null) return refuse("ForegroundServiceStartNotAllowedException: the app is not in front")
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

    /** Whether the manifest declares [kind]'s service with [type] in its `foregroundServiceType`. */
    fun declared(application: Context, kind: String, type: String): Boolean = try {
        val bit = bit(type)
        bit != 0 && application.packageManager.getServiceInfo(ComponentName(application, service(kind)), 0).foregroundServiceType and bit == bit
    } catch (e: PackageManager.NameNotFoundException) { false }

    /** A type's `ServiceInfo` bit; 0 for one this host does not run. `microphone` is API 30's. */
    private fun bit(type: String): Int = when (type) {
        "phoneCall" -> ServiceInfo.FOREGROUND_SERVICE_TYPE_PHONE_CALL
        "mediaProjection" -> ServiceInfo.FOREGROUND_SERVICE_TYPE_MEDIA_PROJECTION
        "microphone" -> if (Build.VERSION.SDK_INT >= 30) ServiceInfo.FOREGROUND_SERVICE_TYPE_MICROPHONE else 0
        else -> 0
    }

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

    /** The union of the live claims' types for [kind]. */
    private fun types(kind: String): Set<String> = held.filter { it.kind == kind && !it.agent }.flatMap { it.types }.toSet()

    /** [claims]' `promoted`, on the main thread, for each whose types [types] now covers and that has not heard them. */
    fun promoted(claims: List<ForegroundClaim>, types: Set<String>) {
        val due = claims.filter { !it.released && types.containsAll(it.types) && it.told != it.types }
        due.forEach { it.told = it.types }
        if (due.isNotEmpty()) Handler(Looper.getMainLooper()).post { due.filter { !it.released }.forEach { it.promoted(it) } }
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
            val types = types(service.kind)
            service.startForeground(ids.getValue(service.kind), notification(service), types.fold(0) { mask, type -> mask or bit(type) })
            promoted(held.filter { it.kind == service.kind && !it.agent }, types)
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
        if (claim.agent) return promoted(listOf(claim), claim.types)
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
