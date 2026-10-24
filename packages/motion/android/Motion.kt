// @exact/motion on Android (LLP 1133.013 D2, D4, D5, D8): the platform's SensorManager, answered in the interface's
// shapes. The build makes one per host when the app binds the extension, outside the agent; under the agent the
// runner's substitute answers and none is made.
//
// - One registration per sensor serves every reader of the names it feeds (D4), at the fastest frequency they ask
//   (50 Hz, `SENSOR_DELAY_GAME`'s, when one names none; `deviceMotion` always at it). Each reader group (a name with one `frequency`, `referenceFrame` and `history`) keeps
//   a ring of its own thinned samples, numbered, answered from the cursor the runner passes as `after` (D5).
// - Android's units, sign and Earth frame are the W3C's (m/s², rad/s, µT; x east, y north, z up), so readings pass
//   through. A rotation vector is answered as the quaternion `SensorManager.getQuaternionFromVector` gives, reordered
//   to x, y, z, w; `deviceOrientation`'s angles are Chromium's, from the rotation matrix.
// - `referenceFrame: "screen"` turns x and y by the display's angle (the context's `orientation`, the web's
//   `screen.orientation.angle`).
// - Above 200 Hz Android 12 needs `HIGH_SAMPLING_RATE_SENSORS`, which no grant line derives yet: without it a faster
//   frequency is asked at 200 Hz. The status's `android` is null: neither the permission nor the microphone
//   toggle's limit is reported yet (D9).
// - Registrations are removed while the app is in the background and made again in the foreground; each reader's next
//   answer reads `interrupted` (D8).
// - A canvas input (D6) is a consumer of its sensor like a reader, thinned to its own frequency (60 Hz by default), each
//   sample written into the surface's input (the context's `surfaces`), which the host drains into the canvas once a
//   frame. It holds the lease while displayed: hidden, it pauses; shown by an `app` commit, it resumes; shown by a
//   tool's or a wake's, it stays paused. The background pauses it too, and the return resumes one whose canvas stayed
//   displayed. The first sample after a pause counts the samples the pause lost in its `dropped`.
// - Tier 3 (D10): `MotionModule.hatch` is given the manager once, when the module is made.
package exact.motion

import android.content.Context
import android.content.pm.PackageManager
import android.hardware.Sensor
import android.hardware.SensorEvent
import android.hardware.SensorEventListener
import android.hardware.SensorManager
import android.os.Build
import android.os.Handler
import android.os.Looper
import android.os.SystemClock
import dev.exact.canvas.ExactExtension
import dev.exact.canvas.ExtensionContext
import org.json.JSONArray
import org.json.JSONObject
import kotlin.math.PI
import kotlin.math.asin
import kotlin.math.atan2
import kotlin.math.cos
import kotlin.math.roundToInt
import kotlin.math.sin

class MotionModule(context: ExtensionContext) : ExactExtension(context) {
    companion object {
        private const val RING = 1024
        private const val MOST_HZ = 200.0
        /** `SENSOR_DELAY_GAME`'s 20,000 µs (`SensorManager.getDelay`). */
        private const val GAME_HZ = 50.0

        /** The app's access hatch (D10): given the manager once, when the module is made. */
        @JvmStatic var hatch: ((SensorManager) -> Unit)? = null
        /** Each name's sensor type. */
        private val TYPES = mapOf(
            "accelerometer" to Sensor.TYPE_ACCELEROMETER,
            "linearAccelerationSensor" to Sensor.TYPE_LINEAR_ACCELERATION,
            "gravitySensor" to Sensor.TYPE_GRAVITY,
            "gyroscope" to Sensor.TYPE_GYROSCOPE,
            "magnetometer" to Sensor.TYPE_MAGNETIC_FIELD,
            "absoluteOrientationSensor" to Sensor.TYPE_ROTATION_VECTOR,
            "relativeOrientationSensor" to Sensor.TYPE_GAME_ROTATION_VECTOR,
            "deviceMotion" to Sensor.TYPE_ACCELEROMETER,
            "deviceOrientation" to Sensor.TYPE_GAME_ROTATION_VECTOR,
        )
        /** Each canvas input's sensor as the source that reads it. */
        private val SOURCES = mapOf(
            "accelerometer" to "accelerometer", "linearAcceleration" to "linearAccelerationSensor", "gravity" to "gravitySensor",
            "gyroscope" to "gyroscope", "magnetometer" to "magnetometer",
            "absoluteOrientation" to "absoluteOrientationSensor", "relativeOrientation" to "relativeOrientationSensor",
        )
        private val SENSORS = mapOf(
            "accelerometer" to Sensor.TYPE_ACCELEROMETER, "linearAcceleration" to Sensor.TYPE_LINEAR_ACCELERATION,
            "gravity" to Sensor.TYPE_GRAVITY, "gyroscope" to Sensor.TYPE_GYROSCOPE,
            "magnetometer" to Sensor.TYPE_MAGNETIC_FIELD, "absoluteOrientation" to Sensor.TYPE_ROTATION_VECTOR,
            "relativeOrientation" to Sensor.TYPE_GAME_ROTATION_VECTOR,
            "deviceMotion" to Sensor.TYPE_ACCELEROMETER, "deviceOrientation" to Sensor.TYPE_GAME_ROTATION_VECTOR,
        )
    }

