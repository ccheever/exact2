// @exact/geolocation on Android (LLP 1133.002 D3, D8, D9, D10, D15): the platform's LocationManager, not Play
// services, answered in the interface's shapes. The build makes one per host when the app binds the extension, outside
// the agent; under the agent the runner's substitute answers and none is made.
//
// - The permission is the manifest's, derived from the app's grant line: fine and coarse for `device.geolocation`,
//   coarse alone for `device.geolocation-approximate`, which is then a ceiling (D7). Both are asked in one request.
// - The state is a fact (D8), with a history in the module's own preferences, since the platform says neither
//   "never asked" nor "permanently denied": a rejection with no rationale owed afterwards is a permanent denial, and
//   a read that finds a grant gone that the last read found is a revocation, which clears the history.
// - Android has no event for a change in Settings: the host's resume reads the state again, and that also settles a
//   request whose result a recreated activity lost (D9).
// - One registration serves every reader of `watchPosition`; it is removed at `release`, when the last reader goes.
// - Tier 3 (D4): `GeolocationModule.hatch` is given each API 31 request's builder after the module's words are set.
package exact.geolocation

import android.Manifest
import android.annotation.TargetApi
import android.content.Context
import android.content.pm.PackageManager
import android.location.Location
import android.location.LocationListener
import android.location.LocationManager
import android.location.LocationRequest
import android.os.Build
import android.os.Bundle
import android.os.CancellationSignal
import android.os.Handler
import android.os.Looper
import dev.exact.canvas.ExactExtension
import dev.exact.canvas.ExtensionContext
import org.json.JSONObject

class GeolocationModule(context: ExtensionContext) : ExactExtension(context) {
    companion object {
        /** The app's access hatch (D4): each API 31 request's builder, after the module's words are set. */
        @TargetApi(31) @JvmStatic var hatch: ((LocationRequest.Builder) -> Unit)? = null
        private const val FINE = Manifest.permission.ACCESS_FINE_LOCATION
        private const val COARSE = Manifest.permission.ACCESS_COARSE_LOCATION
        /** The module's unsaid intervals by quality (D3), which the API 31 builder requires. */
        private val INTERVALS = mapOf("highAccuracy" to 5_000L, "balancedPowerAccuracy" to 30_000L, "lowPower" to 60_000L)
        private val QUALITIES = mapOf(
            "highAccuracy" to LocationRequest.QUALITY_HIGH_ACCURACY,
            "balancedPowerAccuracy" to LocationRequest.QUALITY_BALANCED_POWER_ACCURACY,
            "lowPower" to LocationRequest.QUALITY_LOW_POWER,
        )
    }

    private val main = Handler(Looper.getMainLooper())
    private val app: Context get() = context.application
    private val manager get() = app.getSystemService(Context.LOCATION_SERVICE) as LocationManager
    private val history get() = app.getSharedPreferences("exact.geolocation", Context.MODE_PRIVATE)
    /** The permissions the manifest asks for: fine and coarse, or coarse alone under the approximate line. */
    private val asked: Array<String> by lazy {
        val declared = app.packageManager.getPackageInfo(app.packageName, PackageManager.GET_PERMISSIONS).requestedPermissions.orEmpty()
        arrayOf(FINE, COARSE).filter { it in declared }.toTypedArray()
    }
    private var watch: LocationListener? = null
    private var last: JSONObject? = null
    private val watchers = mutableListOf<(JSONObject) -> Unit>()
    private var said: String? = null

    override fun later(name: String, args: JSONObject, reply: (JSONObject) -> Unit) {
        when (name) {
            "geolocationStatus" -> reply(status())
            "getCurrentPosition" -> allowed(reply) { oneShot(args, reply) }
            "watchPosition" -> allowed(reply) { watch(args, reply) }
            "requestPreciseLocation" -> precise(reply)
            else -> reply(error("NotSupportedError"))
        }
    }

    override fun release(name: String) {
        if (name != "watchPosition") return
        watch?.let { manager.removeUpdates(it) }
        watch = null
        last = null
    }

    override fun resumed() {
        // Compared with what was last answered: `status()` keeps what it reads.
        val before = said ?: return
        if (status().toString() != before) {
            context.changed("geolocation.geolocationStatus")
            if (watch != null) {
                last = null
                context.changed("geolocation.watchPosition")
            }
        }
    }

    // The fact

    private fun has(permission: String) = app.checkSelfPermission(permission) == PackageManager.PERMISSION_GRANTED

