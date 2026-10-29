// @exact/maps on Android (LLP 1133.021 D1-D5, §12): `exact-map` is the Maps SDK's `MapView`, its markers the SDK's
// default red `Marker`s. The key is the app's (`com.google.android.geo.API_KEY`, which the build writes from the
// variable `app.json` names); without one the SDK shows its own blank map.
//
// - The camera is a centre and a zoom, which is Google's own zoom (D1); a camera prop moves the map only when its
//   value changes, and only that prop: a new `zoomLevel` zooms about the centre the map shows now.
// - `moveend` is the camera-idle listener, whoever moved the map, raised as the declared event (a message whose text
//   is `{"exact:event": "moveend", "args": [camera]}`, LLP 1133.021 §9 item 2); `change` is a marker the person
//   tapped, or `""` when they tap the map away from one, never a selection a prop asked for.
// - Under the agent (unless bound with `--bind maps`) the map type is `none` under a tile overlay of plain tiles
//   (§12), so nothing is fetched and a screenshot is stable; the markers stay the SDK's.
// - `MapView` must hear its activity's lifecycle: made, it is created, started and resumed; the app leaving the
//   foreground pauses and stops it, coming back starts and resumes it, and the node going destroys it. The
//   reader forwards no other lifecycle to a hosted view, so `onSaveInstanceState` is not heard (§9 item 4).
// - The hatch (D5) is given the `GoogleMap` at `onMapReady`, after the module's own setup.
package exact.maps

import android.content.ComponentCallbacks2
import android.content.Context
import android.content.res.Configuration
import android.graphics.Bitmap
import android.graphics.Canvas
import android.graphics.Paint
import android.view.View
import com.google.android.gms.maps.CameraUpdateFactory
import com.google.android.gms.maps.GoogleMap
import com.google.android.gms.maps.MapView
import com.google.android.gms.maps.model.LatLng
import com.google.android.gms.maps.model.Marker
import com.google.android.gms.maps.model.MarkerOptions
import com.google.android.gms.maps.model.Tile
import com.google.android.gms.maps.model.TileOverlayOptions
import com.google.android.gms.maps.model.TileProvider
import dev.exact.canvas.ExactExtension
import dev.exact.canvas.ExtensionContext
import org.json.JSONArray
import org.json.JSONObject
import java.io.ByteArrayOutputStream
import kotlin.math.roundToLong

class MapsModule(context: ExtensionContext) : ExactExtension(context) {
    companion object {
        /** The app's access hatch (D5): each map at `onMapReady`, after the module's own setup. */
        @JvmStatic var hatch: ((GoogleMap) -> Unit)? = null
    }

    private val maps = LinkedHashMap<MapView, OneMap>()
    private var shown = true
    /** `onLowMemory`, which the reader does not forward, heard from the application. */
    private val memory = object : ComponentCallbacks2 {
        override fun onConfigurationChanged(newConfig: Configuration) {}
        @Deprecated("Deprecated in Java") override fun onLowMemory() { maps.keys.forEach { it.onLowMemory() } }
        override fun onTrimMemory(level: Int) {}
    }

    init { context.application.registerComponentCallbacks(memory) }

    override fun later(name: String, args: JSONObject, reply: (JSONObject) -> Unit) = reply(JSONObject().put("error", "NotSupportedError"))

    override fun view(tag: String, session: String, id: String, context: Context, props: JSONObject, emit: (String, String) -> Unit): View {
        require(tag == "exact-map") { "no view $tag" }
        val view = MapView(context)
        val map = OneMap(id, view, this.context.agent, emit, this.context::log)
        maps[view] = map
        view.onCreate(null)
        if (shown) { view.onStart(); view.onResume() }
        map.set(props)
        view.getMapAsync(map::ready)
        emit("load", "")
        return view
    }

    override fun update(view: View, props: JSONObject) { maps[view]?.set(props) }

    override fun destroyView(view: View) {
        val map = maps.remove(view) ?: return
        if (shown) { map.view.onPause(); map.view.onStop() }
        map.view.onDestroy()
    }

    override fun foreground(shown: Boolean) {
        if (shown == this.shown) return
        this.shown = shown
        for (v in maps.keys) if (shown) { v.onStart(); v.onResume() } else { v.onPause(); v.onStop() }
    }

    override fun destroy(done: () -> Unit) {
        context.application.unregisterComponentCallbacks(memory)
        done()
    }
}

/** One map: its props, its markers, and what it tells the app. */
private class OneMap(val id: String, val view: MapView, val plain: Boolean, val emit: (String, String) -> Unit, val log: (String) -> Unit) {
    private var map: GoogleMap? = null
    private var props = JSONObject()
    private var first = true
    /** Writing a prop moves the map; that move's idle is still reported, as the camera came to rest. */
    private var applying = false
    private val markers = LinkedHashMap<String, Marker>()

