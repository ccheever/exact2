// @exact/altimeter on Android (LLP 1133.026 D13, D15, D17): the default `TYPE_PRESSURE` sensor, answered in the
// interface's shapes. Under the agent the runner's declared substitute answers and nothing here runs.
//
// - One registration serves every reader group (a name with one `history`), at the shortest sampling period one asks:
//   `android.samplingPeriodUs`, else one from `frequency`, else `SENSOR_DELAY_NORMAL`, as the environment sensors'
//   guide registers it. Each group keeps a ring of its numbered samples, answered from the cursor `after`.
// - Pressure is hPa as Android gives it. A group's height is `SensorManager.getAltitude` of the latest pressure less
//   that of its first sample, its zero, taken once for the group's life (D13): weather moves it about 8 m per hPa.
// - Android needs no permission for the sensor: the status reads `granted` wherever there is one.
// - The registration is removed in the background and made again on return; the zero is kept, and each group's next
//   answer reads `interrupted`. A poll while hidden answers what the group holds, with an empty batch.
// - Tier 3 (D17): `AltimeterModule.hatch` is given the manager once, when the module is made.
package exact.altimeter

import android.content.Context
import android.hardware.Sensor
import android.hardware.SensorEvent
import android.hardware.SensorEventListener
import android.hardware.SensorManager
import android.os.Handler
import android.os.Looper
import android.os.SystemClock
import dev.exact.canvas.ExactExtension
import dev.exact.canvas.ExtensionContext
import org.json.JSONArray
import org.json.JSONObject

class AltimeterModule(context: ExtensionContext) : ExactExtension(context) {
    companion object {
        private const val MOST = 256
        /** `SENSOR_DELAY_NORMAL`'s 200,000 µs (`SensorManager.getDelay`). */
        private const val NORMAL_US = 200_000

        /** The app's access hatch (D17): given the manager once, when the module is made. */
        @JvmStatic var hatch: ((SensorManager) -> Unit)? = null
    }

    private class Group(val name: String, val history: Int, val periodUs: Int) {
        var zero: Double? = null
        var since = 0.0
        val ring = ArrayDeque<Pair<Int, JSONObject>>()
        var sequence = 0
        val pauses = ArrayList<Int>()
        val waiting = ArrayList<() -> Unit>()
    }

    private val manager = (context.application.getSystemService(Context.SENSOR_SERVICE) as SensorManager).also { hatch?.invoke(it) }
    private val sensor: Sensor? = manager.getDefaultSensor(Sensor.TYPE_PRESSURE)
    private val main = Handler(Looper.getMainLooper())
    private val groups = LinkedHashMap<String, Group>()
    private var registeredUs: Int? = null
    private var shown = true

    private val listener = object : SensorEventListener {
        override fun onSensorChanged(event: SensorEvent) = take(event.values[0].toDouble(), event.timestamp)
        override fun onAccuracyChanged(sensor: Sensor?, accuracy: Int) {}
    }

    override fun later(name: String, args: JSONObject, reply: (JSONObject) -> Unit) {
        if (context.agent) return reply(JSONObject().put("error", "NotSupportedError"))
        when (name) {
            "altimeterStatus", "requestAltimeterPermission" -> reply(status())
            "barometer", "altimeter" -> read(name, args, reply)
            else -> reply(JSONObject().put("error", "NotSupportedError"))
        }
    }

    private fun status(): JSONObject =
        JSONObject().put("available", sensor != null).put("state", if (sensor != null) "granted" else "unknown").put("apple", JSONObject.NULL)

    private fun periodOf(args: JSONObject): Int {
        val android = args.optJSONObject("android")
        if (android != null && android.has("samplingPeriodUs")) return android.getDouble("samplingPeriodUs").toInt().coerceAtLeast(0)
        val frequency = args.optDouble("frequency", 0.0)
        return if (frequency > 0) (1_000_000 / frequency).toInt() else NORMAL_US
    }

    private fun read(name: String, args: JSONObject, reply: (JSONObject) -> Unit) {
        if (sensor == null) return reply(JSONObject().put("error", "NotSupportedError"))
        val history = args.optDouble("history", 0.0).toInt().coerceIn(0, MOST)
        val key = "$name $history ${periodOf(args)}"
        val group = groups.getOrPut(key) { Group(name, history, periodOf(args)) }
        restart()
        if (group.ring.isNotEmpty()) return reply(answer(group, args))
        if (!shown) return reply(JSONObject().put("error", "NotReadableError"))
        // The first answer waits for the first sample, a second at most.
        var answered = false
        val once = {
            if (!answered) {
                answered = true
                reply(if (group.ring.isEmpty()) JSONObject().put("error", "NotReadableError") else answer(group, args))
            }
        }
        group.waiting.add(once)
        main.postDelayed(once, 1000)
    }

    private fun answer(group: Group, args: JSONObject): JSONObject {
        val (newest, fields) = group.ring.last()
        val out = JSONObject(fields.toString()).put("sequence", newest).put("samples", JSONArray()).put("dropped", 0).put("interrupted", false)
        if (args.has("after")) {
            val after = args.getDouble("after").toInt()
            val fresh = group.ring.filter { it.first > after }
            val kept = if (group.history > 0 && shown) fresh.takeLast(group.history) else emptyList()
            out.put("samples", JSONArray(kept.map { it.second }))
            out.put("dropped", if (group.history > 0) maxOf(0, newest - after - kept.size) else 0)
            out.put("interrupted", !shown || group.pauses.any { it >= after })
        }
        return out
    }

    override fun release(name: String) {
        groups.entries.removeAll { it.value.name == name }
        restart()
    }

    override fun release(name: String, args: JSONObject) {
        groups.remove("$name ${args.optDouble("history", 0.0).toInt().coerceIn(0, MOST)} ${periodOf(args)}")
        restart()
    }

    override fun foreground(shown: Boolean) {
        if (shown == this.shown) return
        this.shown = shown
        if (!shown) groups.values.forEach { it.pauses.add(it.sequence) }
        restart()
    }

    override fun destroy(done: () -> Unit) {
        groups.clear()
        restart()
        done()
    }

    private fun restart() {
        val wanted = if (shown) groups.values.minOfOrNull { it.periodUs } else null
        if (wanted == registeredUs) return
        if (registeredUs != null) manager.unregisterListener(listener)
        registeredUs = wanted
        if (wanted != null && sensor != null) {
            manager.registerListener(listener, sensor, wanted, main)
            context.log("altimeter: pressure every $wanted µs")
        }
    }

    private fun take(pressure: Double, timestamp: Long) {
        val at = context.now() - (SystemClock.elapsedRealtimeNanos() - timestamp) / 1e6
        val height = SensorManager.getAltitude(SensorManager.PRESSURE_STANDARD_ATMOSPHERE, pressure.toFloat()).toDouble()
        for (group in groups.values) {
            group.sequence += 1
            val fields = if (group.name == "barometer") {
                JSONObject().put("pressure", pressure).put("timestamp", at)
            } else {
                val zero = group.zero ?: height.also { group.zero = it; group.since = at }
                JSONObject().put("relativeAltitude", height - zero).put("since", group.since).put("timestamp", at)
            }
            group.ring.addLast(group.sequence to fields)
            while (group.ring.size > maxOf(1, group.history)) group.ring.removeFirst()
            val waiting = ArrayList(group.waiting)
            group.waiting.clear()
            waiting.forEach { it() }
        }
    }
}
