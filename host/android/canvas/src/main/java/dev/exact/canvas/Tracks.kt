package dev.exact.canvas

import org.json.JSONObject

/**
 * The session's track table (LLP 1133.009 D3, D10 item 6): tracks and streams by id, each track with the
 * implementation that opened it ([TrackOwner]), through which a consuming implementation subscribes, so a recorder
 * records a track another package opened. Main thread.
 */
class Tracks internal constructor() {
    private val entries = HashMap<String, TrackEntry>()
    private class Hold(val kind: String)
    private val holds = HashSet<Hold>()

    /**
     * An exclusive hold on the [kind] device, for a recognizer that opens it itself (LLP 1133.012 D8, D12 item 4):
     * granted only while no track of [kind] has a running capture ([TrackOwner.leased]), and while it lasts an owner
     * opens no capture of that kind ([held]). The returned function releases it; a running capture throws
     * `NotReadableError`.
     */
    fun hold(kind: String): () -> Unit {
        if (entries.values.any { it.kind == kind && it.owner?.leased(it.id) == true }) throw TrackError("NotReadableError")
        val h = Hold(kind)
        holds += h
        return { holds -= h }
    }
    fun held(kind: String): Boolean = holds.any { it.kind == kind }

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
class TrackEntry(val id: String, val kind: String, val tracks: List<String> = emptyList(), val owner: TrackOwner? = null) {
    /** A track's state as `mediaStreamTrack` answers it, for a package that consumes it. */
    var state: (() -> JSONObject)? = null

    /** A video track's running capture while its lease is held (LLP 1133.008 D3, D8 item 3); null otherwise. */
    var video: (() -> VideoSource?)? = null
}

/** Why a track call failed: a `DOMException` name, as answered. */
class TrackError(val code: String) : Exception(code)

/** The implementation that opened a track: it decides whether a consumer joining takes the track's lease (D6). */
interface TrackOwner {
    /** [start] is an explicit start, which reopens an idle track; [origin] is `app`, `tool` or `wake`. */
    fun subscribe(id: String, consumer: TrackConsumer, start: Boolean, origin: String): () -> Unit
    /** Whether track [id]'s capture runs: [Tracks.hold] is refused while one does. */
    fun leased(id: String): Boolean = false
}

/** A consumer of a track: a recorder, a level, a viewfinder. Main thread. */
interface TrackConsumer {
    /** An audio track's capture runs: its source, or null under the agent (no device). */
    fun live(source: AudioSource?) {}
    /** A video track's capture runs: its source, or null under the agent. */
    fun liveVideo(source: VideoSource?) {}
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

/**
 * A running camera capture (LLP 1133.008 D3, D8 item 3): CameraX's one lifecycle's set of use cases, which a consumer
 * adds to rather than reading frames. Its objects are CameraX's (`UseCase`, `Camera`), typed `Any` here so the reader
 * links no CameraX; a package that uses them names CameraX's coordinates (D8 item 5). Main thread.
 */
interface VideoSource {
    /** The bound `androidx.camera.core.Camera`, once the set is bound. */
    val camera: Any?
    /** Add an `androidx.camera.core.UseCase` to the set, which is bound again with it. */
    fun add(useCase: Any)
    fun remove(useCase: Any)
    /** A package's process object for this capture, made by [make] on first ask (the camera's `ImageCapture`). */
    fun <T : Any> shared(key: String, make: () -> T): T
    /** Constraints that keep the camera (`zoom`, `torch`), applied and reported by the tracks on it. */
    fun apply(constraints: JSONObject)
}