    fun ready(m: GoogleMap) {
        map = m
        if (plain) {
            m.mapType = GoogleMap.MAP_TYPE_NONE
            m.addTileOverlay(TileOverlayOptions().tileProvider(PlainTiles).zIndex(-1f))
        }
        m.setOnMarkerClickListener { marker ->
            val id = marker.tag as? String ?: return@setOnMarkerClickListener false
            if (id != props.optString("selected")) {
                log("map ${this.id}: selected $id")
                props.put("selected", id)
                emit("change", id)
            }
            false // the SDK's own selection: the info window shows and the camera centres.
        }
        m.setOnMapClickListener {
            if (props.optString("selected").isNotEmpty()) {
                log("map $id: selected none")
                props.put("selected", "")
                emit("change", "")
            }
        }
        m.setOnCameraIdleListener {
            val c = m.cameraPosition
            val round = { v: Double, places: Int -> Math.pow(10.0, places.toDouble()).let { k -> (v * k).roundToLong() / k } }
            val camera = JSONObject().put("latitude", round(c.target.latitude, 6)).put("longitude", round(c.target.longitude, 6))
                .put("zoom", round(c.zoom.toDouble(), 3))
                .put("android", JSONObject().put("bearing", round(c.bearing.toDouble(), 2)).put("tilt", round(c.tilt.toDouble(), 2)))
            log("map $id: moveend ${camera.get("latitude")} ${camera.get("longitude")} ${camera.get("zoom")}")
            emit("message", JSONObject().put("exact:event", "moveend").put("args", JSONArray().put(camera)).toString())
        }
        log("map $id: ${if (plain) "plain" else "live"} tiles")
        val kept = props
        props = JSONObject()
        first = true
        set(kept)
        MapsModule.hatch?.invoke(m)
    }

    fun set(next: JSONObject) {
        val m = map
        if (m == null) { props = next; return }
        val changed = { k: String -> first || next.opt(k)?.toString() != props.opt(k)?.toString() }
        val animate = !first
        val centre = if (changed("latitude") || changed("longitude")) next.optDouble("latitude").takeIf { !it.isNaN() }?.let { lat ->
            next.optDouble("longitude").takeIf { !it.isNaN() }?.let { LatLng(lat, it) } } else null
        val zoom = if (changed("zoomLevel")) next.optString("zoomLevel").toFloatOrNull() ?: (if (first) 12f else null) else null
        val update = when {
            centre != null && zoom != null -> CameraUpdateFactory.newLatLngZoom(centre, zoom)
            centre != null -> CameraUpdateFactory.newLatLng(centre)
            zoom != null -> CameraUpdateFactory.zoomTo(zoom)
            else -> null
        }
        if (update != null) if (animate) m.animateCamera(update) else m.moveCamera(update)
        if (changed("markers")) {
            markers.values.forEach { it.remove() }
            markers.clear()
            val list = try { JSONArray(next.optString("markers", "[]")) } catch (e: Exception) { JSONArray() }
            for (i in 0 until list.length()) {
                val o = list.optJSONObject(i) ?: continue
                val id = o.optString("id")
                if (id.isEmpty()) continue
                val options = MarkerOptions().position(LatLng(o.optDouble("latitude"), o.optDouble("longitude")))
                    .title(o.optString("title").ifEmpty { null }).contentDescription(o.optString("title").ifEmpty { id })
                m.addMarker(options)?.let { it.tag = id; markers[id] = it }
            }
        }
        if (changed("selected") || changed("markers")) {
            val selected = next.optString("selected")
            markers.forEach { (id, marker) -> if (id == selected) marker.showInfoWindow() else marker.hideInfoWindow() }
        }
        next.optString("aria-label").ifEmpty { null }?.let { view.contentDescription = it }
        props = JSONObject(next.toString())
        first = false
    }
}

/** §12's plain tiles: a flat colour with each tile's top and left edges, the same at every zoom. */
private object PlainTiles : TileProvider {
    private val png: ByteArray by lazy {
        val side = 256
        val bitmap = Bitmap.createBitmap(side, side, Bitmap.Config.ARGB_8888)
        val canvas = Canvas(bitmap)
        canvas.drawColor(0xFFE8ECEF.toInt())
        val edge = Paint().apply { color = 0xFFC9D1D8.toInt(); strokeWidth = 1f }
        canvas.drawLine(0f, 0f, side.toFloat(), 0f, edge)
        canvas.drawLine(0f, 0f, 0f, side.toFloat(), edge)
        ByteArrayOutputStream().also { bitmap.compress(Bitmap.CompressFormat.PNG, 100, it) }.toByteArray()
    }

    override fun getTile(x: Int, y: Int, zoom: Int): Tile = Tile(256, 256, png)
}
