package com.exact.benchmark

import android.view.View

/** A real scene, with an acknowledgement from the affected Compose/View phase. */
interface BenchmarkTarget {
    val view: View
    val revision: Long
    val appliedRevision: Long
    val retainedRows: Int
    val startupReady: Boolean get() = true
    val scrollPositionPx: Int get() = 0
    val renderedScrollPositionPx: Int get() = scrollPositionPx
    val scrollRangePx: Int get() = 0
    val scrollViewportPx: Int get() = 0
    fun prepare(workload: String)
    fun performAction(workload: String)
    fun scrollTo(offsetPx: Int) { error("continuous scroll is unsupported by this target") }
}
