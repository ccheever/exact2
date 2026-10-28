// @exact/geocoding on Android (LLP 1133.021 D6-D9): the platform's `android.location.Geocoder`, which needs no
// library. From API 33 its listener forms answer; before, the synchronous form, deprecated in 33, runs off the main
// thread, since it "may hit the network". Five results for `geocode` and one for `reverseGeocode` (Google's advice of
// 1 to 5), `android.maxResults` overriding; no match answers `[]`. No geocoder (`Geocoder.isPresent()` false) is
// `NotSupportedError`; a failure to reach the service is `NetworkError`.
package exact.geocoding

import android.location.Address
import android.location.Geocoder
import android.os.Build
import android.os.Handler
import android.os.Looper
import dev.exact.canvas.ExactExtension
import dev.exact.canvas.ExtensionContext
import org.json.JSONArray
import org.json.JSONObject
import java.util.Locale
import java.util.concurrent.Executors

class GeocodingModule(context: ExtensionContext) : ExactExtension(context) {
    private val main = Handler(Looper.getMainLooper())
    private val worker by lazy { Executors.newSingleThreadExecutor() }
    /** The sends let go: their answers are dropped (Android's geocoder has no cancel). */
    private val cancelled = HashSet<Int>()

    override fun later(name: String, args: JSONObject, reply: (JSONObject) -> Unit) {
        val android = args.optJSONObject("android") ?: JSONObject()
        val locale = android.optString("locale").ifEmpty { null }?.let(Locale::forLanguageTag) ?: Locale.getDefault()
        val request = context.request
        val answer: (List<Address>?, String?) -> Unit = { places, error ->
            main.post {
                if (request != null && cancelled.remove(request)) return@post
                if (error != null) reply(JSONObject().put("error", error))
                else reply(JSONObject().put("exact:answer", JSONArray().apply { places.orEmpty().forEach { put(place(it)) } }))
            }
        }
        when (name) {
            "geocodingStatus" -> reply(JSONObject().put("available", Geocoder.isPresent()))
            "geocode" -> {
                val (address, max) = args.optString("address") to android.optInt("maxResults", 5)
                lookup(locale, answer, { g, l -> if (Build.VERSION.SDK_INT >= 33) Later.name(g, address, max, l) }) { g ->
                    @Suppress("DEPRECATION") g.getFromLocationName(address, max)
                }
            }
            "reverseGeocode" -> {
                val (lat, lon, max) = Triple(args.optDouble("latitude"), args.optDouble("longitude"), android.optInt("maxResults", 1))
                lookup(locale, answer, { g, l -> if (Build.VERSION.SDK_INT >= 33) Later.location(g, lat, lon, max, l) }) { g ->
                    @Suppress("DEPRECATION") g.getFromLocation(lat, lon, max)
                }
            }
            else -> reply(JSONObject().put("error", "NotSupportedError"))
        }
    }

    override fun cancel(request: Int) { cancelled += request }

    /** The listener form from API 33 ([later]), else [now] synchronously on the worker. */
    private fun lookup(locale: Locale, answer: (List<Address>?, String?) -> Unit, later: (Geocoder, (List<Address>?, String?) -> Unit) -> Unit,
                       now: (Geocoder) -> List<Address>?) {
        if (!Geocoder.isPresent()) return answer(null, "NotSupportedError")
        val g = Geocoder(context.application, locale)
        if (Build.VERSION.SDK_INT >= 33) return later(g, answer)
        worker.execute {
            try { answer(now(g).orEmpty(), null) } catch (e: java.io.IOException) { answer(null, "NetworkError") } catch (e: IllegalArgumentException) { answer(null, "TypeError") }
        }
    }

    /** API 33's listener forms. */
    @android.annotation.TargetApi(33)
    private object Later {
        private fun listener(answer: (List<Address>?, String?) -> Unit) = object : Geocoder.GeocodeListener {
            override fun onGeocode(addresses: MutableList<Address>) = answer(addresses, null)
            override fun onError(message: String?) = answer(null, "NetworkError")
        }
        fun name(g: Geocoder, address: String, max: Int, answer: (List<Address>?, String?) -> Unit) = g.getFromLocationName(address, max, listener(answer))
        fun location(g: Geocoder, lat: Double, lon: Double, max: Int, answer: (List<Address>?, String?) -> Unit) = g.getFromLocation(lat, lon, max, listener(answer))
    }

    /** D6's `Place`, with Android's structured fields in `android`. */
    private fun place(a: Address): JSONObject {
        val lines = (0..a.maxAddressLineIndex).mapNotNull { a.getAddressLine(it) }.joinToString(", ")
        fun opt(v: String?): Any = v ?: JSONObject.NULL
        return JSONObject()
            .put("latitude", if (a.hasLatitude()) a.latitude else 0.0)
            .put("longitude", if (a.hasLongitude()) a.longitude else 0.0)
            .put("name", opt(a.featureName))
            .put("fullAddress", opt(lines.ifEmpty { null }))
            .put("city", opt(a.locality))
            .put("countryCode", opt(a.countryCode))
            .put("country", opt(a.countryName))
            .put("apple", JSONObject.NULL)
            .put("android", JSONObject().put("adminArea", opt(a.adminArea)).put("subAdminArea", opt(a.subAdminArea))
                .put("subLocality", opt(a.subLocality)).put("thoroughfare", opt(a.thoroughfare))
                .put("subThoroughfare", opt(a.subThoroughfare)).put("postalCode", opt(a.postalCode)))
    }
}
