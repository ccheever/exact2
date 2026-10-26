// @exact/health on Android (LLP 1133.026 D5, D6, D8, D10, D11): Health Connect through the Jetpack client
// (`androidx.health.connect:connect-client`, §16 question 6), answered in the interface's shapes.
//
// - `available` is `getSdkStatus` equal to `SDK_AVAILABLE`; otherwise `android.sdkStatus` says whether the provider is
//   missing or needs an update (D5).
// - Each type's `read` comes from `getGrantedPermissions`, and the one Android permission history (LLP 1133.008 D8
//   item 7) keyed per permission: a type the person declined twice reads `denied`, since Health Connect shows nothing
//   on a third request; one declined once, or revoked in Health Connect, reads `prompt`.
// - A granted type is `complete` from `since`: 30 days before now without `READ_HEALTH_DATA_HISTORY`, the one window
//   that holds however the first grant moved (D5); none with it.
// - The request starts the client's permission contract for the types still `prompt` and history when asked (D6).
// - Statistics are `aggregateGroupByDuration` for hours and `aggregateGroupByPeriod` for days, weeks and months, in the
//   device's zone. Workouts are every `ExerciseSessionRecord` in the range, paged by the interface's `start:id` cursor.
// - Health Connect has no observer: the grants are read again at each resume, and a change, or a return to the
//   foreground, asks every reader again (D8).
// - Under the agent no client is made: `AgentHealth` answers over the fixture with the states the runner told.
// - Tier 3 (D11): `HealthModule.hatch` is given the client once, when it is made.
package exact.health

import android.content.Context
import android.os.Handler
import android.os.Looper
import androidx.health.connect.client.HealthConnectClient
import androidx.health.connect.client.PermissionController
import androidx.health.connect.client.aggregate.AggregateMetric
import androidx.health.connect.client.aggregate.AggregationResult
import androidx.health.connect.client.permission.HealthPermission
import androidx.health.connect.client.records.ActiveCaloriesBurnedRecord
import androidx.health.connect.client.records.DistanceRecord
import androidx.health.connect.client.records.ExerciseSessionRecord
import androidx.health.connect.client.records.StepsRecord
import androidx.health.connect.client.records.metadata.DataOrigin
import androidx.health.connect.client.request.AggregateGroupByDurationRequest
import androidx.health.connect.client.request.AggregateGroupByPeriodRequest
import androidx.health.connect.client.request.AggregateRequest
import androidx.health.connect.client.request.ReadRecordsRequest
import androidx.health.connect.client.time.TimeRangeFilter
import androidx.health.connect.client.units.Energy
import androidx.health.connect.client.units.Length
import dev.exact.canvas.ExactExtension
import dev.exact.canvas.ExtensionContext
import org.json.JSONArray
import org.json.JSONObject
import java.time.Duration
import java.time.Instant
import java.time.Period
import java.time.ZoneId
import kotlin.coroutines.Continuation
import kotlin.coroutines.EmptyCoroutineContext
import kotlin.coroutines.startCoroutine

