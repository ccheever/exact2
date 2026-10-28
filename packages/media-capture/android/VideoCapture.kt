// @exact/media-capture's video half on Android (LLP 1133.008 D1-D3, D6): cameras as Media CameraCapture's video tracks
// through CameraX, each in the session's track table with its state and, while leased, its capture (`VideoSource`),
// to which a consumer such as @exact/camera's viewfinder adds its use cases. MediaCapture.kt routes its names here
// for a video track and `video=`.
//
// - One capture for the process (D3): CameraX's `ProcessCameraProvider` with one lifecycle this file drives, started
//   while a lease is live and the app is in front, and the whole set of use cases bound in one `bindToLifecycle` on
//   main whenever it changes. A lease on another camera while one is live waits, `in-use`.
// - A consumer that shows the capture takes the lease whenever its origin is the person's or the app's own code and
//   the app is in front (§14); a tool's or a wake's takes none. The last consumer gone, the track is `idle` and the
//   set is unbound.
// - The camera's state becomes `muteReason`: `CameraState`'s errors for another app's use or the policy, and the
//   activity's stop, `background`.
// - The permission is `CAMERA`, its state a fact from the host's one history (`ExtensionContext.permission`, D8 item 7).
// - Under the agent no provider is made and nothing is asked: two cameras, "Agent back camera" and "Agent front
//   camera", whose consumers are told `liveVideo(null)`.
@file:androidx.annotation.OptIn(androidx.camera.camera2.interop.ExperimentalCamera2Interop::class)

package exact.mediacapture

import android.Manifest
import android.content.Context
import android.content.pm.PackageManager
import androidx.camera.camera2.interop.Camera2CameraInfo
import androidx.camera.core.Camera
import androidx.camera.core.CameraInfo
import androidx.camera.core.CameraSelector
import androidx.camera.core.CameraState
import androidx.camera.core.UseCase
import androidx.camera.lifecycle.ProcessCameraProvider
import androidx.core.content.ContextCompat
import androidx.lifecycle.Lifecycle
import androidx.lifecycle.LifecycleOwner
import androidx.lifecycle.LifecycleRegistry
import dev.exact.canvas.ExtensionContext
import dev.exact.canvas.TrackConsumer
import dev.exact.canvas.TrackEntry
import dev.exact.canvas.TrackError
import dev.exact.canvas.TrackOwner
import dev.exact.canvas.VideoSource
import org.json.JSONArray
import org.json.JSONObject

/** One session's video tracks: the owner the track table names for them. */
internal class VideoTracks(private val context: ExtensionContext, private val nextId: () -> Int) : TrackOwner {
    internal class Track(val id: String, val stream: String) {
        var label = ""
        var enabled = true
        var muted = true
        var muteReason: String? = "idle"
        var readyState = "live"
        /** The camera, by CameraX's selector: `back`, `front` or a camera id. */
        var camera = "back"
        var settings = JSONObject()
        var capabilities = JSONObject()
        val consumers = LinkedHashMap<TrackConsumer, String>()
        var leased = false
        var waiting: String? = null
        var platformMute: String? = null
    }

    private val tracks = LinkedHashMap<String, Track>()
    private var overlay: String? = null
    private var opened = false

    fun owns(id: String?) = id != null && tracks.containsKey(id)
    private fun say(line: String) = context.log("media-capture: $line")
    private fun announce() { context.changed("media-capture.mediaStreamTrack"); disclose() }

    /** Under the agent, each track for `state.extensions` (LLP 1133.008 §9, Disclosure). */
    private fun disclose() = context.state(JSONObject().put("videoTracks", JSONArray(tracks.values.map { t ->
        shape(t).put("consumers", JSONArray(t.consumers.values.sorted())).put("lease", t.leased).put("waiting", t.waiting ?: JSONObject.NULL)
    })))
    private fun error(code: String) = JSONObject().put("error", code)

    // The names

