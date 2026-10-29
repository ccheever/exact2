// @exact/media-capture on Android (LLP 1133.009 D1-D3, D6, D8, D9): the microphone as Media Capture's tracks, one
// `AudioRecord` for the process, each track in the session's track table (`context.tracks`) so a package that consumes
// a track (`@exact/recording`) reaches it by id.
//
// - One capture (D3): one `AudioRecord` whose read loop, on a thread of its own, hands each read to every tap. It
//   runs while any track holds a lease, configured by the lease that opened it, and stops with the last.
// - A track holds a lease while it has a consumer (D6): its first consumer takes it, and after use only a start does;
//   a reader joining an idle track hears silence and opens nothing. A tool's or a wake's consumer never leases.
// - Unsaid, the source is `MIC` with no effect (D2); `echoCancellation` written true asks for `AcousticEchoCanceler`
//   where the device has one, else `VOICE_COMMUNICATION`; `android.audioSource` and `android.privacySensitive` are
//   the platform's words (D4).
// - The capture policy silences an app whose capture another app takes: `isClientSilenced` mutes the tracks,
//   `silenced` (D8, API 29).
// - The permission is `RECORD_AUDIO`, its state a fact with the history geolocation keeps (D9).
// - The background (LLP 1133.033 D5): with `device.microphone-background` the capture claims the host's `capture`
//   service, type `microphone`, at the first lease and lets it go at the last, so it records on while the app is hidden.
//   Without the line, or with the claim refused, Android lets a hidden app hear only silence, so leaving the app mutes
//   the tracks, `background`, and lets their lease go, as on iOS; coming back takes it again.
// - Under the agent no record is made and nothing is asked: a stream with one track, "Agent microphone".
// - Tier 3 (D5): `MediaCaptureModule.hatch` is given the `AudioRecord.Builder` once per capture, after the words.
package exact.mediacapture

import android.Manifest
import android.annotation.SuppressLint
import android.content.Context
import android.content.pm.PackageManager
import android.media.AudioFormat
import android.media.AudioManager
import android.media.AudioRecord
import android.media.AudioRecordingConfiguration
import android.media.MediaRecorder
import android.media.audiofx.AcousticEchoCanceler
import android.media.audiofx.AutomaticGainControl
import android.media.audiofx.NoiseSuppressor
import android.os.Build
import android.os.Handler
import android.os.Looper
import dev.exact.canvas.AudioSource
import dev.exact.canvas.ExactExtension
import dev.exact.canvas.ExtensionContext
import dev.exact.canvas.ForegroundClaim
import dev.exact.canvas.TrackConsumer
import dev.exact.canvas.TrackEntry
import dev.exact.canvas.TrackError
import dev.exact.canvas.TrackOwner
import org.json.JSONArray
import org.json.JSONObject

class MediaCaptureModule(context: ExtensionContext) : ExactExtension(context), TrackOwner {
    companion object {
        /** The app's access hatch (D5): the builder, once per capture, after the module's words are set. */
        @JvmStatic var hatch: ((AudioRecord.Builder) -> Unit)? = null
        private const val PERMISSION = Manifest.permission.RECORD_AUDIO
        private val SOURCES = mapOf(
            "mic" to MediaRecorder.AudioSource.MIC, "camcorder" to MediaRecorder.AudioSource.CAMCORDER,
            "voiceRecognition" to MediaRecorder.AudioSource.VOICE_RECOGNITION,
            "voiceCommunication" to MediaRecorder.AudioSource.VOICE_COMMUNICATION,
            "unprocessed" to MediaRecorder.AudioSource.UNPROCESSED, "voicePerformance" to 10,
        )
    }

    private class Track(val id: String, val stream: String, val label: String, val options: JSONObject) {
        var enabled = true
        var muted = true
        var muteReason: String? = "idle"
        var readyState = "live"
        val consumers = LinkedHashMap<TrackConsumer, Unit>()
        var active = false
        var used = false
        var leased = false
        var platformMute: String? = null
    }

    private val main = Handler(Looper.getMainLooper())
    private val app: Context get() = context.application
    private val history get() = app.getSharedPreferences("exact.media-capture", Context.MODE_PRIVATE)
    private val tracks = LinkedHashMap<String, Track>()
    private var next = 0
    /** The cameras (VideoCapture.kt, LLP 1133.008). */
    private val cameras = VideoTracks(context) { ++next }
    private var overlay: String? = null
    private var opened = false
    /** The `capture` service's claim, held while any track is active (LLP 1133.033 D5). */
    private var background: ForegroundClaim? = null

