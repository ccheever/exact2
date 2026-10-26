// @exact/pedometer on Android (LLP 1133.026 D12, D15, D17): Google Play services' Recording API where it is current,
// with the step counter between reads, else the step counter alone. Under the agent the runner's declared substitute
// answers and nothing here runs.
//
// - The Recording API records steps for an app that is not running, "for up to 10 days" (D12). The first granted
//   request subscribes to `TYPE_STEP_COUNT_DELTA`; each answer is its total from the group's boundary to now (one
//   bucket), plus the step counter's increase since that read while the app is in the foreground, so the line moves
//   step by step. `since` is the latest of `start`, the subscription's start and ten days back: no read is issued from
//   before what the API keeps.
// - Without Play services at `LOCAL_RECORDING_CLIENT_MIN_VERSION_CODE`, the step counter alone counts while the app
//   is in the foreground; each return from the background starts a new zero, so `since` moves and the answer never
//   claims steps it could not have seen. `android.source` says which path answered.
// - A counter that falls (a reboot) starts a new zero on either path.
// - Both ask `ACTIVITY_RECOGNITION`, through the host's permission history (LLP 1133.002 D8).
// - Tier 3 (D17): `PedometerModule.hatch` is given the manager once, when the module is made.
package exact.pedometer

import android.Manifest
import android.content.Context
import android.hardware.Sensor
import android.hardware.SensorEvent
import android.hardware.SensorEventListener
import android.hardware.SensorManager
import android.os.Handler
import android.os.Looper
import com.google.android.gms.common.ConnectionResult
import com.google.android.gms.common.GoogleApiAvailability
import com.google.android.gms.fitness.FitnessLocal
import com.google.android.gms.fitness.LocalRecordingClient
import com.google.android.gms.fitness.data.LocalDataType
import com.google.android.gms.fitness.data.LocalField
import com.google.android.gms.fitness.request.LocalDataReadRequest
import dev.exact.canvas.ExactExtension
import dev.exact.canvas.ExtensionContext
import org.json.JSONObject
import java.util.concurrent.TimeUnit

class PedometerModule(context: ExtensionContext) : ExactExtension(context) {
    companion object {
        /** What the Recording API keeps (D12): "up to 10 days". */
        private const val KEPT_MS = 10 * 86_400_000.0
        private const val PERMISSION = Manifest.permission.ACTIVITY_RECOGNITION

        /** The app's access hatch (D17): given the manager once, when the module is made. */
        @JvmStatic var hatch: ((SensorManager) -> Unit)? = null
    }

    private val manager = (context.application.getSystemService(Context.SENSOR_SERVICE) as SensorManager).also { hatch?.invoke(it) }
    private val counter: Sensor? = manager.getDefaultSensor(Sensor.TYPE_STEP_COUNTER)
    private val recording: LocalRecordingClient? = try {
        val ok = GoogleApiAvailability.getInstance()
            .isGooglePlayServicesAvailable(context.application, LocalRecordingClient.LOCAL_RECORDING_CLIENT_MIN_VERSION_CODE) == ConnectionResult.SUCCESS
        if (ok) FitnessLocal.getLocalRecordingClient(context.application) else null
    } catch (e: Throwable) { null }
    private val main = Handler(Looper.getMainLooper())
    private val counts = HashMap<String, Count>()
    /** When the subscription began, as this process saw it. */
    private var subscribed: Double? = null
    private var listening = false
    private var shown = true

    private val listener = object : SensorEventListener {
        override fun onSensorChanged(event: SensorEvent) {
            val value = event.values[0].toDouble()
            counts.values.forEach { it.counter(value, context.now()) }
        }
        override fun onAccuracyChanged(sensor: Sensor?, accuracy: Int) {}
    }

    override fun later(name: String, args: JSONObject, reply: (JSONObject) -> Unit) {
        if (context.agent) return reply(JSONObject().put("error", "NotSupportedError"))
        when (name) {
            "pedometerStatus" -> reply(status())
            "requestPedometerPermission" -> request(reply)
            "pedometer" -> read(args, reply)
            else -> reply(JSONObject().put("error", "NotSupportedError"))
        }
    }

    private val available get() = recording != null || counter != null