    private class Sample(val sequence: Int, val fields: JSONObject)

    /** A canvas input (D6): [sensor]'s samples, read as source [name] reads them, written into [input]. */
    private class Feed(val surface: String, val sensor: String, val name: String, val input: dev.exact.canvas.SurfaceInput,
                       val frequency: Double, val screen: Boolean) {
        val period = 1000 / frequency
        var next = Double.NEGATIVE_INFINITY
        /** When it last stopped taking samples, until the next one it takes. */
        var pausedAt: Double? = null
    }
    private class Group(val args: JSONObject) {
        val period: Double = args.optDouble("frequency", 0.0).let { if (it > 0) 1000 / it else 0.0 }
        var next = Double.NEGATIVE_INFINITY
        val ring = ArrayDeque<Sample>()
        var sequence = 0
        val pauses = mutableListOf<Int>()
        val waiting = mutableListOf<() -> Unit>()
    }

    private val main = Handler(Looper.getMainLooper())
    private val manager = (context.application.getSystemService(Context.SENSOR_SERVICE) as SensorManager).also { hatch?.invoke(it) }
    /** Reader groups by name, then by key. */
    private val groups = mutableMapOf<String, MutableMap<String, Group>>()
    /** Each registered sensor type's listener and its period in µs. */
    private val registered = mutableMapOf<Int, Pair<SensorEventListener, Int>>()
    /** The latest event values of each sensor type, which `deviceMotion` composes. */
    private val latest = mutableMapOf<Int, FloatArray>()
    private var shown = true
    /** Canvas inputs by surface and sensor. */
    private val feeds = mutableMapOf<String, Feed>()
    /** The surfaces the runner said are hidden, and those whose inputs wait for an app's commit to show them (the
     * lease), while an input feeds them. */
    private val hidden = mutableSetOf<String>()
    private val leased = mutableSetOf<String>()

    override fun later(name: String, args: JSONObject, reply: (JSONObject) -> Unit) {
        main.post {
            when (name) {
                "motionStatus", "requestMotionPermission" -> reply(status())
                "startSensorInput" -> reply(startInput(args))
                "stopSensorInput" -> reply(stopInput(args))
                in TYPES -> read(name, args, reply)
                else -> reply(JSONObject().put("error", "NotSupportedError"))
            }
        }
    }

    private fun has(type: Int) = manager.getDefaultSensor(type) != null

    /** Whether rates above 200 Hz are this app's: before Android 12 every app's, from it the permission's holders'. */
    private val high get() = Build.VERSION.SDK_INT < 31 ||
        context.application.checkSelfPermission("android.permission.HIGH_SAMPLING_RATE_SENSORS") == PackageManager.PERMISSION_GRANTED

    private fun status(): JSONObject {
        val sensors = JSONObject()
        for ((n, t) in SENSORS) sensors.put(n, has(t))
        // Motion sensors ask no runtime permission on Android (D9).
        return JSONObject().put("available", has(Sensor.TYPE_ACCELEROMETER)).put("state", "granted").put("sensors", sensors)
            .put("apple", JSONObject.NULL).put("android", JSONObject.NULL)
    }

    private fun key(args: JSONObject) =
        "${args.optDouble("frequency", 0.0)} ${args.optString("referenceFrame", "device")} ${args.optInt("history", 0)}"