    fun open(args: JSONObject, stream: String, reply: (JSONObject) -> Unit) {
        val constraints = args.optJSONObject("videoConstraints") ?: JSONObject()
        val make = {
            val t = Track("track-${context.session}-${nextId()}", stream)
            t.camera = constraints.optString("deviceId").ifEmpty { if (constraints.optString("facingMode") == "user") "front" else "back" }
            read(t)
            t.platformMute = overlay
            tracks[t.id] = t
            opened = true
            val entry = TrackEntry(t.id, "video", owner = this)
            entry.state = { shape(t) }
            entry.video = { if (t.leased && !context.agent) CameraCapture else null }
            context.tracks.add(entry)
            say("$stream: opened with ${t.id} (${t.label})")
            announce()
            context.changed("media-capture.enumerateDevices")
            reply(shape(t))
        }
        if (context.agent) return make()
        if (!context.application.packageManager.hasSystemFeature(PackageManager.FEATURE_CAMERA_ANY)) return reply(error("NotFoundError"))
        when (context.permission(PERMISSION).state) {
            "granted" -> CameraCapture.provider(context.application) { make() }
            "denied" -> reply(error("NotAllowedError"))
            else -> context.askPermission(PERMISSION) {
                context.changed("media-capture.cameraStatus")
                if (context.permission(PERMISSION).state == "granted") CameraCapture.provider(context.application) { make() } else reply(error("NotAllowedError"))
            }
        }
    }

    fun applyConstraints(args: JSONObject, reply: (JSONObject) -> Unit) {
        val t = tracks[args.optString("track")] ?: return reply(error("NotFoundError"))
        if (t.readyState != "live") return reply(shape(t))
        val next = args.optString("deviceId").ifEmpty { null }
            ?: when (args.optString("facingMode")) { "user" -> "front"; "environment" -> "back"; else -> null }
        if (next != null && next != t.camera) {
            // Another camera under the same id (D1): a capture start (D6), refused while another lease holds the camera.
            if (context.origin != "app" || !context.foreground) return reply(error("NotAllowedError"))
            if (t.leased && CameraCapture.holders.size > 1) return reply(error("NotReadableError"))
            t.camera = next
            if (t.leased && !context.agent) CameraCapture.rebind(context.application, t.camera)
            read(t)
            say("${t.id}: moved to ${t.label}")
            announce()
            return reply(shape(t))
        }
        val kept = JSONObject()
        for (k in listOf("zoom", "torch")) if (args.has(k) && !args.isNull(k)) kept.put(k, args.get(k))
        if (context.agent) {
            if (kept.has("torch") && t.capabilities.optBoolean("torch")) t.settings.put("torch", kept.getBoolean("torch"))
        } else if (t.leased) CameraCapture.apply(kept)
        announce()
        reply(shape(t))
    }

    fun stop(id: String): JSONObject { val t = tracks.getValue(id); end(t, "stopTrack"); return shape(t) }

    fun setEnabled(id: String, enabled: Boolean): JSONObject {
        val t = tracks.getValue(id)
        t.enabled = enabled
        if (!enabled && t.leased) { release(t); mute(t, "idle") }
        if (enabled && !t.leased && t.consumers.isNotEmpty() && t.readyState == "live") lease(t, context.origin)
        announce()
        return shape(t)
    }

    fun state(id: String) = shape(tracks.getValue(id))

    fun devices(): JSONArray {
        val out = JSONArray()
        if (context.agent) {
            if (!opened) return out.put(device("", "", null))
            return out.put(device(AGENT_BACK, "Agent back camera", "environment")).put(device(AGENT_FRONT, "Agent front camera", "user"))
        }
        if (context.permission(PERMISSION).state != "granted") return out.put(device("", "", null))
        val provider = CameraCapture.ready ?: return out
        for (info in provider.availableCameraInfos) {
            val facing = when (info.lensFacing) { CameraSelector.LENS_FACING_BACK -> "environment"; CameraSelector.LENS_FACING_FRONT -> "user"; else -> null }
            val id = Camera2CameraInfo.from(info).cameraId
            out.put(device(id, "Camera $id${facing?.let { " ($it)" } ?: ""}", facing))
        }
        return out
    }