    private fun status(): JSONObject {
        val fine = has(FINE)
        val granted = fine || has(COARSE)
        val prefs = history
        // A grant the last read found and this one does not: revoked in Settings, or "Only this time" lapsed.
        if (granted != prefs.getBoolean("granted", false)) prefs.edit().putBoolean("granted", granted).putBoolean("permanent", false).apply()
        val activity = context.activity
        val rationale = !granted && activity != null && asked.any { activity.shouldShowRequestPermissionRationale(it) }
        val permanent = !granted && prefs.getBoolean("permanent", false)
        val available = asked.isNotEmpty() && (Build.VERSION.SDK_INT < 28 || manager.isLocationEnabled)
        val out = JSONObject()
            .put("available", available)
            .put("state", if (granted) "granted" else if (permanent) "denied" else "prompt")
            .put("accuracy", if (!granted) "unknown" else if (fine) "precise" else "approximate")
            .put("apple", JSONObject.NULL)
            .put("android", JSONObject().put("shouldShowRationale", rationale).put("permanentlyDenied", permanent))
        said = out.toString()
        return out
    }

    /** [then], once the platform allows it: asked in one request while `prompt` (D9); refused while `denied`. */
    private fun allowed(reply: (JSONObject) -> Unit, then: () -> Unit) {
        val state = status().getString("state")
        when {
            asked.isEmpty() -> reply(error("NotSupportedError"))
            state == "granted" -> then()
            state == "denied" -> reply(error("PERMISSION_DENIED"))
            else -> ask { if (status().getString("state") == "granted") then() else reply(error("PERMISSION_DENIED")) }
        }
    }

    /** The platform's dialog for [asked], then [done]. A rejection with no rationale owed afterwards is permanent. */
    private fun ask(done: () -> Unit) {
        context.requestPermissions(asked) {
            val activity = context.activity
            val rejected = !has(FINE) && !has(COARSE)
            if (rejected && activity != null && asked.none { activity.shouldShowRequestPermissionRationale(it) }) {
                history.edit().putBoolean("permanent", true).apply()
            }
            context.changed("geolocation.geolocationStatus")
            done()
        }
    }

    // D10: the fine and coarse pair again, under the precise line only; the answer is the status.
    private fun precise(reply: (JSONObject) -> Unit) {
        if (FINE !in asked || has(FINE) || !has(COARSE)) return reply(status())
        context.requestPermissions(asked) {
            context.changed("geolocation.geolocationStatus")
            reply(status())
        }
    }

    // Positions

    private fun quality(args: JSONObject): String {
        val words = args.optJSONObject("android")
        return words?.optString("quality")?.takeIf { it.isNotEmpty() }
            ?: if (args.has("enableHighAccuracy") && args.getBoolean("enableHighAccuracy")) "highAccuracy" else "balancedPowerAccuracy"
    }

    /** The written provider; else fused where the device has it; else gps for high accuracy and network otherwise (D3). */
    private fun provider(args: JSONObject): String? {
        val written = args.optJSONObject("android")?.optString("provider")?.takeIf { it.isNotEmpty() }
        if (written == "passive" && !has(FINE)) return null
        val chosen = written ?: when {
            Build.VERSION.SDK_INT >= 31 && manager.hasProvider(LocationManager.FUSED_PROVIDER) -> LocationManager.FUSED_PROVIDER
            quality(args) == "highAccuracy" -> LocationManager.GPS_PROVIDER
            else -> LocationManager.NETWORK_PROVIDER
        }
        return chosen.takeIf { it in manager.allProviders }
    }

    @TargetApi(31)
    private fun request(args: JSONObject, interval: Long): LocationRequest {
        val words = args.optJSONObject("android") ?: JSONObject()
        val builder = LocationRequest.Builder(words.optLong("intervalMillis", interval))
            .setQuality(QUALITIES.getValue(quality(args)))
        if (words.has("minUpdateIntervalMillis")) builder.setMinUpdateIntervalMillis(words.getLong("minUpdateIntervalMillis"))
        if (words.has("maxUpdateDelayMillis")) builder.setMaxUpdateDelayMillis(words.getLong("maxUpdateDelayMillis"))
        if (words.has("minUpdateDistanceMeters")) builder.setMinUpdateDistanceMeters(words.getDouble("minUpdateDistanceMeters").toFloat())
        if (args.has("timeout")) builder.setDurationMillis(args.getLong("timeout").coerceAtLeast(1))
        hatch?.invoke(builder)
        return builder.build()
    }

    /** A fix younger than a positive `maximumAge` answers at once (D2). */
    private fun eligible(provider: String, args: JSONObject): Location? {
        val age = args.optDouble("maximumAge", 0.0)
        if (age <= 0) return null
        val fix = manager.getLastKnownLocation(provider) ?: return null
        return fix.takeIf { System.currentTimeMillis() - it.time <= age }
    }

