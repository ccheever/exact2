// @exact/recording on Android (LLP 1133.009 D1, D2, D6, D8, §10): a track `@exact/media-capture` opened, reached
// through the session's track table, recorded to a file and metered.
//
// - A recorder is a consumer of its stream's audio track, so it holds the track's lease while it is not `inactive`;
//   its start is a start (D6). A level reader is a consumer too, and never a start.
// - `audio/mp4` (the unsaid default, D2) is AAC from `MediaCodec` in an MP4 from `MediaMuxer`, whose index is
//   written at `stop()`, so a take whose process ended is unreadable (D6: the recipe says so); `audio/wav` is 16-bit
//   PCM, readable up to what was written. Where `device.microphone-background` is granted the process can outlive
//   the screen and be stopped from the Task Manager, so the AAC goes to an ADTS part made an MP4 at stop, or at the
//   next process's first call (Parts.kt, LLP 1133.033 D5). From API 29 `audio/ogg` and `audio/webm` are Opus, which
//   `MediaMuxer` writes page by page, so a take whose process ended is readable up to its last page (D6);
//   `android.outputFormat` and `android.audioEncoder` beat the type's container and codec (D4).
// - Reads arrive on the capture's thread; each recorder copies what it takes and encodes on a thread of its own. A
//   writer two seconds behind stops its recorder with `error` "the writer fell behind" (D6).
// - An interruption, the capture policy's silencing or the background pauses a recording recorder (D8); a track that
//   ends stops it (`track-ended`).
// - `audioLevel` (D1): the RMS of each `interval`, computed on the capture's thread; an answer covers every interval
//   since the last.
// - Under the agent (§10) nothing records: the level is the synthetic signal on the session clock, and a stopped
//   recorder's file is WAV of a tone that follows it, answered as `audio/wav`.
package exact.recording

import android.content.Context
import android.graphics.Canvas
import android.graphics.Color
import android.graphics.Paint
import android.media.MediaCodec
import android.media.MediaCodecInfo
import android.media.MediaFormat
import android.media.MediaMuxer
import android.os.Handler
import android.os.HandlerThread
import android.os.Looper
import android.view.Choreographer
import android.view.View
import dev.exact.canvas.AudioSource
import dev.exact.canvas.ExactExtension
import dev.exact.canvas.ExtensionContext
import dev.exact.canvas.TrackConsumer
import dev.exact.canvas.TrackError
import org.json.JSONArray
import org.json.JSONObject
import java.io.File
import java.io.RandomAccessFile
import java.util.concurrent.atomic.AtomicInteger
import kotlin.math.floor
import kotlin.math.sin
import kotlin.math.sqrt

class RecordingModule(context: ExtensionContext) : ExactExtension(context) {
    companion object {
        /** The synthetic signal at step [i] (one step a 50 ms of the session clock): the recorder's formula (§10). */
        fun synthetic(i: Int): Double {
            val x = i.toDouble()
            return minOf(1.0, 0.12 + 0.75 * kotlin.math.abs(sin(x * 0.37)) * (0.55 + 0.45 * sin(x * 0.05)))
        }
    }

    private val main = Handler(Looper.getMainLooper())
    private val recorders = LinkedHashMap<String, Recorder>()
    private val meters = HashMap<String, Meter>()
    private var next = 0
    /** A take may outlive the screen: the app's lines declare the `capture` service (LLP 1133.033 D5). */
    private val outlives = !context.agent && context.foregroundService.declared("capture", "microphone")

    init {
        // The parts a process that died left are made MP4s before any take of this one begins.
        if (outlives) Thread({ recoverParts(context.application) { line -> main.post { context.log(line) } } }, "exact.recording.recover").start()
    }

    private fun error(code: String) = JSONObject().put("error", code)
    private fun announce() = context.changed("recording.mediaRecorderState")

