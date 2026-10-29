// @exact/geofencing on Android (LLP 1133.021 D10-D14, §15 question 3): Play services' `GeofencingClient`, over the
// host's region store (`ExactRegions`, the truth: Play services lists no app's geofences and drops them at reboot and
// when location goes off). The build makes one per host when the app binds the extension, outside the agent; under
// the agent the runner's substitute answers, unless the drive binds it (`--bind geofencing`).
//
// - Ask (D13): two decisions in order, fine and coarse first, then `ACCESS_BACKGROUND_LOCATION` alone (a request for
//   both at once is ignored from Android 11, where the second opens the app's location settings page). A denied first
//   step fails `PERMISSION_DENIED` without the second.
// - Twenty a fence app (D11), `android.limit` up to Play services' 100; a radius under 100 m is raised with a journal
//   line. A fence added while inside raises `geofenceenter` (`INITIAL_TRIGGER_ENTER`); a re-arm adds none.
// - A crossing reaches `GeofenceReceiver` (Receivers.kt), which writes it to the store and asks for a region wake. A
//   live session's module delivers it at once; with none (no headless session yet, §9 item 6) the store keeps it,
//   and the module delivers it when a later launch makes it (D14 step 3), with its original time.
package exact.geofencing

import android.Manifest
import android.app.PendingIntent
import android.content.Context
import android.content.Intent
import android.content.pm.PackageManager
import android.os.Build
import com.google.android.gms.common.ConnectionResult
import com.google.android.gms.common.GoogleApiAvailability
import com.google.android.gms.common.api.ApiException
import com.google.android.gms.location.Geofence
import com.google.android.gms.location.GeofenceStatusCodes
import com.google.android.gms.location.GeofencingRequest
import com.google.android.gms.location.LocationServices
import dev.exact.canvas.ExactExtension
import dev.exact.canvas.ExactRegions
import dev.exact.canvas.ExtensionContext
import org.json.JSONArray
import org.json.JSONObject

class GeofencingModule(context: ExtensionContext) : ExactExtension(context) {
    companion object {
        private const val FINE = Manifest.permission.ACCESS_FINE_LOCATION
        private const val COARSE = Manifest.permission.ACCESS_COARSE_LOCATION
        private const val BACKGROUND = Manifest.permission.ACCESS_BACKGROUND_LOCATION
        private const val LIMIT = 20

        /** The broadcast Play services fills in with each crossing: mutable, since it adds the event's extras. */
        internal fun pendingIntent(app: Context): PendingIntent {
            val flags = PendingIntent.FLAG_UPDATE_CURRENT or (if (Build.VERSION.SDK_INT >= 31) PendingIntent.FLAG_MUTABLE else 0)
            return PendingIntent.getBroadcast(app, 0, Intent(app, GeofenceReceiver::class.java), flags)
        }

        /** Play services' fence for [region]: entry and exit, or a dwell for `android.loiteringDelay`. */
        internal fun fence(region: JSONObject): Geofence {
            val options = region.optJSONObject("options") ?: JSONObject()
            val dwell = options.optLong("loiteringDelay", 0)
            return Geofence.Builder()
                .setRequestId(region.getString("name"))
                .setCircularRegion(region.getDouble("latitude"), region.getDouble("longitude"), region.getDouble("radius").toFloat())
                .setExpirationDuration(options.optLong("expirationDuration", Geofence.NEVER_EXPIRE))
                .setTransitionTypes((if (dwell > 0) Geofence.GEOFENCE_TRANSITION_DWELL else Geofence.GEOFENCE_TRANSITION_ENTER) or Geofence.GEOFENCE_TRANSITION_EXIT)
                .apply { if (dwell > 0) setLoiteringDelay(dwell.toInt()) }
                .apply { if (options.has("notificationResponsiveness")) setNotificationResponsiveness(options.getInt("notificationResponsiveness")) }
                .build()
        }

        /** Add [regions] to Play services: [initial] raises an entry for a fence the device is already inside. */
        internal fun arm(app: Context, regions: List<JSONObject>, initial: Boolean, done: (Exception?) -> Unit) {
            if (regions.isEmpty() || app.checkSelfPermission(FINE) != PackageManager.PERMISSION_GRANTED) return done(null)
            val request = GeofencingRequest.Builder().setInitialTrigger(if (initial) GeofencingRequest.INITIAL_TRIGGER_ENTER else 0)
                .addGeofences(regions.map(::fence)).build()
            try {
                LocationServices.getGeofencingClient(app).addGeofences(request, pendingIntent(app))
                    .addOnSuccessListener { done(null) }.addOnFailureListener { done(it) }
            } catch (e: SecurityException) {
                done(e)
            }
        }
    }

