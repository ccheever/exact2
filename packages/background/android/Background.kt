// @exact/background on Android (LLP 1133.017 D2, D8, D12, D14, D17): WorkManager, the RFC's mechanism. The build makes
// one per host when the app binds the extension, outside the agent; under the agent the runner's substitute answers,
// unless the drive binds it (`--bind background`).
//
// - Registrations live in the module's preferences, so a tag outlives the process.
// - A `sync` is a `OneTimeWorkRequest` unique by tag with `NetworkType.CONNECTED` (D2), and fires at once while the
//   process runs and the network is up (`ConnectivityManager`'s default network). Each firing is its own wake
//   (`context.wake`), told its outcome; D1's retrying is the module's, as on Apple, the work enqueued again after the
//   backoff (`setInitialDelay`).
// - A `periodicsync` is a `PeriodicWorkRequest` unique by tag, its interval `minInterval` and its flex 5 minutes, so
//   the floor holds (D12's `flexInterval`).
// - When WorkManager runs [BackgroundWorker], the live session's module raises the tag's event and the worker returns
//   once its invocation ended. With no session in the process (the app's activity gone) the worker retries a `sync`
//   at WorkManager's backoff and skips a `periodicsync`: the headless session is not built (§14 stage 3), and a `sync`
//   still fires at the next launch.
// - The status is `denied` while `ActivityManager.isBackgroundRestricted()` is true (D14), with the standby bucket.
package exact.background

import android.app.ActivityManager
import android.app.usage.UsageStatsManager
import android.content.Context
import android.net.ConnectivityManager
import android.net.Network
import android.os.Handler
import android.os.Looper
import androidx.work.Constraints
import androidx.work.Data
import androidx.work.ExistingPeriodicWorkPolicy
import androidx.work.ExistingWorkPolicy
import androidx.work.NetworkType
import androidx.work.OneTimeWorkRequestBuilder
import androidx.work.PeriodicWorkRequestBuilder
import androidx.work.WorkManager
import androidx.work.Worker
import androidx.work.WorkerParameters
import dev.exact.canvas.ExactExtension
import dev.exact.canvas.ExtensionContext
import org.json.JSONArray
import org.json.JSONObject
import java.lang.ref.WeakReference
import java.util.concurrent.CountDownLatch
import java.util.concurrent.TimeUnit

class BackgroundModule(context: ExtensionContext) : ExactExtension(context) {
    companion object {
        private const val BACKOFF = 5 * 60_000.0
        private const val LONGEST = 60 * 60_000.0
        private const val ATTEMPTS = 3
        private const val FLOOR = 900_000.0
        /** The live session's module, which a worker raises its tag through. */
        @Volatile internal var live: WeakReference<BackgroundModule>? = null
        internal fun work(kind: String, tag: String) = "exact.background $kind $tag"
    }

    private class Registration(var registered: Double, var attempts: Int = 0, var notBefore: Double? = null,
                               var minInterval: Double? = null, var lastRun: Double? = null)

    private val main = Handler(Looper.getMainLooper())
    private val app: Context get() = context.application
    private val prefs get() = app.getSharedPreferences("exact.background", Context.MODE_PRIVATE)
    private val work get() = WorkManager.getInstance(app)
    /** By kind (`sync`, `periodicsync`), then tag. */
    private val tags = HashMap<String, HashMap<String, Registration>>()
    /** Invocations raised and not ended, and tags registered again during their own, as `kind tag`. */
    private val raised = HashSet<String>()
    private val again = HashSet<String>()
    private var online = false
    private var next = 0
    private val look = Runnable { fire() }
    private val network = object : ConnectivityManager.NetworkCallback() {
        override fun onAvailable(n: Network) { main.post { online = true; fire() } }
        override fun onLost(n: Network) { main.post { online = false } }
    }

    init {
        read()
        if (!context.agent) {
            live = WeakReference(this)
            app.getSystemService(ConnectivityManager::class.java).registerDefaultNetworkCallback(network)
        }
    }

