package dev.exact.canvas

import android.content.Context
import android.os.Handler
import android.os.Looper
import org.json.JSONArray
import org.json.JSONObject
import java.lang.ref.WeakReference

/**
 * The host's region store (LLP 1133.021 D14): the regions a package watches and each one's undelivered transition,
 * readable and writable from any process state with no session, since a package's geofence receiver runs in a
 * process the system started for its broadcast. The store is the truth: Play services keeps no list of an app's
 * geofences and drops them at reboot and when location is turned off, so the package re-arms from it.
 *
 * A transition is written first and delivered second. [wake] hands the store's transitions to a live session's
 * package; with none (Android has no headless session yet, §9 item 6) the store keeps them, and the package delivers
 * them when a session next makes it (D14 step 3), each with its original time.
 */
object ExactRegions {
    private val main = Handler(Looper.getMainLooper())
    private var listener: WeakReference<() -> Unit>? = null

    /** The store, in the app's own preferences. */
    @JvmStatic fun store(context: Context): RegionStore = RegionStore(context.applicationContext)

    /** The live session's package hears of new transitions through [deliver] (null: none listens). */
    @JvmStatic fun listen(deliver: (() -> Unit)?) { listener = deliver?.let { WeakReference(it) } }

    /** Ask for a region wake: the live session's package delivers the store's transitions; false when no session is
     * live in this process, and the store keeps them for the next launch. */
    @JvmStatic fun wake(context: Context): Boolean {
        val deliver = listener?.get() ?: return false
        main.post(deliver)
        return true
    }
}

/** The regions and their transitions ([ExactRegions.store]). Every call is synchronous and safe on any thread. */
class RegionStore internal constructor(private val context: Context) {
    private val prefs get() = context.getSharedPreferences("exact.regions", Context.MODE_PRIVATE)

    private fun all(): JSONObject = synchronized(RegionStore::class.java) { JSONObject(prefs.getString("regions", "{}") ?: "{}") }
    private fun keep(regions: JSONObject) = synchronized(RegionStore::class.java) { prefs.edit().putString("regions", regions.toString()).commit() }

    /** Every region: `{name, latitude, longitude, radius, inside?, options?, pending?}`, by name. */
    fun list(): List<JSONObject> = all().let { r -> r.keys().asSequence().sorted().map { r.getJSONObject(it) }.toList() }

    /** Watch [region] (its `name` the key), replacing one of that name and its undelivered transition. */
    fun put(region: JSONObject) = synchronized(RegionStore::class.java) {
        val regions = all()
        regions.put(region.getString("name"), JSONObject(region.toString()).apply { remove("pending") })
        keep(regions)
    }

    fun remove(name: String) = synchronized(RegionStore::class.java) { val r = all(); r.remove(name); keep(r) }

    /** Region [name] was entered ([inside]) or left at [at] (ms since the epoch): `inside` set, and the one
     * undelivered transition replaced by this one, the latest (D11's collapse). [extra] rides with it. */
    fun transition(name: String, inside: Boolean, at: Double, extra: JSONObject? = null) = synchronized(RegionStore::class.java) {
        val regions = all()
        val r = regions.optJSONObject(name) ?: return@synchronized
        r.put("inside", inside)
        r.put("pending", JSONObject().put("event", if (inside) "enter" else "leave").put("timestamp", at).apply { extra?.let { put("extra", it) } })
        keep(regions)
    }

    /** The regions with an undelivered transition. */
    fun pending(): List<JSONObject> = list().filter { it.has("pending") }

    /** Region [name]'s transition was delivered: forgotten. */
    fun delivered(name: String) = synchronized(RegionStore::class.java) {
        val regions = all()
        regions.optJSONObject(name)?.remove("pending")
        keep(regions)
    }

    /** The regions as an answer's list, without what the store keeps for itself. */
    fun answer(): JSONArray = JSONArray().apply {
        for (r in list()) put(JSONObject().put("name", r.getString("name")).put("latitude", r.getDouble("latitude"))
            .put("longitude", r.getDouble("longitude")).put("radius", r.getDouble("radius")).put("inside", r.opt("inside") ?: JSONObject.NULL))
    }
}
