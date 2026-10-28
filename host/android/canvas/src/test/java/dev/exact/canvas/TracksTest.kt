package dev.exact.canvas

import org.junit.Assert.*
import org.junit.Test

/** The track table's exclusive hold (LLP 1133.012 D8, D12 item 4). */
class TracksTest {
    private class Owner(var running: Boolean) : TrackOwner {
        override fun subscribe(id: String, consumer: TrackConsumer, start: Boolean, origin: String): () -> Unit = {}
        override fun leased(id: String) = running
    }

    @Test fun aHoldIsRefusedWhileACaptureOfItsKindRuns() {
        val tracks = Tracks()
        val owner = Owner(running = true)
        tracks.add(TrackEntry("track-1", "audio", owner = owner))
        val refused = assertThrows(TrackError::class.java) { tracks.hold("audio") }
        assertEquals("NotReadableError", refused.code)
        assertFalse(tracks.held("audio"))
        // A capture of another kind does not stand in the way.
        tracks.hold("video")()
        owner.running = false
        val release = tracks.hold("audio")
        assertTrue(tracks.held("audio"))
        release()
        assertFalse(tracks.held("audio"))
    }

    @Test fun eachHoldIsReleasedOnItsOwn() {
        val tracks = Tracks()
        val first = tracks.hold("audio")
        val second = tracks.hold("audio")
        first()
        assertTrue(tracks.held("audio"))
        first()
        assertTrue(tracks.held("audio"))
        second()
        assertFalse(tracks.held("audio"))
    }
}