class HealthModule(context: ExtensionContext) : ExactExtension(context) {
    companion object {
        /** The app's access hatch (D11): given the client once, when it is made. */
        @JvmStatic var hatch: ((HealthConnectClient) -> Unit)? = null

        private const val PROVIDER = "com.google.android.apps.healthdata"
        private const val MONTH_MS = 30 * 86_400_000.0
        private val NAMES = listOf("healthStatus", "healthStatistics", "healthWorkouts")
        private const val HISTORY = HealthPermission.PERMISSION_READ_HEALTH_DATA_HISTORY

        /** Tier 1's types (D1): each one's permission, its record class's name and its aggregate. */
        private val PERMISSIONS = mapOf(
            "steps" to HealthPermission.getReadPermission(StepsRecord::class),
            "distance" to HealthPermission.getReadPermission(DistanceRecord::class),
            "activeEnergy" to HealthPermission.getReadPermission(ActiveCaloriesBurnedRecord::class),
            "workouts" to HealthPermission.getReadPermission(ExerciseSessionRecord::class),
            "history" to HISTORY,
        )
        private val RECORDS = mapOf("StepsRecord" to "steps", "DistanceRecord" to "distance", "ActiveCaloriesBurnedRecord" to "activeEnergy")
        private val UNITS = mapOf("steps" to "count", "distance" to "m", "activeEnergy" to "kcal")

        private fun metric(type: String): AggregateMetric<*>? = when (type) {
            "steps" -> StepsRecord.COUNT_TOTAL
            "distance" -> DistanceRecord.DISTANCE_TOTAL
            "activeEnergy" -> ActiveCaloriesBurnedRecord.ACTIVE_CALORIES_TOTAL
            else -> null
        }

        private fun value(result: AggregationResult, metric: AggregateMetric<*>): Double? = when (val v = result[metric]) {
            is Long -> v.toDouble()
            is Length -> v.inMeters
            is Energy -> v.inKilocalories
            else -> null
        }

        private val ACTIVITIES = mapOf(
            ExerciseSessionRecord.EXERCISE_TYPE_WALKING to "walking",
            ExerciseSessionRecord.EXERCISE_TYPE_RUNNING to "running",
            ExerciseSessionRecord.EXERCISE_TYPE_RUNNING_TREADMILL to "running",
            ExerciseSessionRecord.EXERCISE_TYPE_HIKING to "hiking",
            ExerciseSessionRecord.EXERCISE_TYPE_BIKING to "cycling",
            ExerciseSessionRecord.EXERCISE_TYPE_BIKING_STATIONARY to "cycling",
            ExerciseSessionRecord.EXERCISE_TYPE_SWIMMING_POOL to "swimming",
            ExerciseSessionRecord.EXERCISE_TYPE_SWIMMING_OPEN_WATER to "swimming",
        )

        /** D5's summary over the data types' `read` words; history's denial stays out of it. */
        private fun summary(types: List<JSONObject>): String {
            val reads = types.map { it.optString("read") }
            val data = types.filter { it.optString("type") != "history" }.map { it.optString("read") }
            return when {
                "prompt" in reads -> "prompt"
                "unknown" in data -> "unknown"
                data.isNotEmpty() && data.all { it == "denied" } -> "denied"
                else -> "granted"
            }
        }

        /** One page after the key cursor `after` (`<start>:<id>`), D8. */
        internal fun page(all: List<JSONObject>, after: String?, limit: Int, since: Double?, complete: Boolean): JSONObject {
            var rest = all
            if (after != null) {
                val cut = after.indexOf(':')
                val at = if (cut > 0) after.substring(0, cut).toDoubleOrNull() else null
                val id = if (cut > 0) after.substring(cut + 1) else ""
                if (at == null || all.none { it.optDouble("start") == at && it.optString("id") == id }) {
                    return JSONObject().put("error", "InvalidStateError").put("message", "cursor-stale")
                }
                rest = all.dropWhile { val s = it.optDouble("start"); s < at || (s == at && it.optString("id") <= id) }
            }
            val shown = rest.take(limit)
            val next: Any = if (rest.size > limit) shown.last().let { "${it.optDouble("start").toLong()}:${it.optString("id")}" } else JSONObject.NULL
            return JSONObject().put("workouts", JSONArray(shown)).put("next", next).put("since", since ?: JSONObject.NULL).put("complete", complete)
        }
    }

    private val app: Context = context.application
    private val main = Handler(Looper.getMainLooper())
    private var destroyed = false
    private val client: HealthConnectClient by lazy { HealthConnectClient.getOrCreate(app).also { c -> hatch?.invoke(c) } }
    private val agentStore by lazy { AgentHealth(app.assets).also { a -> a.missing?.let(context::log) } }
    /** What the runner told, under the agent: each permission's state. */
    private var told = JSONObject()
    /** The grants read last, to announce a change found at a resume. */
    private var granted: Set<String>? = null
    private var announcing = false
    private var shown = true

    override fun later(name: String, args: JSONObject, reply: (JSONObject) -> Unit) {
        val once: (JSONObject) -> Unit = { out -> main.post { reply(out) } }
        when (name) {
            "healthStatus" -> status(args, once)
            "requestHealthAuthorization" -> request(args, once)
            "healthStatistics" -> statistics(args, once)
            "healthWorkouts" -> workouts(args, once)
            else -> once(JSONObject().put("error", "NotSupportedError"))
        }
    }

    override fun permissions(states: JSONObject) { told = states }

    override fun preferred(values: JSONObject) {
        if (!context.agent) return
        values.optString("store").takeIf { it.isNotEmpty() }?.let { file ->
            val name = file.substringAfterLast('/')
            context.log(if (agentStore.load(name)) "health: store $name" else "health: no fixture $name")
        }
        values.optString("now").takeIf { it.isNotEmpty() }?.let { agentStore.now = AgentHealth.local(it, agentStore.zone) }
        // On the next turn, so a drive's next read sees the swap.
        main.post { NAMES.forEach { context.changed("health.$it") } }
    }