    override fun later(name: String, args: JSONObject, reply: (JSONObject) -> Unit) {
        val tag = args.optString("tag")
        val bad = tag.isEmpty() || tag.toByteArray().size > 64
        when (name) {
            "registerSync" -> when {
                bad -> reply(error("TypeError"))
                denied -> reply(error("NotAllowedError"))
                else -> { register("sync", tag, null); reply(JSONObject()) }
            }
            "registerPeriodicSync" -> when {
                bad -> reply(error("TypeError"))
                denied -> reply(error("NotAllowedError"))
                else -> { register("periodicsync", tag, maxOf(args.optDouble("minInterval", FLOOR), FLOOR)); reply(JSONObject()) }
            }
            "unregisterPeriodicSync" -> {
                tags["periodicsync"]?.remove(tag)
                if (!context.agent) work.cancelUniqueWork(work("periodicsync", tag))
                keep("periodicsync")
                reply(JSONObject())
            }
            "syncTags" -> reply(JSONObject().put("exact:answer", listed("sync")))
            "periodicSyncTags" -> reply(JSONObject().put("exact:answer", listed("periodicsync")))
            "backgroundSyncStatus" -> reply(status())
            else -> reply(error("NotSupportedError"))
        }
    }

    override fun resumed() = context.changed("background.backgroundSyncStatus")

    private fun error(code: String) = JSONObject().put("error", code)

    // The fact

    private val restricted: Boolean get() = app.getSystemService(ActivityManager::class.java).isBackgroundRestricted
    private val denied: Boolean get() = restricted

    private fun status(): JSONObject {
        val bucket = when (app.getSystemService(UsageStatsManager::class.java).appStandbyBucket) {
            UsageStatsManager.STANDBY_BUCKET_ACTIVE -> "active"
            UsageStatsManager.STANDBY_BUCKET_WORKING_SET -> "working_set"
            UsageStatsManager.STANDBY_BUCKET_FREQUENT -> "frequent"
            UsageStatsManager.STANDBY_BUCKET_RARE -> "rare"
            UsageStatsManager.STANDBY_BUCKET_RESTRICTED -> "restricted"
            else -> "unknown"
        }
        return JSONObject().put("available", true).put("state", if (restricted) "denied" else "granted").put("periodic", true)
            .put("sync", true).put("android", JSONObject().put("standbyBucket", bucket).put("backgroundRestricted", restricted))
    }

    private fun listed(kind: String) = JSONArray().also { out ->
        tags[kind].orEmpty().toSortedMap().forEach { (tag, r) ->
            out.put(JSONObject().put("tag", tag).put("minInterval", r.minInterval ?: JSONObject.NULL).put("lastRun", r.lastRun ?: JSONObject.NULL))
        }
    }

    // Registrations

    private fun read() {
        val kept = try { JSONObject(prefs.getString("tags", "{}") ?: "{}") } catch (e: Exception) { JSONObject() }
        for (kind in kept.keys()) {
            val byTag = kept.getJSONObject(kind)
            tags[kind] = HashMap(byTag.keys().asSequence().associateWith { tag ->
                val o = byTag.getJSONObject(tag)
                fun num(k: String) = if (o.isNull(k)) null else o.optDouble(k)
                Registration(o.optDouble("registered"), o.optInt("attempts"), num("notBefore"), num("minInterval"), num("lastRun"))
            })
        }
    }

    private fun keep(kind: String) {
        val out = JSONObject()
        for ((k, byTag) in tags) out.put(k, JSONObject().also { o ->
            byTag.forEach { (tag, r) ->
                o.put(tag, JSONObject().put("registered", r.registered).put("attempts", r.attempts).put("notBefore", r.notBefore ?: JSONObject.NULL)
                    .put("minInterval", r.minInterval ?: JSONObject.NULL).put("lastRun", r.lastRun ?: JSONObject.NULL))
            }
        })
        prefs.edit().putString("tags", out.toString()).apply()
        context.changed(if (kind == "sync") "background.syncTags" else "background.periodicSyncTags")
    }

    private fun register(kind: String, tag: String, interval: Double?) {
        if ("$kind $tag" in raised) again += "$kind $tag"
        val r = tags[kind]?.get(tag) ?: Registration(context.now())
        r.minInterval = interval
        tags.getOrPut(kind) { HashMap() }[tag] = r
        keep(kind)
        enqueue(kind, tag, r)
        fire()
    }

    /** The tag's work: a `sync` once its backoff is over, with a network; a `periodicsync` at its interval. */
    private fun enqueue(kind: String, tag: String, r: Registration) {
        if (context.agent) return
        val connected = Constraints.Builder().setRequiredNetworkType(NetworkType.CONNECTED).build()
        val data = Data.Builder().putString("kind", kind).putString("tag", tag).build()
        if (kind == "sync") {
            val delay = maxOf(0.0, (r.notBefore ?: 0.0) - context.now()).toLong()
            val request = OneTimeWorkRequestBuilder<BackgroundWorker>().setConstraints(connected).setInputData(data)
                .setInitialDelay(delay, TimeUnit.MILLISECONDS).build()
            work.enqueueUniqueWork(work(kind, tag), ExistingWorkPolicy.REPLACE, request)
        } else {
            val request = PeriodicWorkRequestBuilder<BackgroundWorker>((r.minInterval ?: FLOOR).toLong(), TimeUnit.MILLISECONDS,
                5 * 60_000L, TimeUnit.MILLISECONDS).setConstraints(connected).setInputData(data).build()
            work.enqueueUniquePeriodicWork(work(kind, tag), ExistingPeriodicWorkPolicy.UPDATE, request)
        }
    }

