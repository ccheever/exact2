// @exact/camera on Android (LLP 1133.008 D1-D5, D8, D10): the viewfinder `exact-camera`, CameraX's `PreviewView`
// bound as a `Preview` use case, over a video track of @exact/media-capture's, and `takePhoto` through the capture's
// one `ImageCapture`, written to the app's files.
//
// - A viewfinder is a consumer of its track while it is displayed and its `srcObject` names the track (D3), joined
//   with the origin the runner reports, so a tool's display takes no lease (§14). The process has one `Preview`: it
//   shows in the viewfinder displayed last, and another displayed at once shows its empty box (D10).
// - The `ImageCapture` is the capture's process object (D3): each session's photos in turn. Unsaid, it is CameraX's
//   own (`CAPTURE_MODE_MINIMIZE_LATENCY`, flash off, JPEG, D2); `fillLightMode` and the `android.*` words set the
//   flash, mode and quality, a builder field built again when it changes. The shutter sound is
//   `MediaActionSound.SHUTTER_CLICK` at each capture, unless `android.shutterSound` is false where the platform
//   allows it.
// - Hosted views take no touches beyond a click (`host/android/README.md`): no tap to focus or pinch to zoom yet (D8
//   item 6, recorded as a tell).
// - Under the agent no capture exists: the viewfinder shows a drawn test card, mirrored for a `user` camera, and
//   `takePhoto` writes it as JPEG, or the app asset `prefer extension camera.picture={"path":…}` names, unchanged.
//   The drawn card is Android's own (the package's card.jpg is not in the APK), so its bytes differ from the other
//   hosts'.
// - Tier 3 (D5): `useCasesHatch` sees the use cases before each bind, `imageCaptureHatch` each `ImageCapture.Builder`
//   after the module's words, `cameraHatch` the `Camera` after each bind.
package exact.camera

import android.content.Context
import android.graphics.Bitmap
import android.graphics.BitmapFactory
import android.graphics.Canvas
import android.graphics.Color
import android.graphics.Paint
import android.media.MediaActionSound
import android.view.View
import android.widget.ImageView
import androidx.camera.core.Camera
import androidx.camera.core.ImageCapture
import androidx.camera.core.ImageCaptureException
import androidx.camera.core.Preview
import androidx.camera.view.PreviewView
import androidx.core.content.ContextCompat
import dev.exact.canvas.ExactExtension
import dev.exact.canvas.ExtensionContext
import dev.exact.canvas.TrackConsumer
import dev.exact.canvas.VideoSource
import org.json.JSONArray
import org.json.JSONObject
import java.io.ByteArrayOutputStream
import java.io.File

class CameraModule(context: ExtensionContext) : ExactExtension(context) {
    companion object {
        @JvmStatic var useCasesHatch: ((MutableList<Any>) -> Unit)? = null
        @JvmStatic var imageCaptureHatch: ((ImageCapture.Builder) -> Unit)? = null
        @JvmStatic var cameraHatch: ((Camera) -> Unit)? = null
        /** The one `Preview` of the process, and the viewfinder that shows it. */
        private var preview: Preview? = null
        private var showing: PreviewView? = null
    }

    /** What the runner said of a viewfinder, and what it consumes. */
    private class ViewState {
        var view: View? = null
        var src: String? = null
        var changes = 0
        var reports = 0
        var shown = false
        var madeBy = "app"
        var sub: Sub? = null
    }

    private class Sub(val track: String, val origin: String) : TrackConsumer {
        var live = false
        var leave: () -> Unit = {}
        var onLive: (VideoSource?) -> Unit = {}
        var onBlank: () -> Unit = {}
        override fun liveVideo(source: VideoSource?) { live = true; onLive(source) }
        override fun muted(reason: String) = onBlank()
        override fun unmuted() {}
        override fun ended() { live = false; onBlank() }
    }

    private val views = HashMap<String, ViewState>()
    private var picture: String? = null
    private var taken = 0
    private val photos = JSONArray()
    private val sound by lazy { MediaActionSound().apply { load(MediaActionSound.SHUTTER_CLICK) } }