    init { if (!context.agent) Capture.listeners += this }

    override fun later(name: String, args: JSONObject, reply: (JSONObject) -> Unit) {
        val t = tracks[args.optString("track")]
        if (cameras.owns(args.optString("track"))) {
            val id = args.optString("track")
            when (name) {
                "applyConstraints" -> return cameras.applyConstraints(args, reply)
                "stopTrack" -> return reply(cameras.stop(id))
                "setTrackEnabled" -> return reply(cameras.setEnabled(id, args.optBoolean("enabled", true)))
                "mediaStreamTrack" -> return reply(cameras.state(id))
            }
        }
        when (name) {
            "getUserMedia" -> if (args.optBoolean("video", false) || args.has("videoConstraints")) withCamera(args, reply) else getUserMedia(args, reply)
            "applyConstraints" -> reply(error(if (t == null) "NotFoundError" else "NotSupportedError"))
            "cameraStatus" -> reply(cameras.status())
            "stopTrack" -> if (t == null) reply(error("NotFoundError")) else { end(t, "stopTrack"); reply(shape(t)) }
            "setTrackEnabled" -> {
                if (t == null) return reply(error("NotFoundError"))
                t.enabled = args.optBoolean("enabled", true)
                // Turning a track back on is a start: it reopens an idle one that has consumers (D6).
                if (t.enabled && !t.active && t.consumers.isNotEmpty() && t.readyState == "live") activate(t)
                announce()
                reply(shape(t))
            }
            // A read of no track (`track=""`): an app reads before it holds one.
            "mediaStreamTrack" -> reply(if (args.optString("track").isEmpty()) JSONObject().put("id", "").put("kind", "audio").put("label", "")
                .put("enabled", false).put("muted", true).put("readyState", "ended").put("settings", JSONObject()) else t?.let(::shape) ?: error("NotFoundError"))
            "enumerateDevices" -> {
                val all = devices().getJSONArray("devices")
                val more = cameras.devices()
                for (i in 0 until more.length()) all.put(more.get(i))
                reply(JSONObject().put("devices", all))
            }
            "microphoneStatus" -> reply(status())
            else -> reply(error("NotSupportedError"))
        }
    }

    /** A stream with a camera's track, and the microphone's first where it asks for both (LLP 1133.008 D1). */
    private fun withCamera(args: JSONObject, reply: (JSONObject) -> Unit) {
        val stream = "stream-${context.session}-${++next}"
        val video = { made: List<JSONObject> ->
            cameras.open(args, stream) { v ->
                // What opened first is let go when the camera fails (LLP 1133.009 D3).
                if (v.has("error")) {
                    made.forEach { m -> tracks[m.optString("id")]?.let { end(it, "the camera failed") } }
                    reply(v)
                } else {
                    val all = made + v
                    context.tracks.add(TrackEntry(stream, "stream", all.map { it.getString("id") }))
                    reply(JSONObject().put("id", stream).put("active", true).put("tracks", JSONArray(all)))
                }
            }
        }
        if (!args.optBoolean("audio", false) && !args.has("audioConstraints")) return video(emptyList())
        getUserMedia(args) { a ->
            if (a.has("error")) reply(a)
            else {
                val made = a.getJSONArray("tracks")
                video((0 until made.length()).map { made.getJSONObject(it) })
            }
        }
    }

    private fun error(code: String) = JSONObject().put("error", code)
    private fun announce() = context.changed("media-capture.mediaStreamTrack")

    private fun getUserMedia(args: JSONObject, reply: (JSONObject) -> Unit) {
        // No kind asked for is Media Capture's TypeError, before anything opens.
        if (!args.optBoolean("audio", false) && !args.has("audioConstraints")) return reply(error("TypeError"))
        // A recognizer holding the microphone exclusively (LLP 1133.012 D8): nothing opens beside it.
        if (context.tracks.held("audio")) return reply(error("NotReadableError"))
        val make = {
            next += 1
            val t = Track("track-${context.session}-$next", "stream-${context.session}-$next",
                if (context.agent) "Agent microphone" else "Microphone", args)
            t.platformMute = overlay
            tracks[t.id] = t
            opened = true
            context.tracks.add(TrackEntry(t.id, "audio", owner = this))
            context.tracks.add(TrackEntry(t.stream, "stream", listOf(t.id)))
            context.log("media-capture: ${t.stream} opened with ${t.id}")
            announce()
            context.changed("media-capture.enumerateDevices")
            reply(JSONObject().put("id", t.stream).put("active", true).put("tracks", JSONArray().put(shape(t))))
        }
        if (context.agent) return make()
        when (status().getString("state")) {
            "granted" -> make()
            "denied" -> reply(error("NotAllowedError"))
            else -> ask { if (granted()) make() else reply(error("NotAllowedError")) }
        }
    }