    private fun device(id: String, label: String, facing: String?) = JSONObject().put("deviceId", id).put("kind", "videoinput")
        .put("label", label).put("groupId", id).put("facingMode", facing ?: JSONObject.NULL)

    fun status(): JSONObject {
        val p = context.permission(PERMISSION)
        return JSONObject().put("available", context.application.packageManager.hasSystemFeature(PackageManager.FEATURE_CAMERA_ANY))
            .put("state", p.state).put("apple", JSONObject.NULL)
            .put("android", JSONObject().put("shouldShowRationale", p.shouldShowRationale).put("permanentlyDenied", p.permanentlyDenied))
    }

    fun preferred(value: JSONObject?) {
        overlay = if (value?.optBoolean("muted") == true) value.optString("muteReason").ifEmpty { "interrupted" } else null
        for (t in tracks.values.toList()) {
            if (t.readyState != "live") continue
            if (value?.optString("readyState") == "ended") { end(t, "the agent ended it"); continue }
            t.platformMute = overlay
            if (t.leased) settle(t)
        }
    }

    /** The activity stopped or started again: the capture's lifecycle follows, and its tracks mute `background`. */
    fun foreground(shown: Boolean) {
        if (!context.agent) CameraCapture.foreground(shown)
        for (t in tracks.values) {
            if (t.readyState != "live" || !t.leased) continue
            t.platformMute = if (shown) overlay else "background"
            settle(t)
        }
    }

    fun dispose(id: String) { tracks.values.filter { it.id == id || it.stream == id }.forEach { end(it, "disposed") } }
    fun endAll(why: String) = tracks.values.toList().forEach { end(it, why) }

    // Leases (D3)

    override fun subscribe(id: String, consumer: TrackConsumer, start: Boolean, origin: String): () -> Unit {
        val t = tracks[id] ?: throw TrackError("NotFoundError")
        if (t.readyState == "ended") { consumer.ended(); return {} }
        t.consumers[consumer] = origin
        when {
            t.leased -> {
                consumer.liveVideo(if (context.agent) null else CameraCapture)
                if (t.muted) consumer.muted(t.muteReason ?: "idle")
            }
            origin == "app" && t.enabled && context.foreground -> {
                say("${t.id}: a consumer takes the lease ($origin origin)")
                lease(t, origin)
                if (!t.leased) consumer.muted(t.muteReason ?: "idle")
            }
            else -> {
                say("${t.id}: a consumer takes no lease ($origin origin)")
                consumer.muted(t.muteReason ?: "idle")
            }
        }
        disclose()
        return {
            if (t.consumers.remove(consumer) != null && t.consumers.isEmpty()) {
                t.waiting = null
                if (t.leased) { release(t); mute(t, "idle") }
            }
            disclose()
        }
    }

    private fun each(t: Track, f: (TrackConsumer) -> Unit) = t.consumers.keys.toList().forEach(f)

    private fun mute(t: Track, reason: String) {
        if (t.muted && t.muteReason == reason) return
        t.muted = true
        t.muteReason = reason
        each(t) { it.muted(reason) }
        announce()
    }

    private fun unmute(t: Track) {
        if (!t.muted) return
        t.muted = false
        t.muteReason = null
        each(t) { it.unmuted() }
        announce()
    }

    private fun settle(t: Track) { t.platformMute?.let { mute(t, it) } ?: unmute(t) }

    private fun lease(t: Track, origin: String) {
        if (t.leased || t.readyState != "live") return
        if (!context.agent && !CameraCapture.lease(context.application, t, this)) {
            t.waiting = origin
            CameraCapture.wait(t, this)
            say("${t.id}: waits for the camera, in use by another lease")
            mute(t, "in-use")
            return
        }
        t.leased = true
        t.waiting = null
        read(t)
        each(t) { it.liveVideo(if (context.agent) null else CameraCapture) }
        settle(t)
    }

    private fun release(t: Track) {
        if (!t.leased) return
        t.leased = false
        if (!context.agent) CameraCapture.unlease(t)
    }