    private fun say(line: String) = context.log("camera: $line")
    private fun error(code: String) = JSONObject().put("error", code)

    override fun later(name: String, args: JSONObject, reply: (JSONObject) -> Unit) {
        when (name) {
            "takePhoto" -> takePhoto(args, reply)
            "getPhotoCapabilities" -> reply(capabilities(args))
            "picture" -> reply(JSONObject().put("path", picture ?: ""))
            else -> reply(error("NotSupportedError"))
        }
    }

    override fun preferred(values: JSONObject) {
        picture = values.optJSONObject("picture")?.optString("path")?.ifEmpty { null }
    }

    // Viewfinders (D3, D8 items 1 and 2)

    override fun view(tag: String, session: String, id: String, context: Context, props: JSONObject, emit: (String, String) -> Unit): View {
        require(tag == "exact-camera") { "no view $tag" }
        val v: View = if (this.context.agent) ImageView(context).apply { setBackgroundColor(Color.BLACK) }
            else PreviewView(context).apply { implementationMode = PreviewView.ImplementationMode.COMPATIBLE }
        state(id).view = v
        update(v, props)
        return v
    }

    override fun update(view: View, props: JSONObject) {
        val id = views.entries.firstOrNull { it.value.view === view }?.key ?: return
        // `contain` unsaid is `<video>`'s; PreviewView's own default (FILL_CENTER) is a written `cover` (D2).
        when (view) {
            is PreviewView -> view.scaleType = when (props.optString("object-fit")) {
                "cover" -> PreviewView.ScaleType.FILL_CENTER
                else -> PreviewView.ScaleType.FIT_CENTER
            }
            is ImageView -> view.scaleType = when (props.optString("object-fit")) {
                "cover" -> ImageView.ScaleType.CENTER_CROP
                "fill" -> ImageView.ScaleType.FIT_XY
                else -> ImageView.ScaleType.FIT_CENTER
            }
        }
        val v = state(id)
        if (!props.has("srcObject")) v.src = ""
        else props.optString("srcObject").let { if (it != v.src) { v.src = it; v.changes += 1 } }
        reconcile(id)
    }

    override fun destroyView(view: View) {
        val v = views.values.firstOrNull { it.view === view } ?: return
        // The reader lets a hidden viewfinder's view go (`display: none`) and makes another when it shows again,
        // under the same id: what the runner said of it stands, since it reports a `srcObject` it bound only once.
        v.view = null
        v.sub?.leave?.invoke()
        v.sub = null
    }

    override fun displayed(session: String, id: String, shown: Boolean, origin: String) {
        val v = state(id)
        v.shown = shown
        if (shown) v.madeBy = origin
        reconcile(id)
    }

    override fun bound(session: String, id: String, prop: String, origin: String) {
        if (prop != "srcObject") return
        val v = state(id)
        v.reports += 1
        v.madeBy = origin
        reconcile(id)
    }

    private fun state(id: String) = views.getOrPut(id) { ViewState() }

    private fun reconcile(id: String) {
        val v = views[id] ?: return
        val src = v.src ?: ""
        val want = v.view != null && v.shown && src.isNotEmpty() && v.reports >= v.changes && context.tracks.lookup(src)?.kind == "video"
        v.sub?.let { if (!want || it.track != src) { it.leave(); v.sub = null; blank(v.view) } }
        v.sub?.let { if (want && !it.live && v.madeBy == "app" && it.origin != "app") { it.leave(); v.sub = null } }
        if (!want || v.sub != null) return
        val sub = Sub(src, v.madeBy)
        sub.onLive = { source -> show(v, sub, source) }
        sub.onBlank = { blank(v.view) }
        v.sub = sub
        try {
            sub.leave = context.tracks.subscribe(src, sub, false, v.madeBy)
            say("viewfinder $id consumes $src (${v.madeBy} origin)")
        } catch (e: Exception) {
            v.sub = null
            say("viewfinder $id: ${e.message}")
        }
    }

