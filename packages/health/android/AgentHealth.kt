// The agent's Health store on Android (LLP 1133.026 §11), as Apple's `AgentHealth.swift` is: what @exact/health reads
// under the agent, where it makes no `HealthConnectClient`. It is the package's `fixture/health.json`, or another file
// there the agent's `store` names, which an APK carries as the app's assets under `health-fixture/` (the reader puts an
// app's assets at `app/assets/` in the APK): hourly samples per type and a list of workouts, from one fixture source.
//
// It answers as Health Connect answers (D5): a granted type's answers are `complete` from `since`, 30 days before now
// unless history is granted. Buckets are calendar intervals in the session's zone (`EXACT_AGENT_TIME_ZONE`, UTC unless
// the drive names one), anchored at `start`.
package exact.health

import android.content.res.AssetManager
import org.json.JSONArray
import org.json.JSONObject
import java.time.Instant
import java.time.LocalDate
import java.time.LocalDateTime
import java.time.ZoneId
import java.time.ZoneOffset
import java.time.format.DateTimeParseException

internal class AgentHealth(private val assets: AssetManager) {
    companion object {
        private const val HOUR = 3_600_000.0

        /** D8's order: ascending by `start`, then `id`. */
        val ordered = compareBy<JSONObject>({ it.optDouble("start", 0.0) }, { it.optString("id") })

        /** The bucket edges from [start] to [end] by [interval] in [zone], at most 400; null for more. */
        fun edges(start: Double, end: Double, interval: String, zone: ZoneId): List<Double>? {
            val from = Instant.ofEpochMilli(start.toLong()).atZone(zone)
            val step: (Int) -> Double? = { n ->
                when (interval) {
                    "PT1H" -> start + n * HOUR
                    "P1D" -> from.plusDays(n.toLong()).toInstant().toEpochMilli().toDouble()
                    "P1W" -> from.plusWeeks(n.toLong()).toInstant().toEpochMilli().toDouble()
                    "P1M" -> from.plusMonths(n.toLong()).toInstant().toEpochMilli().toDouble()
                    else -> null
                }
            }
            val out = mutableListOf(start)
            while (out.last() < end) {
                if (out.size > 400) return null
                out.add(minOf(end, step(out.size) ?: return null))
            }
            return out
        }

        /** `2026-10-25T09:00` in [zone], as ms since the epoch. */
        fun local(text: String, zone: ZoneId): Double? = try {
            (if (text.length == 10) LocalDate.parse(text).atStartOfDay() else LocalDateTime.parse(text)).atZone(zone).toInstant().toEpochMilli().toDouble()
        } catch (e: DateTimeParseException) { null }
    }

    /** Each type's samples, `(start ms, value)`, ascending. */
    private var samples: Map<String, List<Pair<Double, Double>>> = emptyMap()
    var units: Map<String, String> = emptyMap()
        private set
    var workouts: List<JSONObject> = emptyList()
        private set
    /** The fixture's today (`now`), past which no sample is seen. */
    var now: Double? = null
    val zone: ZoneId = try { ZoneId.of(System.getenv("EXACT_AGENT_TIME_ZONE") ?: "UTC") } catch (e: Exception) { ZoneOffset.UTC }
    /** Said once when the fixture is not in the APK. */
    var missing: String? = null
        private set

    init { if (!load("health.json")) missing = "no agent Health store: the APK carries no assets/health-fixture/health.json" }

    /** Reads fixture file [name]; false where there is none. */
    fun load(name: String): Boolean {
        if (name.contains('/')) return false
        val json = try {
            JSONObject(assets.open("app/assets/health-fixture/$name").bufferedReader().use { it.readText() })
        } catch (e: java.io.IOException) { return false } catch (e: org.json.JSONException) { return false }
        val u = json.optJSONObject("units") ?: JSONObject()
        units = u.keys().asSequence().associateWith { u.getString(it) }
        val list = json.optJSONArray("workouts") ?: JSONArray()
        workouts = (0 until list.length()).map(list::getJSONObject).sortedWith(ordered)
        val hours = json.optJSONObject("hours") ?: JSONObject()
        samples = hours.keys().asSequence().associateWith { type ->
            val days = hours.getJSONObject(type)
            days.keys().asSequence().flatMap { day ->
                val midnight = try { LocalDate.parse(day).atStartOfDay(ZoneOffset.UTC).toInstant().toEpochMilli().toDouble() } catch (e: DateTimeParseException) { null }
                val values = days.getJSONArray(day)
                if (midnight == null) emptySequence() else (0 until values.length()).asSequence().map { midnight + it * HOUR to values.getDouble(it) }
            }.sortedBy { it.first }.toList()
        }
        return true
    }

    /** [type]'s buckets: null where no sample is seen, a sum from [since]. */
    fun buckets(type: String, edges: List<Double>, since: Double?): JSONArray {
        val list = samples[type].orEmpty()
        val out = JSONArray()
        for ((start, end) in edges.zipWithNext()) {
            val from = maxOf(start, since ?: start)
            val to = minOf(end, now ?: end)
            val seen = list.filter { it.first >= from && it.first < to }
            out.put(JSONObject().put("start", start).put("end", end).put("value", if (seen.isEmpty()) JSONObject.NULL else seen.sumOf { it.second }))
        }
        return out
    }

    /** The workouts starting in `[start, end)` and ending at or after [since]. */
    fun workouts(start: Double, end: Double, since: Double?): List<JSONObject> = workouts.filter {
        val s = it.optDouble("start", 0.0)
        val e = it.optDouble("end", s)
        s >= start && s < end && e >= (since ?: Double.NEGATIVE_INFINITY) && s < (now ?: Double.POSITIVE_INFINITY)
    }
}