    private fun status(): JSONObject {
        val p = context.permission(PERMISSION)
        return JSONObject().put("available", available).put("state", if (available) p.state else "unknown").put("apple", JSONObject.NULL)
            .put("android", JSONObject().put("shouldShowRationale", p.shouldShowRationale).put("permanentlyDenied", p.permanentlyDenied))
    }

    private fun request(reply: (JSONObject) -> Unit) {
        if (!available || context.permission(PERMISSION).state != "prompt") return reply(status())
        context.askPermission(PERMISSION) {
            subscribe()
            reply(status())
        }
    }

    /** The Recording API's subscription, made once granted; renewed when Play services dropped it (D12). */
    private fun subscribe() {
        val client = recording ?: return
        if (subscribed != null || context.permission(PERMISSION).state != "granted") return
        client.subscribe(LocalDataType.TYPE_STEP_COUNT_DELTA)
            .addOnSuccessListener { subscribed = subscribed ?: context.now(); context.log("pedometer: recording subscribed") }
            .addOnFailureListener { e -> context.log("pedometer: recording subscription failed: ${e.message}") }
    }

    private fun read(args: JSONObject, reply: (JSONObject) -> Unit) {
        if (!available) return reply(JSONObject().put("error", "NotSupportedError"))
        when (context.permission(PERMISSION).state) {
            "granted" -> {}
            "prompt" -> return reply(JSONObject().put("error", "NotAllowedError").put("message", "requestPedometerPermission"))
            else -> return reply(JSONObject().put("error", "NotAllowedError"))
        }
        subscribe()
        val key = if (args.has("start")) "${args.getDouble("start")}" else "first"
        val now = context.now()
        val count = counts.getOrPut(key) { Count(if (args.has("start")) args.getDouble("start") else now) }
        listen()
        val client = recording ?: return reply(answer(count.sensor(now), "sensor"))
        if (!count.due(now)) return reply(answer(count.between(now), "recording"))
        // The Recording total from the boundary, which reads no further back than the API keeps.
        val from = maxOf(count.start, subscribed ?: count.start, now - KEPT_MS)
        val request = LocalDataReadRequest.Builder().aggregate(LocalDataType.TYPE_STEP_COUNT_DELTA)
            .bucketByTime(maxOf(1, ((now - from) / 1000).toInt() + 1), TimeUnit.SECONDS)
            .setTimeRange((from / 1000).toLong(), (now / 1000).toLong() + 1, TimeUnit.SECONDS).build()
        client.readData(request).addOnSuccessListener { response ->
            val total = response.buckets.flatMap { it.dataSets }.flatMap { it.dataPoints }
                .sumOf { it.getValue(LocalField.FIELD_STEPS).asInt().toLong() }.toDouble()
            main.post { reply(answer(count.recorded(total, from, context.now()), "recording")) }
        }.addOnFailureListener { e ->
            context.log("pedometer: recording read failed: ${e.message}")
            main.post { reply(answer(count.sensor(context.now()), "sensor")) }
        }
    }

    private fun answer(a: Count.Answer, source: String): JSONObject =
        JSONObject().put("steps", a.steps).put("since", a.since).put("timestamp", a.at).put("apple", JSONObject.NULL)
            .put("android", JSONObject().put("source", source))

    /** The step counter between reads, while the app is in the foreground. */
    private fun listen() {
        val want = shown && counts.isNotEmpty() && counter != null
        if (want == listening) return
        listening = want
        if (want) manager.registerListener(listener, counter, SensorManager.SENSOR_DELAY_NORMAL, main) else manager.unregisterListener(listener)
    }

    override fun release(name: String) {
        if (name != "pedometer") return
        counts.clear()
        listen()
    }

    override fun release(name: String, args: JSONObject) {
        if (name != "pedometer") return
        counts.remove(if (args.has("start")) "${args.getDouble("start")}" else "first")
        listen()
    }

    override fun foreground(shown: Boolean) {
        if (shown == this.shown) return
        this.shown = shown
        if (!shown) counts.values.forEach { it.hidden() }
        listen()
    }

    override fun resumed() = subscribe()

    override fun destroy(done: () -> Unit) {
        counts.clear()
        listen()
        done()
    }
}