    private fun oneShot(args: JSONObject, reply: (JSONObject) -> Unit) {
        val provider = provider(args) ?: return reply(error("POSITION_UNAVAILABLE"))
        eligible(provider, args)?.let { return reply(position(it)) }
        var settled = false
        val settle = { answer: JSONObject -> if (!settled) { settled = true; reply(answer) } }
        val cancel = CancellationSignal()
        if (args.has("timeout")) main.postDelayed({ cancel.cancel(); settle(error("TIMEOUT")) }, args.getLong("timeout"))
        val got = { fix: Location? -> main.post { settle(fix?.let(::position) ?: error("POSITION_UNAVAILABLE")) }; Unit }
        when {
            Build.VERSION.SDK_INT >= 31 ->
                manager.getCurrentLocation(provider, request(args, INTERVALS.getValue(quality(args))), cancel, app.mainExecutor) { got(it) }
            Build.VERSION.SDK_INT >= 30 -> manager.getCurrentLocation(provider, cancel, app.mainExecutor) { got(it) }
            else -> {
                // API 29 has no one-shot: the first update of a registration removed after it.
                val listener = object : Listener() {
                    override fun onLocationChanged(fix: Location) { manager.removeUpdates(this); got(fix) }
                }
                cancel.setOnCancelListener { manager.removeUpdates(listener) }
                manager.requestLocationUpdates(provider, 0L, 0f, listener, Looper.getMainLooper())
            }
        }
    }

    private fun watch(args: JSONObject, reply: (JSONObject) -> Unit) {
        last?.let { return reply(it) }
        watchers += reply
        if (watch != null) return
        val provider = provider(args) ?: return deliver(error("POSITION_UNAVAILABLE"))
        val listener = object : Listener() {
            override fun onLocationChanged(fix: Location) = deliver(position(fix))
            override fun onProviderDisabled(provider: String) = deliver(error("POSITION_UNAVAILABLE"))
        }
        watch = listener
        val interval = INTERVALS.getValue(quality(args))
        val distance = args.optJSONObject("android")?.optDouble("minUpdateDistanceMeters", 0.0)?.toFloat() ?: 0f
        when {
            Build.VERSION.SDK_INT >= 31 -> manager.requestLocationUpdates(provider, request(args, interval), app.mainExecutor, listener)
            Build.VERSION.SDK_INT >= 30 -> manager.requestLocationUpdates(provider, interval, distance, app.mainExecutor, listener)
            else -> manager.requestLocationUpdates(provider, interval, distance, listener, Looper.getMainLooper())
        }
    }

    private fun deliver(answer: JSONObject) {
        if (!answer.has("error")) last = answer
        val waiting = watchers.toList()
        watchers.clear()
        waiting.forEach { it(answer) }
        context.changed("geolocation.watchPosition")
    }

    /** A fix in the interface's `GeolocationPosition` (D1): the altitude above the WGS 84 ellipsoid, as the web's. */
    private fun position(fix: Location): JSONObject {
        val android = JSONObject().put("provider", fix.provider ?: "")
            .put("isMock", if (Build.VERSION.SDK_INT >= 31) fix.isMock else JSONObject.NULL)
            .put("mslAltitudeMeters", if (Build.VERSION.SDK_INT >= 34 && fix.hasMslAltitude()) fix.mslAltitudeMeters else JSONObject.NULL)
        val coords = JSONObject()
            .put("latitude", fix.latitude).put("longitude", fix.longitude).put("accuracy", fix.accuracy.toDouble())
            .put("altitude", if (fix.hasAltitude()) fix.altitude else JSONObject.NULL)
            .put("altitudeAccuracy", if (fix.hasVerticalAccuracy()) fix.verticalAccuracyMeters.toDouble() else JSONObject.NULL)
            .put("heading", if (fix.hasBearing()) fix.bearing.toDouble() else JSONObject.NULL)
            .put("speed", if (fix.hasSpeed()) fix.speed.toDouble() else JSONObject.NULL)
        return JSONObject().put("coords", coords).put("timestamp", fix.time.toDouble()).put("apple", JSONObject.NULL).put("android", android)
    }

    private fun error(code: String) = JSONObject().put("error", code)

    /** Every method of the interface: on API 29 only `onLocationChanged` has no body, and the platform calls the rest. */
    private abstract class Listener : LocationListener {
        override fun onProviderEnabled(provider: String) {}
        override fun onProviderDisabled(provider: String) {}
        @Deprecated("the platform still calls it on API 29")
        override fun onStatusChanged(provider: String?, status: Int, extras: Bundle?) {}
    }
}