    override fun foreground(shown: Boolean) {
        if (shown && !this.shown) announce()
        this.shown = shown
    }

    override fun resumed() {
        if (context.agent || sdkStatus() != "available") return
        call({ client.permissionController.getGrantedPermissions() }) { r ->
            val now = r.getOrNull() ?: return@call
            if (granted != null && now != granted) announce()
            granted = now
        }
    }

    override fun destroy(done: () -> Unit) {
        destroyed = true
        done()
    }

    /**
     * [block], the client's suspend calls, then [done] on the main thread with its result, never after [destroy]. The
     * client runs its work on its own threads, so the standard library's coroutine start is all a call needs.
     */
    private fun <T> call(block: suspend () -> T, done: (Result<T>) -> Unit) =
        block.startCoroutine(Continuation(EmptyCoroutineContext) { r -> main.post { if (!destroyed) done(r) } })

    /** Every reader asked again, coalesced (D8). */
    private fun announce() {
        if (announcing) return
        announcing = true
        main.postDelayed({
            announcing = false
            NAMES.forEach { context.changed("health.$it") }
        }, 250)
    }

    private fun sdkStatus(): String = when (HealthConnectClient.getSdkStatus(app, PROVIDER)) {
        HealthConnectClient.SDK_AVAILABLE -> "available"
        HealthConnectClient.SDK_UNAVAILABLE_PROVIDER_UPDATE_REQUIRED -> "providerUpdateRequired"
        else -> "unavailable"
    }

    /** The types a status or request names: its `read` list, and `history` when it writes `android.readHealthDataHistory`. */
    private fun named(args: JSONObject): List<String> {
        val read = args.optJSONArray("read")?.let { a -> (0 until a.length()).map(a::getString) }.orEmpty().filter { it in PERMISSIONS }
        return if (args.optJSONObject("android")?.optBoolean("readHealthDataHistory") == true) read + "history" else read
    }

    // MARK: status and the request

    /** Each type's `read`, `since` and `complete` (D5), from the grants or, under the agent, from what was told. */
    private fun types(names: List<String>, grants: Set<String>, now: Double): List<JSONObject> {
        val history = app.getSharedPreferences("exact.permissions", Context.MODE_PRIVATE)
        val historyGranted = if (context.agent) told.optString("health.history") == "granted" else HISTORY in grants
        return names.map { t ->
            val permission = PERMISSIONS.getValue(t)
            val read = if (context.agent) told.optString("health.$t", "prompt").takeIf { it in setOf("granted", "denied") } ?: "prompt"
                else if (permission in grants) "granted" else if (history.getBoolean(permission, false)) "denied" else "prompt"
            if (!context.agent && read == "granted") history.edit().remove(permission).remove("$permission#once").apply()
            val since: Any = if (read == "granted" && t != "history" && !historyGranted) now - MONTH_MS else JSONObject.NULL
            JSONObject().put("type", t).put("read", read).put("since", since).put("complete", read == "granted")
                .put("android", JSONObject()
                    .put("shouldShowRationale", read != "granted" && context.activity?.shouldShowRequestPermissionRationale(permission) == true)
                    .put("permanentlyDenied", read == "denied"))
        }
    }

    private fun answer(types: List<JSONObject>): JSONObject =
        JSONObject().put("available", true).put("state", summary(types)).put("types", JSONArray(types))
            .put("apple", JSONObject.NULL).put("android", JSONObject().put("sdkStatus", "available"))

    private fun unavailable(sdk: String): JSONObject =
        JSONObject().put("available", false).put("state", "unknown").put("types", JSONArray())
            .put("apple", JSONObject.NULL).put("android", JSONObject().put("sdkStatus", sdk))

    /** The grants now, then [done] with them; a failure answers [reply]. */
    private fun grants(reply: (JSONObject) -> Unit, done: (Set<String>) -> Unit) {
        if (context.agent) return done(emptySet())
        val sdk = sdkStatus()
        if (sdk != "available") return reply(unavailable(sdk))
        call({ client.permissionController.getGrantedPermissions() }) { r ->
            val now = r.getOrElse { return@call reply(failure(it)) }
            granted = now
            done(now)
        }
    }

    private fun status(args: JSONObject, reply: (JSONObject) -> Unit) =
        grants(reply) { g -> reply(answer(types(named(args), g, context.now()))) }

