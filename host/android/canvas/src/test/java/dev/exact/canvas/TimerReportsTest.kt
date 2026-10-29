package dev.exact.canvas

import org.junit.Assert.*
import org.junit.Test

class TimerReportsTest {
    @Test fun publicationsAndReadersUseIndependentMonotonicAcknowledgements() {
        val reports = TimerReports()
        val first = Any(); val replacement = Any()
        reports.replace(first)
        assertTrue(reports.applying)
        val a = reports.publish(); val b = reports.publish()
        assertTrue(reports.report(first, true, true, a))
        assertTrue(reports.scrolling); assertTrue(reports.sampling); assertTrue(reports.applying)
        reports.report(first, false, false, b)
        assertFalse(reports.applying)
        reports.report(first, false, false, a)
        assertFalse(reports.applying)
        reports.replace(replacement)
        assertTrue(reports.applying)
        assertFalse(reports.report(first, false, false, b))
        assertFalse(reports.known)
        val c = reports.publish()
        reports.report(replacement, false, false, b)
        assertTrue(reports.applying)
        reports.report(replacement, false, false, c)
        assertFalse(reports.applying)
        reports.mainWork++
        assertTrue(reports.applying)
        reports.mainWork--
        assertFalse(reports.applying)
    }
}