    override fun later(name: String, args: JSONObject, reply: (JSONObject) -> Unit) {
        val r = recorders[args.optString("recorder")]
        when (name) {
            "startMediaRecorder" -> reply(start(args))
            "pauseMediaRecorder", "resumeMediaRecorder" -> {
                if (r == null) return reply(error("NotFoundError"))
                if (r.state == "inactive") return reply(error("InvalidStateError"))
                if (name == "pauseMediaRecorder") r.pause("requested") else if (r.state == "paused") r.resume()
                reply(r.info())
            }
            "stopMediaRecorder" -> if (r == null) reply(error("NotFoundError")) else r.finish("requested", null, reply)
            // A read of no recorder (`recorder=""`): an app reads before it holds one.
            "mediaRecorderState" -> reply(if (args.optString("recorder").isEmpty()) JSONObject().put("id", "").put("state", "inactive")
                .put("mimeType", "").put("path", "").put("duration", 0) else r?.info() ?: error("NotFoundError"))
            "isTypeSupported" -> {
                val type = args.optString("type")
                reply(JSONObject().put("type", type).put("supported", if (context.agent) type == "audio/wav" else type in TYPES))
            }
            "audioLevel" -> reply(level(args))
            else -> reply(error("NotSupportedError"))
        }
    }

    private fun start(args: JSONObject): JSONObject {
        val entry = context.tracks.lookup(args.optString("stream"))
        if (entry?.kind != "stream") return error("NotFoundError")
        val track = entry.tracks.firstOrNull { context.tracks.lookup(it)?.kind == "audio" } ?: return error("NotSupportedError")
        val type = if (context.agent) "audio/wav" else args.optString("mimeType").ifEmpty { "audio/mp4" }
        if (type !in TYPES) return error("NotSupportedError")
        val form = if (context.agent) Form.WAV else Form.of(type, args.optJSONObject("android")) ?: return error("NotSupportedError")
        val path = pathFor(args.optString("path"), type) ?: return error("NotSupportedError")
        val file = context.file(path) ?: return error("NotSupportedError")
        next += 1
        val r = Recorder("recorder-${context.session}-$next", type, form, path, file, args.optDouble("audioBitsPerSecond", 0.0))
        try {
            r.leave = context.tracks.subscribe(track, r, true, context.origin)
        } catch (e: TrackError) {
            return error(e.code)
        }
        recorders[r.id] = r
        context.log("recording: ${r.id} recording ${args.optString("stream")} to $path")
        announce()
        return r.info()
    }

    /** The path with its container's extension: added when the app wrote none, null when it disagrees. */
    private fun pathFor(path: String, type: String): String? {
        val ext = mapOf("audio/mp4" to ".m4a", "audio/wav" to ".wav", "audio/ogg" to ".ogg", "audio/webm" to ".webm")[type] ?: ""
        val name = path.substringAfterLast('/')
        val dot = name.lastIndexOf('.')
        if (dot <= 0) return path + ext
        return if (name.substring(dot) == ext) path else null
    }

    private fun level(args: JSONObject): JSONObject {
        val track = args.optString("track")
        // A read of no track (`track=""`) is silence: an app reads before it holds one.
        if (track.isEmpty()) return JSONObject().put("audioLevel", 0).put("totalAudioEnergy", 0).put("totalSamplesDuration", 0).put("levels", JSONArray())
        val key = "$track ${args.optDouble("interval", 0.0)} ${args.optDouble("history", 0.0)}"
        val m = meters[key] ?: Meter(args.optDouble("interval", 50.0).takeIf { it > 0 } ?: 50.0,
            args.optDouble("history", 100.0).takeIf { it > 0 }?.toInt() ?: 100, context.now()).also { m ->
            try { m.leave = context.tracks.subscribe(track, m, false, context.origin) } catch (e: TrackError) { return error(e.code) }
            meters[key] = m
        }
        return m.read(context.now())
    }

    override fun release(name: String, args: JSONObject) {
        if (name != "audioLevel") return
        meters.remove("${args.optString("track")} ${args.optDouble("interval", 0.0)} ${args.optDouble("history", 0.0)}")?.stop()
    }

    override fun release(name: String) {
        if (name != "audioLevel") return
        meters.values.forEach { it.stop() }
        meters.clear()
    }

    /** `<audio-waveform>` (D13): the track's levels as bars, newest at the right, drawn each frame from a meter of its
     * own, so no level crosses into Contract; a reader of the track that never starts it. */
    override fun view(tag: String, session: String, id: String, context: Context, props: JSONObject, emit: (String, String) -> Unit): View {
        require(tag == "audio-waveform") { "no view $tag" }
        return Waveform(context).also { update(it, props) }
    }