    private fun read(name: String, args: JSONObject, reply: (JSONObject) -> Unit) {
        val type = TYPES.getValue(name)
        if (!has(type)) return reply(JSONObject().put("error", "NotSupportedError"))
        val asked = JSONObject(args.toString()).apply { remove("after") }
        val group = groups.getOrPut(name) { mutableMapOf() }.getOrPut(key(asked)) { Group(asked) }
        restart()
        if (group.ring.isEmpty()) {
            if (!shown) return reply(JSONObject().put("error", "NotReadableError"))
            var answered = false
            val once = {
                if (!answered) {
                    answered = true
                    reply(if (group.ring.isEmpty()) JSONObject().put("error", "NotReadableError") else answer(group, args))
                }
            }
            group.waiting += once
            main.postDelayed(once, 1000)
            return
        }
        reply(answer(group, args))
    }

    /** A reader's answer from its group's ring after `after` (D5). */
    private fun answer(group: Group, args: JSONObject): JSONObject {
        val newest = group.ring.last()
        val out = JSONObject(newest.fields.toString()).put("sequence", newest.sequence)
            .put("samples", JSONArray()).put("dropped", 0).put("interrupted", false)
        if (args.has("after") && !args.isNull("after")) {
            val after = args.getDouble("after").toInt()
            val history = minOf(RING, group.args.optInt("history", 0))
            val fresh = group.ring.filter { it.sequence > after }
            val kept = if (history > 0) fresh.takeLast(history) else emptyList()
            out.put("samples", JSONArray(kept.map { it.fields }))
            out.put("dropped", if (history > 0) maxOf(0, newest.sequence - after - kept.size) else 0)
            out.put("interrupted", group.pauses.any { it >= after })
        }
        return out
    }

    override fun release(name: String) {
        groups.remove(name)
        restart()
    }

    override fun release(name: String, args: JSONObject) {
        groups[name]?.remove(key(JSONObject(args.toString()).apply { remove("after") }))
        restart()
    }

    override fun foreground(shown: Boolean) {
        if (shown == this.shown) return
        this.shown = shown
        if (!shown) for (g in groups.values.flatMap { it.values }) g.pauses += g.sequence
        pauseFeeds()
        restart()
    }

    private fun startInput(args: JSONObject): JSONObject {
        val surface = args.optString("surface")
        val sensor = args.optString("sensor")
        val name = SOURCES[sensor]
        if (name == null || !has(TYPES.getValue(name))) return JSONObject().put("error", "NotSupportedError")
        val input = context.surfaces.input(surface)
            ?: return JSONObject().put("error", "NotFoundError").put("message", "no canvas places surface $surface")
        val frequency = args.optDouble("frequency", 0.0).takeIf { it > 0 } ?: 60.0
        feeds["$surface $sensor"] = Feed(surface, sensor, name, input, frequency, args.optString("referenceFrame") == "screen")
        // A new start takes the canvas as the runner last said it is.
        if (surface !in hidden) leased.remove(surface)
        restart()
        context.log("motion: input $sensor on $surface at $frequency Hz${if (active(surface)) "" else ", paused"}")
        return JSONObject().put("surface", surface).put("sensor", sensor)
    }

    private fun stopInput(args: JSONObject): JSONObject {
        val surface = args.optString("surface")
        val sensor = args.optString("sensor")
        feeds.remove("$surface $sensor")?.input?.end(sensor, context.now())
        if (feeds.values.none { it.surface == surface }) { hidden.remove(surface); leased.remove(surface) }
        restart()
        return JSONObject().put("surface", surface).put("sensor", sensor)
    }

    override fun displayed(surface: String, shown: Boolean, origin: String) {
        if (shown) hidden.remove(surface) else hidden.add(surface)
        if (!shown) leased.add(surface) else if (origin == "app") leased.remove(surface)
        context.log("motion: surface $surface displayed $shown ($origin origin), its inputs ${if (active(surface)) "run" else "paused"}")
        pauseFeeds()
        restart()
    }

    /** Whether the inputs on [surface] take samples now. */
    private fun active(surface: String) = shown && surface !in hidden && surface !in leased

    /** Each input that stops taking samples now notes when. */
    private fun pauseFeeds() {
        for (f in feeds.values) if (!active(f.surface) && f.pausedAt == null) f.pausedAt = context.now()
    }