    /** Each `sync` tag eligible while online, as an invocation of its own; a look again at the soonest backoff. */
    private fun fire() {
        if (!online || context.agent) return
        main.removeCallbacks(look)
        val now = context.now()
        var soonest = Double.POSITIVE_INFINITY
        for ((tag, r) in tags["sync"].orEmpty().toMap()) {
            if ("sync $tag" in raised) continue
            val at = r.notBefore
            if (at != null && at > now) { soonest = minOf(soonest, at); continue }
            raise("sync", tag, null)
        }
        if (soonest.isFinite()) main.postDelayed(look, (soonest - now).toLong())
    }

    /** Raise [tag]'s event as a wake of its own; [done] hears whether it completed. */
    private fun raise(kind: String, tag: String, done: ((Boolean) -> Unit)?) {
        val r = tags[kind]?.get(tag)
        val args = if (kind == "sync") JSONArray().put(tag).put(JSONObject().put("tag", tag).put("lastChance", (r?.attempts ?: 0) + 1 >= ATTEMPTS))
            else JSONArray().put(tag)
        raised += "$kind $tag"
        again -= "$kind $tag"
        context.wake(kind, args, "$kind $tag ${next++}") { outcome ->
            main.post {
                raised -= "$kind $tag"
                val registered = again.remove("$kind $tag")
                ended(kind, tag, outcome, registered)
                done?.invoke(outcome == "completed")
                fire()
            }
        }
    }

    /** WorkManager ran [tag]'s work: its event raised now, unless it is raised already or no longer registered. */
    internal fun ran(kind: String, tag: String, done: (Boolean) -> Unit) = main.post {
        val r = tags[kind]?.get(tag)
        when {
            r == null -> done(true)
            "$kind $tag" in raised -> done(true)
            kind == "sync" && (r.notBefore ?: 0.0) > context.now() -> done(true)
            else -> { context.log("background: WorkManager ran $kind $tag"); raise(kind, tag, done) }
        }
    }

    /** D1's rules for an invocation's end. */
    private fun ended(kind: String, tag: String, outcome: String, registered: Boolean) {
        val r = tags[kind]?.get(tag) ?: return
        if (kind == "periodicsync") {
            if (outcome == "completed") { r.lastRun = context.now(); r.attempts = 0 }
            return keep(kind)
        }
        if (outcome == "completed" && !registered || outcome == "unheard") {
            tags[kind]?.remove(tag)
            if (!context.agent) work.cancelUniqueWork(work(kind, tag))
            context.log("background.sync $tag: retired ($outcome)")
            return keep(kind)
        }
        r.attempts += 1
        if (r.attempts >= ATTEMPTS) {
            if (!registered) {
                tags[kind]?.remove(tag)
                if (!context.agent) work.cancelUniqueWork(work(kind, tag))
                return keep(kind)
            }
            r.attempts = 0
            r.notBefore = context.now() + LONGEST
        } else {
            r.notBefore = context.now() + minOf(BACKOFF * Math.pow(2.0, (r.attempts - 1).toDouble()), LONGEST)
        }
        keep(kind)
        enqueue(kind, tag, r)
    }
}

/**
 * The work WorkManager runs for a tag, on its own thread: the live session's module raises the tag's event, and the
 * work ends once its invocation ended, within WorkManager's 10 minutes. No session in the process: a `sync` is retried
 * at WorkManager's backoff and a `periodicsync` waits for its next period.
 */
class BackgroundWorker(context: Context, params: WorkerParameters) : Worker(context, params) {
    override fun doWork(): Result {
        val kind = inputData.getString("kind") ?: return Result.failure()
        val tag = inputData.getString("tag") ?: return Result.failure()
        val module = BackgroundModule.live?.get() ?: return if (kind == "sync") Result.retry() else Result.success()
        val ended = CountDownLatch(1)
        var completed = false
        module.ran(kind, tag) { ok -> completed = ok; ended.countDown() }
        if (!ended.await(9, TimeUnit.MINUTES)) return Result.retry()
        return if (completed || kind == "periodicsync") Result.success() else Result.retry()
    }
}