    override fun update(view: View, props: JSONObject) {
        val w = view as? Waveform ?: return
        props.optString("tint").takeIf { it.isNotEmpty() }?.let { c -> runCatching { Color.parseColor(c) }.getOrNull()?.let { w.paint.color = it } }
        val track = props.optString("track")
        if (track == w.track) return
        w.meter?.stop()
        w.track = track
        w.meter = if (track.isEmpty()) null else Meter(50.0, 1000, this.context.now()).also { m ->
            try { m.leave = this.context.tracks.subscribe(track, m, false, this.context.origin) } catch (e: TrackError) { return }
        }
        w.bars = emptyList()
    }

    override fun destroyView(view: View) { (view as? Waveform)?.let { it.gone = true; it.meter?.stop() } }

    private inner class Waveform(context: Context) : View(context), Choreographer.FrameCallback {
        val paint = Paint().apply { color = Color.rgb(209, 36, 47) }
        var track = ""
        var meter: Meter? = null
        var bars: List<Double> = emptyList()
        var gone = false
        private val density = context.resources.displayMetrics.density

        init { Choreographer.getInstance().postFrameCallback(this) }

        override fun doFrame(frameTimeNanos: Long) {
            if (gone) return
            val fit = ((width + GAP * density) / ((BAR + GAP) * density)).toInt()
            bars = meter?.levelsNow(this@RecordingModule.context.now())?.takeLast(fit) ?: emptyList()
            invalidate()
            Choreographer.getInstance().postFrameCallback(this)
        }

        override fun onDraw(canvas: Canvas) {
            val w = width.toFloat()
            val h = height.toFloat()
            val bar = BAR * density
            val gap = GAP * density
            bars.forEachIndexed { i, level ->
                val x = w - (bars.size - i) * (bar + gap) + gap
                val half = maxOf(density, (level * (h / 2 - 2 * density)).toFloat())
                canvas.drawRect(x, h / 2 - half, x + bar, h / 2 + half, paint)
            }
        }
    }

    override fun dispose(ids: List<String>) { ids.mapNotNull { recorders[it] }.forEach { it.finish("requested", null) {} } }

    override fun destroy(done: () -> Unit) {
        meters.values.forEach { it.stop() }
        val left = recorders.values.filter { it.state != "inactive" }
        if (left.isEmpty()) return done()
        val count = AtomicInteger(left.size)
        left.forEach { it.finish("session-ended", null) { if (count.decrementAndGet() == 0) done() } }
    }

    /** One recorder: a consumer of its track, encoding on a thread of its own. */
    private inner class Recorder(val id: String, val type: String, val form: Form, val path: String, val file: File, val bitRate: Double) : TrackConsumer {
        var state = "recording"
        var reason: String? = null
        var error: String? = null
        var result: JSONObject? = null
        var leave: (() -> Unit)? = null
        private var recorded = 0.0
        private var since: Double? = context.now()
        private val spans = mutableListOf(doubleArrayOf(context.now(), Double.NaN))
        private val thread = HandlerThread("exact.recording.$id").also { it.start() }
        private val writer = Handler(thread.looper)
        private var encoder: Encoder? = null
        private var untap: (() -> Unit)? = null
        private val pending = AtomicInteger(0)
        @Volatile private var taking = true
        private val waiting = mutableListOf<(JSONObject) -> Unit>()

        val duration get() = recorded + if (state == "recording") since?.let { maxOf(0.0, context.now() - it) } ?: 0.0 else 0.0

        fun info(): JSONObject {
            val out = JSONObject().put("id", id).put("state", state).put("mimeType", type).put("path", path).put("duration", duration)
            if (bitRate > 0) out.put("audioBitsPerSecond", bitRate)
            reason?.let { out.put("reason", it) }
            error?.let { out.put("error", it) }
            result?.let { out.put("result", it) }
            return out
        }

        fun pause(why: String) {
            if (state != "recording") return
            recorded = duration; since = null; taking = false
            if (context.agent) spans.last()[1] = context.now()
            state = "paused"; reason = why
            context.log("recording: $id paused ($why)")
            announce()
        }

        fun resume() {
            state = "recording"; reason = null; since = context.now(); taking = true
            if (context.agent) spans += doubleArrayOf(context.now(), Double.NaN)
            announce()
        }

        override fun live(source: AudioSource?) {
            if (source == null || untap != null || state == "inactive") return
            val limit = source.sampleRate * 2
            try {
                encoder = Encoder(file, form, source.sampleRate, bitRate, if (outlives) context.application else null)
            } catch (e: Exception) {
                return finish("error", "the file did not open: $e") {}
            }
            untap = source.tap { samples, n ->
                if (!taking) return@tap
                // Nothing waits here: a writer two seconds behind stops the recorder (D6).
                if (pending.addAndGet(n) > limit) { main.post { finish("error", "the writer fell behind") {} }; return@tap }
                val copy = samples.copyOf(n)
                writer.post { encoder?.write(copy); pending.addAndGet(-n) }
            }
        }

        override fun muted(reason: String) {
            when (reason) {
                "idle" -> {}
                "background" -> pause("background")
                "silenced" -> pause("interrupted")
                else -> pause("interrupted")
            }
        }

        override fun unmuted() {}
        override fun ended() = finish("track-ended", null) {}

        fun finish(why: String, failure: String?, done: (JSONObject) -> Unit) {
            result?.let { return done(it) }
            waiting += done
            if (state == "inactive") return
            recorded = duration; since = null; taking = false
            if (context.agent && spans.last()[1].isNaN()) spans.last()[1] = context.now()
            state = "inactive"; reason = why; error = failure
            untap?.invoke(); untap = null
            announce()
            val duration = recorded
            writer.post {
                if (context.agent) { file.parentFile?.mkdirs(); file.writeBytes(tone(spans)) }
                encoder?.finish()
                main.post {
                    leave?.invoke(); leave = null
                    val out = JSONObject().put("path", path).put("mimeType", type).put("duration", duration).put("size", file.length())
                    result = out
                    context.log("recording: $id stopped ($why), $path, ${file.length()} bytes")
                    announce()
                    waiting.forEach { it(out) }
                    waiting.clear()
                    thread.quitSafely()
                }
            }
        }
    }