    /** The registrations the readers need: each sensor type at the fastest frequency of the names it feeds. */
    private fun restart() {
        val wanted = mutableMapOf<Int, Double>()
        if (shown) for ((name, byKey) in groups) for (g in byKey.values) {
            val types = if (name == "deviceMotion") listOf(Sensor.TYPE_ACCELEROMETER, Sensor.TYPE_LINEAR_ACCELERATION, Sensor.TYPE_GYROSCOPE)
                else listOf(TYPES.getValue(name))
            val asked = g.args.optDouble("frequency", 0.0).takeIf { it > 0 && name != "deviceMotion" }
            val f = (asked ?: GAME_HZ).let { if (high) it else minOf(MOST_HZ, it) }
            for (t in types) if (has(t)) wanted[t] = maxOf(wanted[t] ?: 0.0, f)
        }
        for (feed in feeds.values) if (active(feed.surface)) {
            val type = TYPES.getValue(feed.name)
            val f = if (high) feed.frequency else minOf(MOST_HZ, feed.frequency)
            wanted[type] = maxOf(wanted[type] ?: 0.0, f)
        }
        for ((type, pair) in registered.toMap()) {
            val us = wanted[type]?.let { (1_000_000 / it).toInt() }
            if (us != pair.second) { manager.unregisterListener(pair.first); registered.remove(type) }
        }
        for ((type, f) in wanted) {
            if (type in registered) continue
            val us = (1_000_000 / f).toInt()
            val listener = object : SensorEventListener {
                override fun onSensorChanged(event: SensorEvent) = take(type, event)
                override fun onAccuracyChanged(sensor: Sensor, accuracy: Int) {}
            }
            manager.registerListener(listener, manager.getDefaultSensor(type), us, main)
            registered[type] = listener to us
            context.log("motion: sensor $type at $f Hz")
        }
    }

    /** The session's clock at a sensor event's timestamp (ns of `elapsedRealtimeNanos`). */
    private fun stamp(ns: Long) = context.now() - (SystemClock.elapsedRealtimeNanos() - ns) / 1e6

    /** A sample for each group of the names `type` feeds (D4's thinning). No reader is asked again for it: the runner
     * polls each at its `interval`, and the samples between are the next answer's batch. */
    private fun take(type: Int, event: SensorEvent) {
        latest[type] = event.values.clone()
        val at = stamp(event.timestamp)
        for ((name, byKey) in groups) {
            val feeds = if (name == "deviceMotion") type == Sensor.TYPE_ACCELEROMETER else TYPES[name] == type
            if (!feeds) continue
            for (g in byKey.values) {
                if (g.period > 0 && at < g.next - g.period / 2) continue
                g.next = maxOf(g.next + g.period, at + g.period / 2)
                g.sequence += 1
                val screen = if (g.args.optString("referenceFrame") == "screen") angle() else 0
                g.ring.addLast(Sample(g.sequence, fields(name, event.values, at, screen)))
                val bound = maxOf(1, minOf(RING, g.args.optInt("history", 0)))
                while (g.ring.size > bound) g.ring.removeFirst()
                val waiting = g.waiting.toList()
                g.waiting.clear()
                waiting.forEach { it() }
            }
        }
        for (feed in feeds.values) {
            if (TYPES[feed.name] != type || !active(feed.surface)) continue
            if (at < feed.next - feed.period / 2) continue
            feed.next = maxOf(feed.next + feed.period, at + feed.period / 2)
            val deg = if (feed.screen) angle() else 0
            val values = if (feed.name.endsWith("OrientationSensor")) {
                val q = quaternion(event.values, deg)
                DoubleArray(4) { q.getDouble(it) }
            } else {
                val v = xyz(event.values, at, deg)
                doubleArrayOf(v.getDouble("x"), v.getDouble("y"), v.getDouble("z"))
            }
            val dropped = feed.pausedAt?.let { maxOf(0, ((at - it) / feed.period).toInt()) } ?: 0
            feed.pausedAt = null
            feed.input.sample(feed.sensor, values, dropped, at)
        }
    }

    /** The display's angle, as the web's `screen.orientation.angle`. */
    private fun angle(): Int = context.orientation

