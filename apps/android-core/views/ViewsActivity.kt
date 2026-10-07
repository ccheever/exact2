package com.exact.views

import android.app.Activity
import android.content.res.Configuration
import android.os.Build
import android.os.Bundle
import android.os.SystemClock
import android.view.View
import android.view.ViewTreeObserver
import android.view.WindowInsetsController
import android.view.WindowManager
import com.exact.benchmark.BenchmarkTarget
import com.exact.benchmark.ComparisonBenchmark
import com.exact.benchmark.StartupProbe

/** Independent platform baseline: no Compose runtime, Exact code or JNI bridge. */
class ViewsActivity : Activity() {
    private lateinit var scene: ViewsScene
    private var benchmark: ComparisonBenchmark? = null
    private var startup: StartupProbe? = null

    @Suppress("DEPRECATION")
    override fun onCreate(savedInstanceState: Bundle?) {
        val onCreateElapsedNs = SystemClock.elapsedRealtimeNanos()
        super.onCreate(savedInstanceState)
        if (Build.VERSION.SDK_INT >= 30) window.setDecorFitsSystemWindows(false)
        else window.decorView.systemUiVisibility = View.SYSTEM_UI_FLAG_LAYOUT_STABLE or
            View.SYSTEM_UI_FLAG_LAYOUT_FULLSCREEN or View.SYSTEM_UI_FLAG_LAYOUT_HIDE_NAVIGATION
        val light = resources.configuration.uiMode and Configuration.UI_MODE_NIGHT_MASK != Configuration.UI_MODE_NIGHT_YES
        val legacyAppearance = View.SYSTEM_UI_FLAG_LIGHT_STATUS_BAR or View.SYSTEM_UI_FLAG_LIGHT_NAVIGATION_BAR
        window.decorView.systemUiVisibility = if (light) window.decorView.systemUiVisibility or legacyAppearance
            else window.decorView.systemUiVisibility and legacyAppearance.inv()
        if (Build.VERSION.SDK_INT >= 30) {
            val mask = WindowInsetsController.APPEARANCE_LIGHT_STATUS_BARS or WindowInsetsController.APPEARANCE_LIGHT_NAVIGATION_BARS
            window.insetsController?.setSystemBarsAppearance(if (light) mask else 0, mask)
        }
        window.attributes = window.attributes.apply {
            layoutInDisplayCutoutMode = WindowManager.LayoutParams.LAYOUT_IN_DISPLAY_CUTOUT_MODE_SHORT_EDGES
        }
        window.setSoftInputMode(WindowManager.LayoutParams.SOFT_INPUT_ADJUST_NOTHING)
        val measuringStartup = intent.getBooleanExtra("exact_startup", false)
        val heavyList = intent.getBooleanExtra("exact_heavy_list", false)
        val initialRows = if (heavyList) 1000 else if (measuringStartup) intent.getIntExtra("exact_startup_rows", 100) else 100
        scene = ViewsScene(this, initialRows, heavyList)
        val target = object : BenchmarkTarget {
            override val view: View get() = scene
            override val revision: Long get() = scene.revision
            override val appliedRevision: Long get() = scene.appliedRevision
            override val retainedRows: Int get() = scene.retainedRows
            override val startupReady: Boolean get() = scene.startupReady
            override val scrollPositionPx: Int get() = scene.scrollPositionPx
            override val renderedScrollPositionPx: Int get() = scene.renderedScrollPositionPx
            override val scrollViewportPx: Int get() = scene.scrollViewportPx
            override val scrollRangePx: Int get() = scene.scrollRangePx
            override fun scrollTo(offsetPx: Int) = scene.scrollTo(offsetPx)
            override fun prepare(workload: String) { scene.prepare(workload) }
            override fun performAction(workload: String) = scene.performAction(workload)
        }
        setContentView(scene)
        if (intent.getBooleanExtra("exact_compare", false)) {
            benchmark = ComparisonBenchmark(this, target, "views", intent.getIntExtra("exact_samples", 120).coerceIn(20, 1000),
                scrollSamples = intent.getIntExtra("exact_scroll_samples", 0).coerceIn(0, 6000),
                scrollOnly = intent.getBooleanExtra("exact_scroll_only", false) || heavyList,
                heavyList = heavyList, scrollSpeedDpPerSecond = intent.getFloatExtra("exact_scroll_speed", 0f))
            afterFirstDraw { benchmark?.start() }
        }
        if (measuringStartup) {
            startup = StartupProbe(this, target, "views", initialRows,
                intent.getStringExtra("exact_startup_token") ?: "", onCreateElapsedNs)
            startup?.start()
        }
    }
    private fun afterFirstDraw(callback: () -> Unit) {
        var posted = false
        val listener = object : ViewTreeObserver.OnDrawListener {
            override fun onDraw() {
                if (posted) return
                posted = true
                scene.post { scene.viewTreeObserver.removeOnDrawListener(this); callback() }
            }
        }
        scene.viewTreeObserver.addOnDrawListener(listener)
    }
    override fun onDestroy() { benchmark?.close(); startup?.close(); super.onDestroy() }
}