    /** A file being written: AAC in MP4, Opus in Ogg or WebM, or 16-bit PCM WAV. On the recorder's thread. With
     * [parts], AAC in MP4 goes to an ADTS part first (Parts.kt). */
    private class Encoder(val file: File, form: Form, capture: Int, bitRate: Double, parts: Context?) {
        /** Opus takes 8, 12, 16, 24 or 48 kHz; the capture's other rates are resampled to 48 kHz. */
        private val resample = form.codec == MediaFormat.MIMETYPE_AUDIO_OPUS && capture !in OPUS_RATES
        val rate = if (resample) 48000 else capture
        private val step = capture.toDouble() / rate
        private var phase = 0.0
        private var previous: Short = 0
        private val wav: RandomAccessFile?
        private val codec: MediaCodec?
        private val muxer: MediaMuxer?
        private val part: PartWriter?
        private var trackIndex = -1
        private var frames = 0L
        private val info = MediaCodec.BufferInfo()

        init {
            file.parentFile?.mkdirs()
            file.delete()
            if (form == Form.WAV) {
                wav = RandomAccessFile(file, "rw").also { it.write(header(0, rate)) }
                codec = null; muxer = null; part = null
            } else {
                wav = null
                val format = MediaFormat.createAudioFormat(form.codec, rate, 1)
                form.profile?.let { format.setInteger(MediaFormat.KEY_AAC_PROFILE, it) }
                format.setInteger(MediaFormat.KEY_BIT_RATE, if (bitRate > 0) bitRate.toInt() else if (form.profile != null) 96000 else 64000)
                codec = MediaCodec.createEncoderByType(form.codec).also {
                    it.configure(format, null, null, MediaCodec.CONFIGURE_FLAG_ENCODE)
                    it.start()
                }
                part = if (form.container == MediaMuxer.OutputFormat.MUXER_OUTPUT_MPEG_4) parts?.let { PartWriter(it, file) } else null
                muxer = if (part == null) MediaMuxer(file.path, form.container) else null
            }
        }

        fun write(captured: ShortArray) {
            val samples = if (resample) resampled(captured) else captured
            wav?.let { f ->
                val bytes = java.nio.ByteBuffer.allocate(samples.size * 2).order(java.nio.ByteOrder.LITTLE_ENDIAN)
                samples.forEach { bytes.putShort(it) }
                f.write(bytes.array())
                return
            }
            val codec = codec ?: return
            var at = 0
            while (at < samples.size) {
                val index = codec.dequeueInputBuffer(10_000)
                if (index < 0) { drain(false); continue }
                val buffer = codec.getInputBuffer(index)!!
                val n = minOf(samples.size - at, buffer.remaining() / 2)
                buffer.order(java.nio.ByteOrder.LITTLE_ENDIAN).asShortBuffer().put(samples, at, n)
                codec.queueInputBuffer(index, 0, n * 2, frames * 1_000_000 / rate, 0)
                frames += n; at += n
                drain(false)
            }
        }

