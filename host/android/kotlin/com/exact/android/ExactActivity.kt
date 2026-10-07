package com.exact.android

import android.app.Activity
import android.content.res.Configuration
import android.os.Bundle
import android.os.Build
import android.os.SystemClock
import android.view.View
import android.view.WindowManager
import android.view.WindowInsetsController
import android.widget.ScrollView
import com.exact.benchmark.BenchmarkTarget
import com.exact.benchmark.ComparisonBenchmark
import com.exact.benchmark.StartupProbe

/** The app adapter over ExactView, equivalent to the Apple executable adapters. */
class ExactActivity : Activity() {
    private lateinit var exact: ExactView
    private var benchmark: AndroidBenchmark? = null
    private var comparison: ComparisonBenchmark? = null
    private var startup: StartupProbe? = null
    @Suppress("DEPRECATION")
    override fun onCreate(savedInstanceState: Bundle?) {
        val onCreateElapsedNs = SystemClock.elapsedRealtimeNanos()
        super.onCreate(savedInstanceState)
        if (Build.VERSION.SDK_INT >= 30) window.setDecorFitsSystemWindows(false)
        else window.decorView.systemUiVisibility = View.SYSTEM_UI_FLAG_LAYOUT_STABLE or View.SYSTEM_UI_FLAG_LAYOUT_FULLSCREEN or View.SYSTEM_UI_FLAG_LAYOUT_HIDE_NAVIGATION
        val light = resources.configuration.uiMode and Configuration.UI_MODE_NIGHT_MASK != Configuration.UI_MODE_NIGHT_YES
        val legacyAppearance = View.SYSTEM_UI_FLAG_LIGHT_STATUS_BAR or View.SYSTEM_UI_FLAG_LIGHT_NAVIGATION_BAR
        window.decorView.systemUiVisibility = if (light) window.decorView.systemUiVisibility or legacyAppearance
            else window.decorView.systemUiVisibility and legacyAppearance.inv()
        if (Build.VERSION.SDK_INT >= 30) {
            val mask = WindowInsetsController.APPEARANCE_LIGHT_STATUS_BARS or WindowInsetsController.APPEARANCE_LIGHT_NAVIGATION_BARS
            window.insetsController?.setSystemBarsAppearance(if (light) mask else 0, mask)
        }
        window.attributes = window.attributes.apply { layoutInDisplayCutoutMode = WindowManager.LayoutParams.LAYOUT_IN_DISPLAY_CUTOUT_MODE_SHORT_EDGES }
        window.setSoftInputMode(WindowManager.LayoutParams.SOFT_INPUT_ADJUST_NOTHING)
        val measuringStartup = intent.getBooleanExtra("exact_startup", false)
        val heavyList = intent.getBooleanExtra("exact_heavy_list", false)
        val initialRows = if (heavyList) 1000 else if (measuringStartup) intent.getIntExtra("exact_startup_rows", 100) else 100
        require(initialRows == 1 || initialRows == 100 || initialRows == 1000)
        exact = ExactView(this, if (heavyList) "heavy-list" else if (initialRows == 100) null else "rows-$initialRows")
        exact.onTitle = { title = it }
        val target = object : BenchmarkTarget {
            private var actionView: View? = null
            private var scroll: ScrollView? = null
            private val viewportWindowPosition = IntArray(2)
            private val contentWindowPosition = IntArray(2)
            private var highlighted = false
            private var moved = false
            private var heavySelected = heavyList
            private fun scroller(): ScrollView = scroll ?: benchmarkScrollView(exact).also { scroll = it }
            override val view: View get() = exact
            override var revision = 0L
                private set
            override val appliedRevision: Long get() = revision
            override val startupReady: Boolean get() = exact.startupReady
            override var retainedRows = initialRows
                private set
            override val scrollPositionPx: Int get() = scroller().scrollY
            override val renderedScrollPositionPx: Int get() {
                val viewport = scroller()
                viewport.getLocationInWindow(viewportWindowPosition)
                viewport.getChildAt(0).getLocationInWindow(contentWindowPosition)
                return viewportWindowPosition[1] + viewport.paddingTop - contentWindowPosition[1]
            }
            override val scrollViewportPx: Int get() = scroller().let { it.height - it.paddingTop - it.paddingBottom }
            override val scrollRangePx: Int get() = (scroller().getChildAt(0).height - scrollViewportPx).coerceAtLeast(0)
            override fun scrollTo(offsetPx: Int) { scroller().scrollTo(0, offsetPx) }
            override fun prepare(workload: String) {
                val rows = when (workload) {
                    "counter", "paint-1" -> 1
                    "paint-100" -> 100
                    "paint-1000", "transform-1000", "scroll-1000", "heavy-scroll-1000" -> 1000
                    else -> error("Unknown workload '$workload'")
                }
                if (workload == "heavy-scroll-1000") {
                    if (!heavySelected) { check(exact.activate("heavy-list")); heavySelected = true; scroll = null }
                } else check(exact.activate("rows-$rows"))
                if (workload == "scroll-1000" || workload == "heavy-scroll-1000") {
                    if (highlighted) { check(exact.activate("toggle-batch")); highlighted = false }
                    if (moved) { check(exact.activate("toggle-move")); moved = false }
                    scroller().scrollTo(0, 0)
                }
                actionView = if (workload == "scroll-1000" || workload == "heavy-scroll-1000") null else checkNotNull(exact.actionView(when (workload) {
                    "counter" -> "increment"
                    "transform-1000" -> "toggle-move"
                    else -> "toggle-batch"
                }))
                retainedRows = rows
                revision++
            }
            override fun performAction(workload: String) {
                // References invoke their action directly; omit platform click feedback here.
                check(checkNotNull(actionView).callOnClick())
                when (workload) {
                    "paint-1", "paint-100", "paint-1000" -> highlighted = !highlighted
                    "transform-1000" -> moved = !moved
                }
                revision++
            }
        }
        if (measuringStartup) {
            startup = StartupProbe(this, target, "exact", initialRows,
                intent.getStringExtra("exact_startup_token") ?: "", onCreateElapsedNs)
            startup?.start()
        }
        if (intent.getBooleanExtra("exact_compare", false)) {
            comparison = ComparisonBenchmark(this, target, "exact", intent.getIntExtra("exact_samples", 120).coerceIn(20, 1000),
                scrollSamples = intent.getIntExtra("exact_scroll_samples", 0).coerceIn(0, 6000),
                scrollOnly = intent.getBooleanExtra("exact_scroll_only", false) || heavyList,
                heavyList = heavyList, scrollSpeedDpPerSecond = intent.getFloatExtra("exact_scroll_speed", 0f))
            exact.onFirstDraw = { comparison?.start() }
        }
        if (intent.getBooleanExtra("exact_benchmark", false)) {
            benchmark = AndroidBenchmark(this, exact, intent.getIntExtra("exact_samples", 120).coerceIn(20, 1000))
            exact.onFirstDraw = { benchmark?.start() }
        }
        setContentView(exact)
    }
    @Suppress("DEPRECATION")
    override fun onBackPressed() {
        if (!exact.navigateBack()) super.onBackPressed()
    }
    override fun onStart() { super.onStart(); exact.setSessionVisible(true) }
    override fun onStop() { exact.setSessionVisible(false); super.onStop() }
    override fun onDestroy() { benchmark?.close(); comparison?.close(); startup?.close(); exact.close(); super.onDestroy() }
}