    // Tracks and leases (D6)

    override fun subscribe(id: String, consumer: TrackConsumer, start: Boolean, origin: String): () -> Unit {
        val t = tracks[id] ?: throw TrackError("NotFoundError")
        if (t.readyState == "ended") { consumer.ended(); return {} }
        t.consumers[consumer] = Unit
        when {
            t.active -> {
                consumer.live(if (context.agent) null else Capture)
                if (t.muted) consumer.muted(t.muteReason ?: "idle")
            }
            // While a recognizer holds the microphone exclusively, a track stays idle (LLP 1133.012 D8).
            origin == "app" && t.enabled && (!t.used || start) && !context.tracks.held("audio") -> activate(t)
            else -> consumer.muted(t.muteReason ?: "idle")
        }
        return {
            if (t.consumers.remove(consumer) != null && t.consumers.isEmpty() && t.active) deactivate(t)
        }
    }

    override fun leased(id: String) = tracks[id]?.active == true

    private fun each(t: Track, f: (TrackConsumer) -> Unit) = t.consumers.keys.toList().forEach(f)

    private fun mute(t: Track, reason: String) {
        if (t.muted && t.muteReason == reason) return
        t.muted = true; t.muteReason = reason
        each(t) { it.muted(reason) }
        announce()
    }

    private fun unmute(t: Track) {
        if (!t.muted) return
        t.muted = false; t.muteReason = null
        each(t) { it.unmuted() }
        announce()
    }

    private fun settle(t: Track) { t.platformMute?.let { mute(t, it) } ?: unmute(t) }

    private fun activate(t: Track) {
        t.active = true; t.used = true
        if (!context.agent && !t.leased) {
            try {
                Capture.lease(app, t.options)
                t.leased = true
            } catch (e: Exception) {
                context.log("media-capture: ${t.id} did not open: $e")
                t.platformMute = "interrupted"
            }
        }
        each(t) { it.live(if (context.agent) null else Capture) }
        settle(t)
        if (t.leased || context.agent) claimBackground()
    }

    private fun deactivate(t: Track) {
        t.active = false
        release(t)
        mute(t, "idle")
        releaseBackground()
    }

    /** Claim the `capture` service where the app's lines declare it, at a lease taken in the foreground (D5). */
    private fun claimBackground() {
        if (background != null || !context.foregroundService.declared("capture", "microphone")) return
        background = context.foregroundService.claim("capture", setOf("microphone"), context.origin, lost = { why ->
            background = null
            context.log("media-capture: the background claim was refused after it was made: $why")
            if (!context.foreground) foreground(false)
        })
    }

    /** The last active track is gone: the service goes with its claim. */
    private fun releaseBackground() {
        if (tracks.values.any { it.active }) return
        background?.release()
        background = null
    }

    private fun release(t: Track) {
        if (!t.leased) return
        t.leased = false
        Capture.unlease()
    }

    private fun end(t: Track, why: String) {
        if (t.readyState != "live") return
        t.readyState = "ended"; t.active = false
        release(t)
        context.log("media-capture: ${t.id} ended ($why)")
        each(t) { it.ended() }
        t.consumers.clear()
        announce()
        releaseBackground()
    }

    /** The capture policy's silencing of this app's capture, or its end (D8). */
    internal fun silenced(on: Boolean) {
        for (t in tracks.values) {
            if (t.readyState != "live") continue
            if (on) t.platformMute = "silenced" else if (t.platformMute == "silenced") t.platformMute = null
            if (t.active) settle(t)
        }
    }

    /**
     * The app left the foreground or came back (LLP 1133.033 D5): a capture under its claim runs on; any other one
     * mutes `background` and lets its lease go, since a hidden app hears only silence, and takes it again on return.
     */
    override fun foreground(shown: Boolean) {
        cameras.foreground(shown)
        if (background != null) return
        for (t in tracks.values) {
            if (t.readyState != "live" || !t.active) continue
            if (shown) {
                t.platformMute = overlay
                activate(t)
            } else {
                t.platformMute = "background"
                release(t)
                settle(t)
            }
        }
    }