    private fun request(args: JSONObject, reply: (JSONObject) -> Unit) {
        // Under the agent the runner held the ticket and stored its answer.
        if (context.agent) return status(args, reply)
        grants(reply) { g ->
            val asking = types(named(args), g, context.now()).filter { it.optString("read") == "prompt" }.map { PERMISSIONS.getValue(it.optString("type")) }.toSet()
            if (asking.isEmpty()) return@grants status(args, reply)
            val contract = PermissionController.createRequestPermissionResultContract(PROVIDER)
            context.startActivityForResult(contract.createIntent(app, asking)) { code, data ->
                val given = contract.parseResult(code, data)
                // A permission declined twice is not shown again: the history marks it, the type reads `denied`.
                val history = app.getSharedPreferences("exact.permissions", Context.MODE_PRIVATE)
                val edit = history.edit()
                for (p in asking - given) if (history.getBoolean("$p#once", false)) edit.putBoolean(p, true) else edit.putBoolean("$p#once", true)
                edit.apply()
                announce()
                status(args, reply)
            }
        }
    }

    // MARK: reads

    /** What a failed call answers. */
    private fun failure(e: Throwable): JSONObject = when (e) {
        is SecurityException -> JSONObject().put("error", "NotAllowedError").put("message", "requestHealthAuthorization")
        is IllegalStateException -> JSONObject().put("error", "NotAllowedError").put("message", e.message ?: "Health Connect refused the read")
        else -> JSONObject().put("error", "NotReadableError").put("message", e.message ?: "no answer")
    }

    /** [type]'s state; a failure for `prompt` and `denied`, else [done] with its `since` and the grants. */
    private fun admitted(type: String, reply: (JSONObject) -> Unit, done: (Double?, Set<String>) -> Unit) = grants(reply) { g ->
        val state = types(listOf(type), g, context.now()).first()
        when (state.optString("read")) {
            "granted" -> done(state.opt("since") as? Double, g)
            "denied" -> reply(JSONObject().put("error", "NotAllowedError"))
            else -> reply(JSONObject().put("error", "NotAllowedError").put("message", "requestHealthAuthorization"))
        }
    }

    private fun origins(args: JSONObject): Set<DataOrigin> =
        args.optJSONObject("android")?.optJSONArray("dataOrigins")?.let { a -> (0 until a.length()).map { DataOrigin(a.getString(it)) }.toSet() }.orEmpty()

    private fun statistics(args: JSONObject, reply: (JSONObject) -> Unit) {
        val android = args.optJSONObject("android")
        val type = android?.optString("recordType")?.takeIf { it.isNotEmpty() }?.let { RECORDS[it] ?: return reply(JSONObject().put("error", "NotSupportedError")) }
            ?: args.optString("type", "steps").ifEmpty { "steps" }
        val metric = metric(type) ?: return reply(JSONObject().put("error", "NotSupportedError"))
        val start = args.optDouble("start")
        val end = args.optDouble("end")
        val interval = args.optString("interval")
        if (start.isNaN() || end.isNaN() || end <= start || interval.isEmpty()) return reply(JSONObject().put("error", "TypeError").put("message", "start, end and interval"))
        val zone = if (context.agent) agentStore.zone else ZoneId.systemDefault()
        val edges = AgentHealth.edges(start, end, interval, zone) ?: return reply(JSONObject().put("error", "RangeError").put("message", "more than 400 buckets"))
        val answer = { since: Double?, buckets: JSONArray ->
            JSONObject().put("type", type).put("unit", UNITS.getValue(type)).put("since", since ?: JSONObject.NULL).put("complete", true).put("buckets", buckets)
        }
        admitted(type, reply) { since, _ ->
            if (context.agent) {
                context.log("health: statistics $type ${edges.size - 1} buckets")
                return@admitted reply(answer(since, agentStore.buckets(type, edges, since)))
            }
            // Buckets are anchored at `start`, as the edges are; one that ends before `since` reads nothing below.
            val from = Instant.ofEpochMilli(start.toLong())
            val to = Instant.ofEpochMilli(end.toLong())
            call({ sums(metric, from, to, interval, zone, origins(args)) }) { r ->
                val values = r.getOrElse { return@call reply(failure(it)) }
                val buckets = JSONArray()
                for ((s, e) in edges.zipWithNext()) {
                    // A bucket before `since` reads nothing: the platform would answer it from data the app may not see.
                    val v = if (since != null && e <= since) null else values[s.toLong()]
                    buckets.put(JSONObject().put("start", s).put("end", e).put("value", v ?: JSONObject.NULL))
                }
                reply(answer(since, buckets))
            }
        }
    }