    private val app: Context get() = context.application
    private val store get() = ExactRegions.store(app)
    /** The transition being delivered now: one at a time. */
    private var delivering = false
    private var said: String? = null

    init {
        if (!context.agent) {
            ExactRegions.listen(::deliver)
            // Every launch re-arms the store's fences without an initial trigger (D14, Re-arming), and delivers what
            // the store kept undelivered.
            arm(app, store.list(), initial = false) { e -> if (e != null) context.log("geofencing: re-arm failed: $e") }
            android.os.Handler(android.os.Looper.getMainLooper()).post(::deliver)
        }
    }

    override fun later(name: String, args: JSONObject, reply: (JSONObject) -> Unit) {
        when (name) {
            "geofencingStatus" -> reply(status())
            "geofences" -> reply(JSONObject().put("exact:answer", store.answer()))
            "addGeofence" -> allowed(reply) { add(args, reply) }
            "removeGeofence" -> remove(args.optString("name"), reply)
            else -> reply(error("NotSupportedError"))
        }
    }

    override fun resumed() {
        val before = said ?: return
        if (status().toString() != before) context.changed("geofencing.geofencingStatus")
    }

    override fun destroy(done: () -> Unit) {
        ExactRegions.listen(null)
        done()
    }

    private fun error(code: String) = JSONObject().put("error", code)

    // Permission (D13)

    private fun has(p: String) = app.checkSelfPermission(p) == PackageManager.PERMISSION_GRANTED

    private fun playServices() = GoogleApiAvailability.getInstance().isGooglePlayServicesAvailable(app) == ConnectionResult.SUCCESS

    private fun status(): JSONObject {
        val fine = context.permission(FINE)
        val background = if (Build.VERSION.SDK_INT >= 29) context.permission(BACKGROUND) else fine
        val state = when {
            fine.state == "granted" && background.state == "granted" -> "granted"
            fine.state == "denied" || background.state == "denied" -> "denied"
            else -> "prompt"
        }
        val label = if (Build.VERSION.SDK_INT >= 30) app.packageManager.backgroundPermissionOptionLabel.toString() else null
        val out = JSONObject().put("available", playServices()).put("state", state).put("apple", JSONObject.NULL)
            .put("android", JSONObject().put("playServices", playServices()).put("backgroundPermissionOptionLabel", label ?: JSONObject.NULL).put("limit", LIMIT))
        said = out.toString()
        return out
    }

    /** [then] once both decisions allow it: fine and coarse, then background location alone (D13). */
    private fun allowed(reply: (JSONObject) -> Unit, then: () -> Unit) {
        if (!playServices()) return reply(error("NotSupportedError"))
        val state = status().getString("state")
        when {
            state == "granted" -> then()
            state == "denied" -> reply(error("PERMISSION_DENIED"))
            else -> fine {
                if (!has(FINE)) return@fine reply(error("PERMISSION_DENIED"))
                background {
                    context.changed("geofencing.geofencingStatus")
                    if (Build.VERSION.SDK_INT < 29 || has(BACKGROUND)) then() else reply(error("PERMISSION_DENIED"))
                }
            }
        }
    }

