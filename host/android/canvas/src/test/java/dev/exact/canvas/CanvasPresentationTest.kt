package dev.exact.canvas

import org.junit.Assert.*
import org.junit.Test

class CanvasPresentationTest {
    @Test fun deadlineStartsAtCompleteSubmissionAndSuccessWins() {
        val p = CanvasPresentation()
        p.attach()
        assertEquals(0, p.observe(0, 10000))
        assertFalse(p.failed)
        assertEquals(0, p.observe(2, 10000))
        assertEquals(0, p.observe(2, 11999))
        assertEquals(1, p.observe(1, 12000))
        assertTrue(p.ready)
        assertFalse(p.failed)
    }

    @Test fun aTimeoutCanRetryAfterBuffersOrANewAttachment() {
        val p = CanvasPresentation()
        p.attach()
        p.observe(2, 100)
        assertEquals(-1, p.observe(0, 2100))
        assertFalse(p.canAttach(true))
        p.clear()
        p.bufferFrame()
        assertTrue(p.canAttach(true))
        assertFalse(p.canAttach(false))
        p.observe(-1, 2200)
        p.attach()
        assertTrue(p.canAttach(true))
        assertFalse(p.ready)
    }

    @Test fun textureFirstFrameReleasesTheSavedImage() {
        val p = CanvasPresentation()
        p.attach()
        assertFalse(p.ready)
        p.firstFrame()
        assertTrue(p.ready)
        p.attach()
        assertFalse(p.ready)
        p.firstFrame()
        assertTrue(p.ready)
    }
}