    private fun show(v: ViewState, sub: Sub, source: VideoSource?) {
        if (v.sub !== sub) return
        val facing = context.tracks.lookup(sub.track)?.state?.invoke()?.optJSONObject("settings")?.optString("facingMode")
        when (val view = v.view) {
            is ImageView -> {
                view.setImageBitmap(agentBitmap())
                view.scaleX = if (facing == "user") -1f else 1f
            }
            is PreviewView -> {
                val p = preview ?: Preview.Builder().build().also { preview = it }
                p.surfaceProvider = view.surfaceProvider
                showing = view
                val cases = mutableListOf<Any>(p)
                useCasesHatch?.invoke(cases)
                cases.forEach(source!!::add)
                (source.camera as? Camera)?.let { cameraHatch?.invoke(it) }
            }
        }
    }

    private fun blank(view: View?) {
        when (view) {
            is ImageView -> view.setImageDrawable(null)
            is PreviewView -> if (showing === view) {
                showing = null
                preview?.surfaceProvider = null
            }
        }
    }

    // Photos (D1, D2)

    /** Under the agent, each photo for `state.extensions` (§9, Disclosure), by path and SHA-256. */
    private fun disclose(path: String, track: String, bytes: ByteArray) {
        val sha = java.security.MessageDigest.getInstance("SHA-256").digest(bytes).joinToString("") { "%02x".format(it) }
        photos.put(JSONObject().put("path", path).put("sha256", sha).put("track", track).put("origin", context.origin))
        context.state(JSONObject().put("photos", photos))
    }

    private fun takePhoto(args: JSONObject, reply: (JSONObject) -> Unit) {
        val entry = context.tracks.lookup(args.optString("track")) ?: return reply(error("NotFoundError"))
        if (entry.kind != "video") return reply(error("NotSupportedError"))
        val s = entry.state?.invoke() ?: JSONObject()
        // A track that is not shown takes no photo (D1): ended, disabled, or muted (`idle` among them).
        if (s.optString("readyState") != "live" || !s.optBoolean("enabled") || s.optBoolean("muted", true)) return reply(error("InvalidStateError"))
        val ext = "jpg"
        val path = args.optString("path").ifEmpty { null }?.let { if (it.endsWith(".$ext")) it else "$it.$ext" } ?: "app:/tmp/camera/${++taken}.$ext"
        val file = context.file(path) ?: return reply(error("UnknownError"))
        file.parentFile?.mkdirs()
        if (context.agent) {
            val bytes = agentBytes() ?: return reply(error("UnknownError"))
            file.writeBytes(bytes)
            say("took $path (${bytes.size} bytes, the agent's picture)")
            disclose(path, entry.id, bytes)
            return reply(photo(path, file))
        }
        val source = entry.video?.invoke() ?: return reply(error("InvalidStateError"))
        val android = args.optJSONObject("android") ?: JSONObject()
        val capture = imageCapture(source, args, android)
        if (android.optBoolean("shutterSound", true) || MediaActionSound.mustPlayShutterSound()) sound.play(MediaActionSound.SHUTTER_CLICK)
        // A photo in flight is a consumer: its lease holds until the file is written (D3).
        val holder = Sub(entry.id, "app")
        holder.leave = context.tracks.subscribe(entry.id, holder, false, "app")
        capture.takePicture(ImageCapture.OutputFileOptions.Builder(file).build(), ContextCompat.getMainExecutor(context.application),
            object : ImageCapture.OnImageSavedCallback {
                override fun onImageSaved(output: ImageCapture.OutputFileResults) {
                    holder.leave()
                    say("took $path (${file.length()} bytes)")
                    reply(photo(path, file))
                }
                override fun onError(e: ImageCaptureException) {
                    holder.leave()
                    say("takePhoto: ${e.message}")
                    reply(error("UnknownError"))
                }
            })
    }