    /** Another lease ended: a track of this session waiting for the camera takes it, with its kept origin. */
    fun retry(t: Track) {
        val origin = t.waiting ?: return
        t.waiting = null
        if (t.consumers.isNotEmpty() && t.readyState == "live" && t.enabled) lease(t, origin)
    }

    fun interrupted(t: Track, reason: String?) { t.platformMute = reason ?: overlay; settle(t) }
    fun changed(t: Track) { read(t); announce() }

    private fun end(t: Track, why: String) {
        if (t.readyState != "live") return
        t.readyState = "ended"
        t.waiting = null
        release(t)
        say("${t.id}: ended ($why)")
        each(t) { it.ended() }
        t.consumers.clear()
        announce()
    }

    // Answers

    private fun read(t: Track) {
        if (context.agent) {
            val back = t.camera != "front" && t.camera != AGENT_FRONT
            t.camera = if (back) "back" else "front"
            t.label = if (back) "Agent back camera" else "Agent front camera"
            val facing = if (back) "environment" else "user"
            t.settings = JSONObject().put("deviceId", if (back) AGENT_BACK else AGENT_FRONT).put("groupId", "agent").put("facingMode", facing)
                .put("width", 1280).put("height", 960).put("aspectRatio", 4.0 / 3.0).put("frameRate", 30).put("resizeMode", "none")
            if (back) t.settings.put("torch", false)
            t.capabilities = JSONObject().put("facingMode", JSONArray().put(facing)).put("torch", back)
            return
        }
        val info = CameraCapture.info(t.camera) ?: return
        val facing = when (info.lensFacing) { CameraSelector.LENS_FACING_BACK -> "environment"; CameraSelector.LENS_FACING_FRONT -> "user"; else -> null }
        val id = Camera2CameraInfo.from(info).cameraId
        t.label = "Camera $id${facing?.let { " ($it)" } ?: ""}"
        val zoom = info.zoomState.value
        t.settings = JSONObject().put("deviceId", id).put("groupId", id).put("resizeMode", "none")
        facing?.let { t.settings.put("facingMode", it) }
        zoom?.let { t.settings.put("zoom", it.zoomRatio.toDouble()) }
        if (info.hasFlashUnit()) t.settings.put("torch", info.torchState.value == 1)
        t.capabilities = JSONObject().put("torch", info.hasFlashUnit())
        facing?.let { t.capabilities.put("facingMode", JSONArray().put(it)) }
        zoom?.let { t.capabilities.put("zoom", JSONObject().put("min", it.minZoomRatio.toDouble()).put("max", it.maxZoomRatio.toDouble()).put("step", 0.1)) }
    }

    private fun shape(t: Track): JSONObject {
        val out = JSONObject().put("id", t.id).put("kind", "video").put("label", t.label).put("enabled", t.enabled)
            .put("muted", t.muted).put("readyState", t.readyState).put("settings", t.settings).put("capabilities", t.capabilities)
        if (t.muted) t.muteReason?.let { out.put("muteReason", it) }
        if (!context.agent) out.put("android", JSONObject().put("isClientSilenced", false).put("cameraId", t.settings.optString("deviceId"))
            .put("lensFacing", if (t.camera == "front") "front" else "back"))
        return out
    }

    companion object {
        const val PERMISSION = Manifest.permission.CAMERA
        const val AGENT_BACK = "agent-camera-back"
        const val AGENT_FRONT = "agent-camera-front"
    }
}

/** The process's one capture (D3): a provider, one lifecycle, and the set of use cases bound together. */
internal object CameraCapture : VideoSource {
    var ready: ProcessCameraProvider? = null
        private set
    private val waiting = ArrayList<() -> Unit>()
    private val owner = object : LifecycleOwner {
        val registry = LifecycleRegistry(this)
        override val lifecycle: Lifecycle get() = registry
    }
    private var inFront = true
    private var selected = "back"
    private val useCases = LinkedHashSet<UseCase>()
    private val objects = HashMap<String, Any>()
    val holders = LinkedHashMap<VideoTracks.Track, VideoTracks>()
    override var camera: Camera? = null
        private set