    private fun xyz(v: FloatArray, at: Double, deg: Int): JSONObject {
        val r = deg * PI / 180
        val c = cos(r).roundToInt()
        val s = sin(r).roundToInt()
        val (x, y) = (v[0] * c - v[1] * s).toDouble() to (v[0] * s + v[1] * c).toDouble()
        return JSONObject().put("x", x).put("y", y).put("z", v[2].toDouble()).put("timestamp", at)
    }

    /** A rotation vector as the quaternion x, y, z, w, turned into the screen's frame at `deg`. */
    private fun quaternion(v: FloatArray, deg: Int): JSONArray {
        val q = FloatArray(4)
        SensorManager.getQuaternionFromVector(q, v)
        var (x, y, z, w) = listOf(q[1].toDouble(), q[2].toDouble(), q[3].toDouble(), q[0].toDouble())
        if (deg != 0) {
            // q ⊗ (0, 0, −sin(θ/2), cos(θ/2)): a vector's screen components are its device components turned by θ.
            val h = deg * PI / 360
            val bz = -sin(h)
            val bw = cos(h)
            val turned = listOf(x * bw + y * bz, y * bw - x * bz, z * bw + w * bz, w * bw - z * bz)
            x = turned[0]; y = turned[1]; z = turned[2]; w = turned[3]
        }
        return JSONArray(listOf(x, y, z, w))
    }

    private fun fields(name: String, v: FloatArray, at: Double, deg: Int): JSONObject = when (name) {
        "absoluteOrientationSensor", "relativeOrientationSensor" ->
            JSONObject().put("quaternion", quaternion(v, deg)).put("timestamp", at).put("apple", JSONObject.NULL)
        "deviceMotion" -> {
            val v3 = { a: FloatArray? -> a?.let { JSONObject().put("x", it[0].toDouble()).put("y", it[1].toDouble()).put("z", it[2].toDouble()) } ?: JSONObject.NULL }
            val rate = latest[Sensor.TYPE_GYROSCOPE]
            val d = 180 / PI
            JSONObject().put("acceleration", v3(latest[Sensor.TYPE_LINEAR_ACCELERATION])).put("accelerationIncludingGravity", v3(v))
                .put("rotationRate", rate?.let { JSONObject().put("alpha", it[2] * d).put("beta", it[0] * d).put("gamma", it[1] * d) } ?: JSONObject.NULL)
                .put("interval", registered[Sensor.TYPE_ACCELEROMETER]?.second?.let { it / 1000.0 } ?: 0.0).put("timeStamp", at)
        }
        "deviceOrientation" -> orientation(v, at)
        else -> xyz(v, at, deg)
    }

    /** Device Orientation's alpha, beta and gamma from a rotation vector, as Chromium computes them from the matrix. */
    private fun orientation(v: FloatArray, at: Double): JSONObject {
        val m = FloatArray(9)
        SensorManager.getRotationMatrixFromVector(m, v)
        var alpha: Double
        var beta: Double
        var gamma: Double
        if (m[8] > 0) {
            alpha = atan2(-m[1].toDouble(), m[4].toDouble())
            beta = asin(m[7].toDouble())
            gamma = atan2(-m[6].toDouble(), m[8].toDouble())
        } else if (m[8] < 0) {
            alpha = atan2(m[1].toDouble(), -m[4].toDouble())
            beta = -asin(m[7].toDouble())
            beta += if (beta >= 0) -PI else PI
            gamma = atan2(m[6].toDouble(), -m[8].toDouble())
        } else if (m[6] > 0) {
            alpha = atan2(-m[1].toDouble(), m[4].toDouble())
            beta = asin(m[7].toDouble())
            gamma = -PI / 2
        } else if (m[6] < 0) {
            alpha = atan2(m[1].toDouble(), -m[4].toDouble())
            beta = -asin(m[7].toDouble())
            beta += if (beta >= 0) -PI else PI
            gamma = -PI / 2
        } else {
            alpha = atan2(m[3].toDouble(), m[0].toDouble())
            beta = if (m[7] > 0) PI / 2 else -PI / 2
            gamma = 0.0
        }
        if (alpha < 0) alpha += 2 * PI
        val d = 180 / PI
        return JSONObject().put("alpha", alpha * d).put("beta", beta * d).put("gamma", gamma * d).put("absolute", false)
            .put("timeStamp", at).put("apple", JSONObject.NULL)
    }
}