    /** The capture's `ImageCapture`, built again when a builder word changes (D4). */
    private fun imageCapture(source: VideoSource, args: JSONObject, android: JSONObject): ImageCapture {
        val mode = when (android.optString("captureMode")) {
            "maximizeQuality" -> ImageCapture.CAPTURE_MODE_MAXIMIZE_QUALITY
            "zeroShutterLag" -> ImageCapture.CAPTURE_MODE_ZERO_SHUTTER_LAG
            else -> ImageCapture.CAPTURE_MODE_MINIMIZE_LATENCY
        }
        val quality = android.optInt("jpegQuality", 0)
        val key = "camera.imageCapture.$mode.$quality"
        val made = source.shared(key) {
            val builder = ImageCapture.Builder().setCaptureMode(mode)
            if (quality in 1..100) builder.setJpegQuality(quality)
            imageCaptureHatch?.invoke(builder)
            builder.build().also { source.add(it) }
        }
        val flash = android.optString("flashMode").ifEmpty { null } ?: when (args.optString("fillLightMode")) { "flash" -> "on"; "auto" -> "auto"; else -> "off" }
        made.flashMode = when (flash) { "on" -> ImageCapture.FLASH_MODE_ON; "auto" -> ImageCapture.FLASH_MODE_AUTO; else -> ImageCapture.FLASH_MODE_OFF }
        return made
    }

    /** `Photo`: its size as it is shown, its orientation applied. */
    private fun photo(path: String, file: File): JSONObject {
        val o = BitmapFactory.Options().apply { inJustDecodeBounds = true }
        BitmapFactory.decodeFile(file.path, o)
        val turned = runCatching { android.media.ExifInterface(file.path).getAttributeInt(android.media.ExifInterface.TAG_ORIENTATION, 1) >= 5 }.getOrDefault(false)
        return JSONObject().put("path", path).put("type", "image/jpeg").put("size", file.length())
            .put("width", if (turned) o.outHeight else o.outWidth).put("height", if (turned) o.outWidth else o.outHeight)
    }

    private fun capabilities(args: JSONObject): JSONObject {
        val entry = context.tracks.lookup(args.optString("track")) ?: return error("NotFoundError")
        if (entry.kind != "video") return error("NotSupportedError")
        val settings = entry.state?.invoke()?.optJSONObject("settings") ?: JSONObject()
        val torch = entry.state?.invoke()?.optJSONObject("capabilities")?.optBoolean("torch") == true
        val fixed = { n: Int -> JSONObject().put("min", n).put("max", n).put("step", 1) }
        return JSONObject().put("redEyeReduction", "never").put("imageWidth", fixed(settings.optInt("width"))).put("imageHeight", fixed(settings.optInt("height")))
            .put("fillLightMode", if (torch) JSONArray().put("auto").put("off").put("flash") else JSONArray().put("off"))
    }

    // The agent's picture (§9)

    private fun agentBytes(): ByteArray? {
        picture?.let { p -> return runCatching { context.application.assets.open("app/$p").use { it.readBytes() } }.getOrNull() }
        val out = ByteArrayOutputStream()
        agentBitmap().compress(Bitmap.CompressFormat.JPEG, 85, out)
        return out.toByteArray()
    }

    /** A test card whose top and sides read at a glance: colour bars across the top, an F. */
    private fun agentBitmap(): Bitmap {
        picture?.let { p -> runCatching { context.application.assets.open("app/$p").use(BitmapFactory::decodeStream) }.getOrNull()?.let { return it } }
        val b = Bitmap.createBitmap(1280, 960, Bitmap.Config.ARGB_8888)
        val c = Canvas(b)
        c.drawColor(Color.rgb(0x20, 0x24, 0x2c))
        val bars = intArrayOf(0xffe53935.toInt(), 0xfffb8c00.toInt(), 0xfffdd835.toInt(), 0xff43a047.toInt(), 0xff1e88e5.toInt(), 0xff8e24aa.toInt(), Color.WHITE)
        val paint = Paint(Paint.ANTI_ALIAS_FLAG)
        bars.forEachIndexed { i, colour -> paint.color = colour; c.drawRect(i * 1280f / bars.size, 0f, (i + 1) * 1280f / bars.size, 160f, paint) }
        paint.color = Color.WHITE
        paint.textSize = 56f
        c.drawText("TOP LEFT", 40f, 240f, paint)
        paint.textAlign = Paint.Align.RIGHT
        c.drawText("TOP RIGHT", 1240f, 240f, paint)
        paint.textAlign = Paint.Align.CENTER
        paint.textSize = 260f
        c.drawText("F", 640f, 700f, paint)
        return b
    }
}