    init { owner.registry.currentState = Lifecycle.State.CREATED }

    /** The provider, made once, then [done] on main. */
    fun provider(app: Context, done: () -> Unit) {
        if (ready != null) return done()
        waiting += done
        if (waiting.size > 1) return
        val future = ProcessCameraProvider.getInstance(app)
        future.addListener({
            ready = runCatching { future.get() }.getOrNull()
            waiting.toList().also { waiting.clear() }.forEach { it() }
        }, ContextCompat.getMainExecutor(app))
    }

    fun selector(camera: String): CameraSelector = when (camera) {
        "back" -> CameraSelector.DEFAULT_BACK_CAMERA
        "front" -> CameraSelector.DEFAULT_FRONT_CAMERA
        else -> CameraSelector.Builder()
            .addCameraFilter { infos -> infos.filter { Camera2CameraInfo.from(it).cameraId == camera } }.build()
    }

    fun info(camera: String): CameraInfo? = ready?.let { p -> runCatching { selector(camera).filter(p.availableCameraInfos).firstOrNull() }.getOrNull() }

    /** A lease for [t]: false while another camera's lease holds the process's capture. */
    fun lease(app: Context, t: VideoTracks.Track, tracks: VideoTracks): Boolean {
        if (holders.isNotEmpty() && t.camera != selected) return false
        val first = holders.isEmpty()
        holders[t] = tracks
        if (first) { selected = t.camera; bind(); started() }
        return true
    }

    fun unlease(t: VideoTracks.Track) {
        if (holders.remove(t) == null || holders.isNotEmpty()) return
        owner.registry.currentState = Lifecycle.State.CREATED
        ready?.unbindAll()
        camera = null
        useCases.clear()
        objects.clear()
        // A track waiting for the camera takes it.
        for ((track, owners) in waitingTracks()) owners.retry(track)
    }

    private val waiters = ArrayList<Pair<VideoTracks.Track, VideoTracks>>()
    fun wait(t: VideoTracks.Track, tracks: VideoTracks) { waiters += t to tracks }
    private fun waitingTracks() = waiters.filter { it.first.waiting != null }.also { waiters.clear() }

    fun rebind(app: Context, next: String) { selected = next; bind() }

    fun foreground(shown: Boolean) { inFront = shown; started() }

    private fun started() {
        owner.registry.currentState = if (holders.isNotEmpty() && inFront) Lifecycle.State.STARTED else Lifecycle.State.CREATED
    }

    /** The whole set bound in one call on main (CameraX binds use cases together). */
    private fun bind() {
        val provider = ready ?: return
        provider.unbindAll()
        camera = runCatching { provider.bindToLifecycle(owner, selector(selected), *useCases.toTypedArray()) }.getOrNull()
        camera?.cameraInfo?.cameraState?.observe(owner) { state ->
            val reason = when (state.error?.code) {
                null -> null
                CameraState.ERROR_CAMERA_IN_USE, CameraState.ERROR_MAX_CAMERAS_IN_USE -> "in-use"
                else -> "interrupted"
            }
            for ((t, tracks) in holders) tracks.interrupted(t, reason)
        }
    }

    override fun add(useCase: Any) { if (useCases.add(useCase as UseCase)) bind() }
    override fun remove(useCase: Any) { if (useCases.remove(useCase as UseCase)) bind() }

    @Suppress("UNCHECKED_CAST")
    override fun <T : Any> shared(key: String, make: () -> T): T = objects.getOrPut(key, make) as T

    override fun apply(constraints: JSONObject) {
        val control = camera?.cameraControl ?: return
        if (constraints.has("zoom")) control.setZoomRatio(constraints.getDouble("zoom").toFloat())
        if (constraints.has("torch")) control.enableTorch(constraints.getBoolean("torch"))
        for ((t, tracks) in holders) tracks.changed(t)
    }
}
