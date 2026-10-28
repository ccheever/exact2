package dev.exact.canvas

import android.content.Context
import android.media.AudioAttributes
import android.media.AudioFocusRequest
import android.media.AudioManager
import android.os.Handler
import android.os.Looper

/**
 * The process's one audio focus owner (LLP 1133.012 D4, D12 item 2): a module that plays or captures claims focus
 * here rather than requesting it, so the process makes one request, for the strongest type claimed:
 *
 * - `capture`: `AUDIOFOCUS_GAIN` for the capture's length; speech under it requests nothing more.
 * - `playback`: `AUDIOFOCUS_GAIN`; `transient-solo`: `AUDIOFOCUS_GAIN_TRANSIENT`; `transient`:
 *   `AUDIOFOCUS_GAIN_TRANSIENT_MAY_DUCK`.
 *
 * Each uses `USAGE_MEDIA` and `CONTENT_TYPE_SPEECH`, and pauses when ducked, since spoken content "should pause rather
 * than duck" ([Manage audio focus](https://developer.android.com/media/optimize/audio-focus)). One listener serves
 * every request: the platform keys a request by its listener, so a new type replaces the request instead of the
 * process losing focus to itself. Each holder hears a focus change: `focus-transient` for a transient loss (a duck
 * included), `focus` for a lasting one, `focus-gained` when it returns. The last release abandons focus. Main thread.
 */
object AudioFocus {
    private val order = listOf("transient", "transient-solo", "playback", "capture")
    private val claims = ArrayList<AudioClaim>()
    private var request: AudioFocusRequest? = null
    private var requested: String? = null
    internal var application: Context? = null
    private val main = Handler(Looper.getMainLooper())
    private val listener = AudioManager.OnAudioFocusChangeListener { change -> main.post { changed(change) } }

    /** A claim of [type], told each focus change through [events]; null when the platform refused the focus it
     * needed (another app's call, or API 35's rule that only the top app or a foreground service may request it). */
    fun claim(type: String, events: (String) -> Unit): AudioClaim? {
        require(type in order) { "audio focus: no type $type" }
        val c = AudioClaim(type, events)
        claims += c
        if (compose()) return c
        claims -= c
        compose()
        return null
    }

    internal fun release(c: AudioClaim) {
        if (claims.remove(c)) compose()
    }

    /** The request the claims held now need, made if it changed: false when the platform refused it. */
    private fun compose(): Boolean {
        val want = claims.maxByOrNull { order.indexOf(it.type) }?.type
        if (want == requested) return true
        val manager = application?.getSystemService(AudioManager::class.java) ?: return want == null
        if (want == null) {
            request?.let { manager.abandonAudioFocusRequest(it) }
            request = null
            requested = null
            return true
        }
        val gain = when (want) {
            "transient" -> AudioManager.AUDIOFOCUS_GAIN_TRANSIENT_MAY_DUCK
            "transient-solo" -> AudioManager.AUDIOFOCUS_GAIN_TRANSIENT
            else -> AudioManager.AUDIOFOCUS_GAIN
        }
        val attributes = AudioAttributes.Builder().setUsage(AudioAttributes.USAGE_MEDIA).setContentType(AudioAttributes.CONTENT_TYPE_SPEECH).build()
        val made = AudioFocusRequest.Builder(gain).setAudioAttributes(attributes).setWillPauseWhenDucked(true)
            .setOnAudioFocusChangeListener(listener, main).build()
        if (manager.requestAudioFocus(made) != AudioManager.AUDIOFOCUS_REQUEST_GRANTED) return false
        request = made
        requested = want
        return true
    }

    private fun changed(change: Int) {
        val event = when (change) {
            AudioManager.AUDIOFOCUS_LOSS -> "focus"
            AudioManager.AUDIOFOCUS_LOSS_TRANSIENT, AudioManager.AUDIOFOCUS_LOSS_TRANSIENT_CAN_DUCK -> "focus-transient"
            AudioManager.AUDIOFOCUS_GAIN -> "focus-gained"
            else -> return
        }
        // A lasting loss leaves the process off the focus stack: the next claim requests again.
        if (event == "focus") requested = null
        claims.toList().forEach { it.events(event) }
    }
}

/** A module's hold on audio focus ([AudioFocus.claim]); [release] gives it back. */
class AudioClaim internal constructor(internal val type: String, internal val events: (String) -> Unit) {
    fun release() = AudioFocus.release(this)
}