    private fun fine(done: () -> Unit) {
        if (has(FINE)) return done()
        context.requestPermissions(arrayOf(FINE, COARSE)) {
            val a = context.activity
            if (!has(FINE) && a != null && !a.shouldShowRequestPermissionRationale(FINE)) {
                app.getSharedPreferences("exact.permissions", Context.MODE_PRIVATE).edit().putBoolean(FINE, true).apply()
            }
            done()
        }
    }

    /** From Android 11 the platform opens the app's location settings page rather than a dialog; the request settles
     * when the activity resumes and the permission is read again. */
    private fun background(done: () -> Unit) {
        if (Build.VERSION.SDK_INT < 29 || has(BACKGROUND)) return done()
        context.askPermission(BACKGROUND, done)
    }

    // Fences (D10, D11)

    private fun add(args: JSONObject, reply: (JSONObject) -> Unit) {
        val name = args.optString("name")
        val android = args.optJSONObject("android") ?: JSONObject()
        val limit = android.optInt("limit", LIMIT).coerceIn(LIMIT, 100)
        var radius = args.optDouble("radius", 100.0)
        if (radius < 100) {
            context.log("addGeofence $name: radius $radius raised to 100 (LLP 1133.021 D11)")
            radius = 100.0
        }
        if (store.list().count { it.getString("name") != name } >= limit) return reply(error("QuotaExceededError"))
        val region = JSONObject().put("name", name).put("latitude", args.optDouble("latitude")).put("longitude", args.optDouble("longitude"))
            .put("radius", radius).put("options", android)
        arm(app, listOf(region), initial = true) { e ->
            if (e != null) {
                val code = (e as? ApiException)?.statusCode
                return@arm reply(error(when (code) {
                    GeofenceStatusCodes.GEOFENCE_TOO_MANY_GEOFENCES, GeofenceStatusCodes.GEOFENCE_TOO_MANY_PENDING_INTENTS -> "QuotaExceededError"
                    GeofenceStatusCodes.GEOFENCE_NOT_AVAILABLE -> "NotSupportedError"
                    else -> if (e is SecurityException) "PERMISSION_DENIED" else "OperationError"
                }))
            }
            store.put(region)
            context.changed("geofencing.geofences")
            reply(JSONObject().put("name", name).put("latitude", region.getDouble("latitude")).put("longitude", region.getDouble("longitude"))
                .put("radius", radius).put("inside", JSONObject.NULL))
        }
    }

    private fun remove(name: String, reply: (JSONObject) -> Unit) {
        store.remove(name)
        LocationServices.getGeofencingClient(app).removeGeofences(listOf(name))
        context.changed("geofencing.geofences")
        reply(JSONObject())
    }

    // Crossings (D14)

    /** The store's undelivered transitions, raised one at a time as wakes; each is forgotten only when its
     * invocation completes, so an expired one stays for the next launch. */
    private fun deliver() {
        if (delivering) return
        val region = store.pending().firstOrNull() ?: return
        val pending = region.getJSONObject("pending")
        val name = region.getString("name")
        val enter = pending.getString("event") == "enter"
        val geofence = JSONObject().put("name", name).put("latitude", region.getDouble("latitude")).put("longitude", region.getDouble("longitude"))
            .put("radius", region.getDouble("radius")).put("inside", enter)
        val event = JSONObject().put("geofence", geofence).put("timestamp", pending.getDouble("timestamp"))
            .put("android", pending.optJSONObject("extra") ?: JSONObject.NULL)
        delivering = true
        context.wake(if (enter) "geofenceenter" else "geofenceleave", JSONArray().put(event), "region:$name:${pending.getDouble("timestamp").toLong()}") { outcome ->
            delivering = false
            context.log("geofencing: $name ${pending.getString("event")} $outcome")
            if (outcome == "completed" || outcome == "unheard") {
                store.delivered(name)
                context.changed("geofencing.geofences")
                deliver()
            }
        }
    }
}