    /** Under the agent: a preferred `mediaStreamTrack` is the platform's change to every live track (D10 item 11), and
     * a preferred `foregroundService` refusal the platform's refusal of the claim (LLP 1133.033 D21). */
    override fun preferred(values: JSONObject) {
        context.foregroundService.refusing = values.optJSONObject("foregroundService")?.optString("refused")?.ifEmpty { null }
        val value = values.optJSONObject("mediaStreamTrack")
        cameras.preferred(value)
        overlay = if (value?.optBoolean("muted") == true) value.optString("muteReason").ifEmpty { "interrupted" } else null
        for (t in tracks.values) {
            if (t.readyState != "live") continue
            t.platformMute = overlay
            if (t.active) settle(t)
        }
    }

    override fun dispose(ids: List<String>) {
        for (id in ids) tracks.values.filter { it.id == id || it.stream == id }.forEach { end(it, "disposed") }
        ids.forEach(cameras::dispose)
    }

    override fun resumed() { context.changed("media-capture.microphoneStatus") }

    override fun destroy(done: () -> Unit) {
        tracks.values.toList().forEach { end(it, "the session ended") }
        cameras.endAll("the session ended")
        Capture.listeners -= this
        done()
    }

    // The fact (D9), with geolocation's history: a rejection with no rationale owed afterwards is permanent.

    private fun granted() = app.checkSelfPermission(PERMISSION) == PackageManager.PERMISSION_GRANTED

    private fun status(): JSONObject {
        val granted = granted()
        val prefs = history
        if (granted != prefs.getBoolean("granted", false)) prefs.edit().putBoolean("granted", granted).putBoolean("permanent", false).apply()
        val activity = context.activity
        val rationale = !granted && activity != null && activity.shouldShowRequestPermissionRationale(PERMISSION)
        val permanent = !granted && prefs.getBoolean("permanent", false)
        val available = app.packageManager.hasSystemFeature(PackageManager.FEATURE_MICROPHONE)
        return JSONObject().put("available", available)
            .put("state", if (granted) "granted" else if (permanent) "denied" else "prompt")
            .put("apple", JSONObject.NULL)
            .put("android", JSONObject().put("shouldShowRationale", rationale).put("permanentlyDenied", permanent))
    }

    private fun ask(done: () -> Unit) {
        context.requestPermissions(arrayOf(PERMISSION)) {
            val activity = context.activity
            if (!granted() && activity != null && !activity.shouldShowRequestPermissionRationale(PERMISSION)) {
                history.edit().putBoolean("permanent", true).apply()
            }
            context.changed("media-capture.microphoneStatus")
            done()
        }
    }

    private fun devices(): JSONObject {
        val known = if (context.agent) opened else granted()
        val list = JSONArray()
        if (!known) return JSONObject().put("devices", list.put(device("", "")))
        if (context.agent) return JSONObject().put("devices", list.put(device("agent-microphone", "Agent microphone")))
        val manager = app.getSystemService(Context.AUDIO_SERVICE) as AudioManager
        for (d in manager.getDevices(AudioManager.GET_DEVICES_INPUTS)) list.put(device(d.id.toString(), d.productName.toString()))
        return JSONObject().put("devices", list)
    }

    private fun device(id: String, label: String) = JSONObject().put("deviceId", id).put("kind", "audioinput").put("label", label).put("groupId", id)

    private fun shape(t: Track): JSONObject {
        val settings = JSONObject()
            .put("deviceId", if (context.agent) "agent-microphone" else "default")
            .put("sampleRate", Capture.RATE).put("sampleSize", 16).put("channelCount", 1)
            .put("echoCancellation", Capture.processing(t.options)).put("autoGainControl", Capture.processing(t.options))
            .put("noiseSuppression", Capture.processing(t.options))
        val out = JSONObject().put("id", t.id).put("kind", "audio").put("label", t.label).put("enabled", t.enabled)
            .put("muted", t.muted).put("readyState", t.readyState).put("settings", settings).put("apple", JSONObject.NULL)
            .put("android", JSONObject().put("isClientSilenced", t.platformMute == "silenced"))
        if (t.muted) out.put("muteReason", t.muteReason)
        return out
    }