        /** [input] at [rate], by linear interpolation across calls. */
        private fun resampled(input: ShortArray): ShortArray {
            val out = ShortArray(((input.size - phase) / step).toInt().coerceAtLeast(0) + 1)
            var n = 0
            while (phase < input.size && n < out.size) {
                val i = floor(phase).toInt()
                val a = if (i == 0) previous.toDouble() else input[i - 1].toDouble()
                val f = phase - i
                out[n++] = (a + (input[i] - a) * f).toInt().toShort()
                phase += step
            }
            phase -= input.size
            if (input.isNotEmpty()) previous = input.last()
            return out.copyOf(n)
        }

        private fun drain(end: Boolean) {
            val codec = codec ?: return
            while (true) {
                val index = codec.dequeueOutputBuffer(info, if (end) 10_000 else 0)
                when {
                    index == MediaCodec.INFO_OUTPUT_FORMAT_CHANGED -> {
                        part?.configure(codec.outputFormat)
                        muxer?.let { trackIndex = it.addTrack(codec.outputFormat); it.start() }
                    }
                    index >= 0 -> {
                        val buffer = codec.getOutputBuffer(index)!!
                        val frame = info.flags and MediaCodec.BUFFER_FLAG_CODEC_CONFIG == 0 && info.size > 0
                        if (frame && part != null) {
                            buffer.position(info.offset); buffer.limit(info.offset + info.size)
                            part.write(buffer, info.size)
                        } else if (frame && trackIndex >= 0) muxer?.writeSampleData(trackIndex, buffer, info)
                        codec.releaseOutputBuffer(index, false)
                        if (info.flags and MediaCodec.BUFFER_FLAG_END_OF_STREAM != 0) return
                    }
                    else -> if (!end) return
                }
            }
        }

        fun finish() {
            wav?.let { f ->
                val length = f.length() - 44
                f.seek(0); f.write(header(length.toInt(), rate)); f.close()
                return
            }
            val codec = codec ?: return
            val index = codec.dequeueInputBuffer(10_000)
            if (index >= 0) codec.queueInputBuffer(index, 0, 0, frames * 1_000_000 / rate, MediaCodec.BUFFER_FLAG_END_OF_STREAM)
            drain(true)
            codec.stop(); codec.release()
            if (trackIndex >= 0) muxer?.stop()
            muxer?.release()
            part?.finish()
        }
    }

    /** A level on one track with one argument group (D1). */
    private inner class Meter(val interval: Double, val history: Int, var at: Double) : TrackConsumer {
        var leave: (() -> Unit)? = null
        private var untap: (() -> Unit)? = null
        private val levels = ArrayList<Double>()
        private var energy = 0.0
        private var duration = 0.0
        private var last = 0.0
        private var running = false
        private var quiet = false
        private val measured = java.util.concurrent.ConcurrentLinkedQueue<Double>()

        override fun live(source: AudioSource?) {
            running = true
            if (source == null || untap != null) return
            val window = maxOf(1, (source.sampleRate * interval / 1000).toInt())
            var sum = 0.0
            var count = 0
            untap = source.tap { samples, n ->
                for (i in 0 until n) {
                    val s = samples[i] / 32768.0
                    sum += s * s; count += 1
                    if (count >= window) { measured.add(minOf(1.0, sqrt(sum / count))); sum = 0.0; count = 0 }
                }
                while (measured.size > 4096) measured.poll()
            }
        }

        override fun muted(reason: String) { quiet = true }
        override fun unmuted() { quiet = false }
        override fun ended() { running = false; untap?.invoke(); untap = null }
        fun stop() { untap?.invoke(); untap = null; leave?.invoke(); leave = null }

        /** The rolling levels, read up to [now]. */
        fun levelsNow(now: Double): List<Double> { read(now); return levels }

        fun read(now: Double): JSONObject {
            val steps = floor((now - at) / interval).toInt()
            val fresh = ArrayList<Double>()
            if (steps > 0) {
                when {
                    !running || quiet -> repeat(steps) { fresh += 0.0 }
                    context.agent -> for (k in 1..steps) fresh += synthetic(floor((at + k * interval) / 50).toInt())
                    else -> { val all = generateSequence { measured.poll() }.toList(); fresh += all.takeLast(steps) }
                }
                at += steps * interval
            }
            for (level in fresh) { energy += level * level * interval / 1000; duration += interval / 1000; last = level }
            levels += fresh
            while (levels.size > history) levels.removeAt(0)
            return JSONObject().put("audioLevel", last).put("totalAudioEnergy", energy).put("totalSamplesDuration", duration)
                .put("levels", JSONArray(levels))
        }
    }
}