    private fun workouts(args: JSONObject, reply: (JSONObject) -> Unit) {
        val start = args.optDouble("start")
        val end = args.optDouble("end")
        if (start.isNaN() || end.isNaN()) return reply(JSONObject().put("error", "TypeError").put("message", "start and end"))
        val limit = args.optDouble("limit", 50.0).toInt().coerceIn(1, 500)
        val after = args.optString("after").takeIf { it.isNotEmpty() }
        admitted("workouts", reply) { since, g ->
            if (context.agent) {
                val all = agentStore.workouts(start, end, since)
                context.log("health: workouts ${all.size}")
                return@admitted reply(page(all, after, limit, since, true))
            }
            val from = Instant.ofEpochMilli(maxOf(start, since ?: start).toLong())
            call({ sessions(from, Instant.ofEpochMilli(end.toLong()), origins(args), g) }) { r ->
                val all = r.getOrElse { return@call reply(failure(it)) }
                reply(page(all.filter { it.optDouble("start") >= start }, after, limit, since, true))
            }
        }
    }

    /** [metric]'s sum per bucket from [from] to [to], by each bucket's start. */
    private suspend fun sums(metric: AggregateMetric<*>, from: Instant, to: Instant, interval: String, zone: ZoneId, origins: Set<DataOrigin>): Map<Long, Double?> {
        if (interval == "PT1H") {
            return client.aggregateGroupByDuration(AggregateGroupByDurationRequest(setOf(metric), TimeRangeFilter.between(from, to), Duration.ofHours(1), origins))
                .associate { it.startTime.toEpochMilli() to value(it.result, metric) }
        }
        val slicer = when (interval) { "P1D" -> Period.ofDays(1); "P1W" -> Period.ofWeeks(1); else -> Period.ofMonths(1) }
        // Periods slice local time.
        val local = TimeRangeFilter.between(from.atZone(zone).toLocalDateTime(), to.atZone(zone).toLocalDateTime())
        return client.aggregateGroupByPeriod(AggregateGroupByPeriodRequest(setOf(metric), local, slicer, origins))
            .associate { it.startTime.atZone(zone).toInstant().toEpochMilli() to value(it.result, metric) }
    }

    /** Every exercise session from [from] to [to], page by page, in the interface's shape and order. */
    private suspend fun sessions(from: Instant, to: Instant, origins: Set<DataOrigin>, grants: Set<String>): List<JSONObject> {
        val out = mutableListOf<ExerciseSessionRecord>()
        var token: String? = null
        do {
            val response = client.readRecords(ReadRecordsRequest(ExerciseSessionRecord::class, TimeRangeFilter.between(from, to), origins,
                ascendingOrder = true, pageSize = 1000, pageToken = token))
            out += response.records
            token = response.pageToken
        } while (token != null)
        return out.map { workout(it, grants) }.sortedWith(AgentHealth.ordered)
    }

    /** A session in the interface's shape. Its distance and energy are read only where their own types are granted. */
    private suspend fun workout(w: ExerciseSessionRecord, grants: Set<String>): JSONObject {
        val range = TimeRangeFilter.between(w.startTime, w.endTime)
        val sums = listOfNotNull(
            DistanceRecord.DISTANCE_TOTAL.takeIf { PERMISSIONS.getValue("distance") in grants },
            ActiveCaloriesBurnedRecord.ACTIVE_CALORIES_TOTAL.takeIf { PERMISSIONS.getValue("activeEnergy") in grants },
        )
        val result = if (sums.isEmpty()) null else client.aggregate(AggregateRequest(sums.toSet(), range, setOf(w.metadata.dataOrigin)))
        val distance = result?.let { value(it, DistanceRecord.DISTANCE_TOTAL) }
        val energy = result?.let { value(it, ActiveCaloriesBurnedRecord.ACTIVE_CALORIES_TOTAL) }
        return JSONObject().put("id", w.metadata.id).put("activity", ACTIVITIES[w.exerciseType] ?: "other")
            .put("start", w.startTime.toEpochMilli().toDouble()).put("end", w.endTime.toEpochMilli().toDouble())
            .put("duration", Duration.between(w.startTime, w.endTime).toMillis() / 1000.0)
            .put("distance", distance ?: JSONObject.NULL).put("energy", energy ?: JSONObject.NULL)
            .put("source", w.metadata.dataOrigin.packageName)
            .put("apple", JSONObject.NULL).put("android", JSONObject().put("exerciseType", w.exerciseType))
    }
}