    /** The process's one capture (D3). */
    @SuppressLint("MissingPermission")
    object Capture : AudioSource {
        const val RATE = 44100
        override val sampleRate = RATE
        override val channels = 1
        val listeners = mutableSetOf<MediaCaptureModule>()
        private var record: AudioRecord? = null
        private var thread: Thread? = null
        private var leases = 0
        private var callback: AudioManager.AudioRecordingCallback? = null
        private val taps = java.util.concurrent.CopyOnWriteArrayList<(ShortArray, Int) -> Unit>()

        fun processing(options: JSONObject): Boolean {
            val c = options.optJSONObject("audioConstraints") ?: return false
            return c.optBoolean("echoCancellation") || c.optBoolean("noiseSuppression") || c.optBoolean("autoGainControl")
        }

        override fun tap(block: (ShortArray, Int) -> Unit): () -> Unit {
            taps += block
            return { taps -= block }
        }

        fun lease(app: Context, options: JSONObject) {
            if (leases == 0) start(app, options)
            leases += 1
        }

        fun unlease() {
            leases = maxOf(0, leases - 1)
            if (leases == 0) stop()
        }

        private fun start(app: Context, options: JSONObject) {
            val words = options.optJSONObject("android") ?: JSONObject()
            val constraints = options.optJSONObject("audioConstraints") ?: JSONObject()
            val echo = constraints.optBoolean("echoCancellation")
            val source = SOURCES[words.optString("audioSource")]
                ?: if (echo && !AcousticEchoCanceler.isAvailable()) MediaRecorder.AudioSource.VOICE_COMMUNICATION else MediaRecorder.AudioSource.MIC
            val size = maxOf(AudioRecord.getMinBufferSize(RATE, AudioFormat.CHANNEL_IN_MONO, AudioFormat.ENCODING_PCM_16BIT), RATE / 5)
            val builder = AudioRecord.Builder().setAudioSource(source).setBufferSizeInBytes(size * 2)
                .setAudioFormat(AudioFormat.Builder().setEncoding(AudioFormat.ENCODING_PCM_16BIT).setSampleRate(RATE).setChannelMask(AudioFormat.CHANNEL_IN_MONO).build())
            if (Build.VERSION.SDK_INT >= 30 && words.has("privacySensitive")) builder.setPrivacySensitive(words.getBoolean("privacySensitive"))
            hatch?.invoke(builder)
            val r = builder.build()
            if (r.state != AudioRecord.STATE_INITIALIZED) { r.release(); throw IllegalStateException("NotReadableError: the microphone did not open") }
            if (echo && AcousticEchoCanceler.isAvailable()) AcousticEchoCanceler.create(r.audioSessionId)?.setEnabled(true)
            if (constraints.optBoolean("noiseSuppression") && NoiseSuppressor.isAvailable()) NoiseSuppressor.create(r.audioSessionId)?.setEnabled(true)
            if (constraints.optBoolean("autoGainControl") && AutomaticGainControl.isAvailable()) AutomaticGainControl.create(r.audioSessionId)?.setEnabled(true)
            r.startRecording()
            record = r
            if (Build.VERSION.SDK_INT >= 29) {
                val manager = app.getSystemService(Context.AUDIO_SERVICE) as AudioManager
                val cb = object : AudioManager.AudioRecordingCallback() {
                    override fun onRecordingConfigChanged(configs: MutableList<AudioRecordingConfiguration>) {
                        val mine = configs.firstOrNull { it.clientAudioSessionId == r.audioSessionId } ?: return
                        listeners.forEach { it.silenced(mine.isClientSilenced) }
                    }
                }
                manager.registerAudioRecordingCallback(cb, Handler(Looper.getMainLooper()))
                callback = cb
            }
            thread = Thread({
                val buffer = ShortArray(RATE / 20)
                while (record === r) {
                    val n = r.read(buffer, 0, buffer.size)
                    if (n > 0) for (tap in taps) tap(buffer, n)
                }
            }, "exact.media-capture").also { it.start() }
        }

        private fun stop() {
            val r = record ?: return
            record = null
            if (Build.VERSION.SDK_INT >= 29) callback?.let { cb ->
                listeners.firstOrNull()?.let { (it.context.application.getSystemService(Context.AUDIO_SERVICE) as AudioManager).unregisterAudioRecordingCallback(cb) }
            }
            callback = null
            r.stop()
            thread?.join(500)
            thread = null
            r.release()
        }
    }
}
