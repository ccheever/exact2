// A take that survives its process (LLP 1133.033 D5). Under `device.microphone-background` the Task Manager's Stop can
// end the app mid-take with no callback, and `MediaMuxer` writes an MP4's index only at `stop()`. So a take recorded
// where the line is granted is written as a stream readable at every frame: each AAC access unit the encoder makes,
// with the seven-byte ADTS header built from its `csd-0` (ISO/IEC 14496-3 §1.A.2), appended to `<path>.part`. At stop
// the frames go through `MediaMuxer` into `<path>.mp4.tmp`, renamed to `<path>` once the muxer's `stop` succeeds, and
// the part is deleted: `<path>` exists only complete. A part left by a process that died is made an MP4 the same way
// at the module's first call in a later process ([recoverParts]).
package exact.recording

import android.content.Context
import android.media.MediaCodec
import android.media.MediaFormat
import android.media.MediaMuxer
import java.io.File
import java.io.FileOutputStream
import java.nio.ByteBuffer

/** The sampling frequencies an ADTS header indexes. */
private val RATES = intArrayOf(96000, 88200, 64000, 48000, 44100, 32000, 24000, 22050, 16000, 12000, 11025, 8000, 7350)

/** The parts begun and not yet made MP4s, by absolute path: what [recoverParts] finds after a process died. */
private fun ledger(app: Context) = app.getSharedPreferences("exact.recording.parts", Context.MODE_PRIVATE)

/** The ADTS stream of one take ([Encoder]'s writer under the line). On the recorder's thread. */
internal class PartWriter(app: Context, val file: File) {
    val part = File(file.path + ".part")
    private val ledger = ledger(app)
    private val out: FileOutputStream
    /** AudioSpecificConfig's object type, frequency index and channel configuration. */
    private var config: IntArray? = null

    init {
        file.parentFile?.mkdirs()
        part.delete()
        ledger.edit().putBoolean(file.path, true).commit()
        out = FileOutputStream(part)
    }

    /** The encoder's `csd-0`, from its output format. */
    fun configure(format: MediaFormat) {
        val csd = format.getByteBuffer("csd-0") ?: return
        val a = csd.get(0).toInt() and 0xff
        val b = csd.get(1).toInt() and 0xff
        config = intArrayOf(a shr 3, ((a and 7) shl 1) or (b shr 7), (b shr 3) and 0xf)
    }

    /** One access unit, headed and appended; the OS has it once this returns, so a killed process keeps it. */
    fun write(frame: ByteBuffer, size: Int) {
        val (type, rate, channels) = config ?: return
        val length = size + 7
        val header = byteArrayOf(
            0xff.toByte(), 0xf1.toByte(),
            ((((type - 1) and 3) shl 6) or (rate shl 2) or (channels shr 2)).toByte(),
            (((channels and 3) shl 6) or (length shr 11)).toByte(),
            ((length shr 3) and 0xff).toByte(),
            (((length and 7) shl 5) or 0x1f).toByte(),
            0xfc.toByte(),
        )
        val body = ByteArray(size)
        frame.get(body)
        out.write(header + body)
    }

    /** The take ended: its frames made an MP4 at `<path>`, the part deleted. */
    fun finish() {
        out.close()
        mux(part, file)
        ledger.edit().remove(file.path).commit()
    }
}

/** Make [part]'s ADTS frames the MP4 [file], through `<file>.mp4.tmp`; a part with no whole frame makes none. */
internal fun mux(part: File, file: File): Boolean {
    val bytes = part.readBytes()
    val frames = ArrayList<IntArray>()
    var at = 0
    while (at + 7 <= bytes.size && (bytes[at].toInt() and 0xff) == 0xff && (bytes[at + 1].toInt() and 0xf0) == 0xf0) {
        val length = ((bytes[at + 3].toInt() and 3) shl 11) or ((bytes[at + 4].toInt() and 0xff) shl 3) or ((bytes[at + 5].toInt() and 0xff) shr 5)
        if (length < 7 || at + length > bytes.size) break
        frames += intArrayOf(at + 7, length - 7)
        at += length
    }
    if (frames.isEmpty()) {
        part.delete()
        return false
    }
    val b2 = bytes[2].toInt() and 0xff
    val b3 = bytes[3].toInt() and 0xff
    val type = (b2 shr 6) + 1
    val index = (b2 shr 2) and 0xf
    val channels = ((b2 and 1) shl 2) or (b3 shr 6)
    val rate = RATES.getOrElse(index) { 44100 }
    val format = MediaFormat.createAudioFormat(MediaFormat.MIMETYPE_AUDIO_AAC, rate, channels)
    val csd = ByteBuffer.wrap(byteArrayOf(((type shl 3) or (index shr 1)).toByte(), (((index and 1) shl 7) or (channels shl 3)).toByte()))
    format.setByteBuffer("csd-0", csd)
    val tmp = File(file.path + ".mp4.tmp")
    tmp.delete()
    val muxer = MediaMuxer(tmp.path, MediaMuxer.OutputFormat.MUXER_OUTPUT_MPEG_4)
    try {
        val track = muxer.addTrack(format)
        muxer.start()
        val info = MediaCodec.BufferInfo()
        frames.forEachIndexed { i, (start, size) ->
            // 1,024 samples a frame (AAC-LC): the times are rebuilt from the count.
            info.set(0, size, i * 1024L * 1_000_000L / rate, MediaCodec.BUFFER_FLAG_KEY_FRAME)
            muxer.writeSampleData(track, ByteBuffer.wrap(bytes, start, size).slice(), info)
        }
        muxer.stop()
    } finally {
        muxer.release()
    }
    if (!tmp.renameTo(file)) return false
    part.delete()
    return true
}

/** Each part a process that died left, made an MP4 (D5), [log] naming each: `recovered <path>`. */
internal fun recoverParts(app: Context, log: (String) -> Unit) {
    val ledger = ledger(app)
    for (path in ledger.all.keys.toList()) {
        val file = File(path)
        val part = File("$path.part")
        try {
            if (part.isFile && mux(part, file)) log("recording: recovered $path")
        } catch (e: Exception) {
            log("recording: $path did not recover: $e")
        }
        ledger.edit().remove(path).commit()
    }
}
