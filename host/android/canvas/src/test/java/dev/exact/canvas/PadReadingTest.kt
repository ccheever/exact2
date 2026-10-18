package dev.exact.canvas

import org.junit.Assert.*
import org.junit.Test

class PadReadingTest {
    /** A pad without trigger axes: its L2 key holds through motion that reports no triggers. */
    @Test fun aDigitalTriggerHoldsThroughMotion() {
        val r = PadReading(analogTriggers = false)
        assertTrue(r.key(6, true))
        assertTrue(r.motion(0.5f, 0f, 0f, 0f, 0f, 0f, 0f, 0f))
        assertEquals(1f, r.values[6])
        assertEquals(0.5f, r.values[17])
        assertTrue(r.key(6, false))
        assertEquals(0f, r.values[6])
    }

    /** A pad with trigger axes: the axis is the trigger; its key edge changes nothing. */
    @Test fun anAnalogTriggerFollowsItsAxis() {
        val r = PadReading(analogTriggers = true)
        assertFalse(r.key(7, true))
        assertTrue(r.motion(0f, 0f, 0f, 0f, 0f, 0.3f, 0f, 0f))
        assertEquals(77f / 256f, r.values[7])
        assertTrue(r.motion(0f, 0f, 0f, 0f, 0f, 0f, 0f, 0f))
        assertEquals(0f, r.values[7])
    }

    /** The d-pad is keys until a hat reports; then the hat owns it. */
    @Test fun aHatOwnsTheDpadOnceItReports() {
        val r = PadReading(analogTriggers = false)
        r.key(12, true)
        assertFalse(r.motion(0f, 0f, 0f, 0f, 0f, 0f, 0f, 0f))
        assertEquals(1f, r.values[12])
        assertTrue(r.motion(0f, 0f, 0f, 0f, 0f, 0f, 1f, 0f))
        assertEquals(listOf(0f, 0f, 0f, 1f), (12..15).map { r.values[it] })
    }
}