private val TYPES = listOf("audio/mp4", "audio/wav") + if (android.os.Build.VERSION.SDK_INT >= 29) listOf("audio/ogg", "audio/webm") else emptyList()
private val OPUS_RATES = setOf(8000, 12000, 16000, 24000, 48000)

/** A take's container and codec: the type's (D2), or the platform's words (D4). Null for what this API cannot write. */
private data class Form(val container: Int, val codec: String, val profile: Int?) {
    companion object {
        val WAV = Form(-1, "audio/raw", null)
        private val AAC = MediaCodecInfo.CodecProfileLevel.AACObjectLC
        private val HE_AAC = MediaCodecInfo.CodecProfileLevel.AACObjectHE

        fun of(type: String, words: JSONObject?): Form? {
            if (type == "audio/wav") return WAV
            val opus = android.os.Build.VERSION.SDK_INT >= 29
            val container = when (words?.optString("outputFormat")?.ifEmpty { null } ?: mapOf("audio/mp4" to "mpeg4", "audio/ogg" to "ogg", "audio/webm" to "webm")[type]) {
                "mpeg4" -> MediaMuxer.OutputFormat.MUXER_OUTPUT_MPEG_4
                "ogg" -> if (opus) MediaMuxer.OutputFormat.MUXER_OUTPUT_OGG else return null
                "webm" -> MediaMuxer.OutputFormat.MUXER_OUTPUT_WEBM
                else -> return null
            }
            val encoder = words?.optString("audioEncoder")?.ifEmpty { null } ?: if (container == MediaMuxer.OutputFormat.MUXER_OUTPUT_MPEG_4) "aac" else "opus"
            val form = when (encoder) {
                "aac" -> Form(container, MediaFormat.MIMETYPE_AUDIO_AAC, AAC)
                "heAac" -> Form(container, MediaFormat.MIMETYPE_AUDIO_AAC, HE_AAC)
                "opus" -> if (opus) Form(container, MediaFormat.MIMETYPE_AUDIO_OPUS, null) else return null
                else -> return null
            }
            // `MediaMuxer` puts AAC in MP4 and Opus in Ogg and WebM.
            return form.takeIf { (container == MediaMuxer.OutputFormat.MUXER_OUTPUT_MPEG_4) == (it.codec == MediaFormat.MIMETYPE_AUDIO_AAC) }
        }
    }
}

private const val BAR = 3f
private const val GAP = 2f

/** A WAV header for [length] bytes of 16-bit mono PCM at [rate]. */
private fun header(length: Int, rate: Int): ByteArray {
    val b = java.nio.ByteBuffer.allocate(44).order(java.nio.ByteOrder.LITTLE_ENDIAN)
    b.put("RIFF".toByteArray()); b.putInt(36 + length); b.put("WAVE".toByteArray())
    b.put("fmt ".toByteArray()); b.putInt(16); b.putShort(1); b.putShort(1); b.putInt(rate); b.putInt(rate * 2); b.putShort(2); b.putShort(16)
    b.put("data".toByteArray()); b.putInt(length)
    return b.array()
}

/** The agent's take: WAV at 8 kHz of a 440 Hz tone whose amplitude follows the signal over its spans (§10). */
private fun tone(spans: List<DoubleArray>, rate: Int = 8000): ByteArray {
    val samples = ArrayList<Short>()
    for (span in spans) {
        var t = span[0]
        while (t < span[1]) {
            val level = RecordingModule.synthetic(floor(t / 50).toInt())
            samples += (maxOf(-1.0, minOf(1.0, 0.8 * level * sin(2 * Math.PI * 440 * t / 1000))) * 32767).toInt().toShort()
            t += 1000.0 / rate
        }
    }
    val body = java.nio.ByteBuffer.allocate(samples.size * 2).order(java.nio.ByteOrder.LITTLE_ENDIAN)
    samples.forEach { body.putShort(it) }
    return header(samples.size * 2, rate) + body.array()
}
