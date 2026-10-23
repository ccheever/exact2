package dev.exact.canvas

/**
 * The session's track table (LLP 1133.009 D3, D10 item 6): tracks and streams by id, each track with the
 * implementation that opened it ([TrackOwner]), through which a consuming implementation subscribes, so a recorder
 * records a track another package opened. Main thread.
 */
class Tracks internal constructor() {
    private val entries = HashMap<String, TrackEntry>()
    fun add(entry: TrackEntry) { entries[entry.id] = entry }
    fun lookup(id: String): TrackEntry? = entries[id]
    fun remove(id: String) { entries.remove(id) }

    /** [consumer] joins track [id] through its owner; the returned function leaves. */
    fun subscribe(id: String, consumer: TrackConsumer, start: Boolean, origin: String = "app"): () -> Unit {
        val owner = entries[id]?.owner ?: throw TrackError("NotFoundError")
        return owner.subscribe(id, consumer, start, origin)
    }
}

/** A track or a stream; a stream lists its tracks. */
class TrackEntry(val id: String, val kind: String, val tracks: List<String> = emptyList(), val owner: TrackOwner? = null)

/** Why a track call failed: a `DOMException` name, as answered. */
class TrackError(val code: String) : Exception(code)

/** The implementation that opened a track: it decides whether a consumer joining takes the track's lease (D6). */
interface TrackOwner {
    /** [start] is an explicit start, which reopens an idle track; [origin] is `app`, `tool` or `wake`. */
    fun subscribe(id: String, consumer: TrackConsumer, start: Boolean, origin: String): () -> Unit
}

/** A consumer of a track: a recorder, a level. Main thread. */
interface TrackConsumer {
    /** The capture runs: its source, or null under the agent (no device). */
    fun live(source: AudioSource?)
    /** `idle`, `interrupted`, `background`, `silenced`, … */
    fun muted(reason: String)
    fun unmuted()
    fun ended()
}

/** A running audio capture, which a consumer taps. */
interface AudioSource {
    val sampleRate: Int
    val channels: Int
    /** [block] hears each read of 16-bit PCM frames (interleaved, its length in samples) on the capture's thread until
     * the returned function is called. It must not block. */
    fun tap(block: (ShortArray, Int) -> Unit): () -> Unit
}
