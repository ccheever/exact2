package com.exact.compose

import android.content.res.Configuration
import android.os.Build
import android.os.Bundle
import android.os.SystemClock
import android.view.View
import android.view.ViewTreeObserver
import android.view.WindowInsetsController
import android.view.WindowManager
import androidx.activity.ComponentActivity
import androidx.compose.ui.platform.ComposeView
import com.exact.benchmark.BenchmarkTarget
import com.exact.benchmark.ComparisonBenchmark
import com.exact.benchmark.StartupProbe

/** A Compose-only app: no Exact presenter, plan, native library or JNI bridge. */
class ComposeActivity : ComponentActivity() {
    private val scene = ComposeScene()
    private lateinit var compose: ComposeView
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
        val heavyList = intent.getBooleanExtra("exact_heavy_list", false)
        val startupRows = if (heavyList) 1000 else intent.getIntExtra("exact_startup_rows", 100).coerceIn(1, 1000)
        if (heavyList) scene.heavyList() else if (intent.getBooleanExtra("exact_startup", false)) scene.setRows(startupRows)
        compose = ComposeView(this).apply { setContent { CoreScene(scene) } }
        val target = object : BenchmarkTarget {
            override val view: View get() = compose
            override val revision: Long get() = scene.revision
            override val appliedRevision: Long get() = scene.appliedRevision
            override val retainedRows: Int get() = scene.rowCount
            override val scrollPositionPx: Int get() = scene.scrollPositionPx
            override val renderedScrollPositionPx: Int get() = scene.renderedScrollPositionPx
            override val scrollViewportPx: Int get() = scene.scrollViewportPx
            override val scrollRangePx: Int get() = scene.scrollRangePx
            override fun scrollTo(offsetPx: Int) = scene.scrollTo(offsetPx)
            override fun prepare(workload: String) { scene.prepare(workload); compose.invalidate() }
            override fun performAction(workload: String) = scene.performAction(workload)
        }
        setContentView(compose)
        if (intent.getBooleanExtra("exact_compare", false)) {
            benchmark = ComparisonBenchmark(this, target, "compose", intent.getIntExtra("exact_samples", 120).coerceIn(20, 1000),
                scrollSamples = intent.getIntExtra("exact_scroll_samples", 0).coerceIn(0, 6000),
                scrollOnly = intent.getBooleanExtra("exact_scroll_only", false) || heavyList,
                heavyList = heavyList, scrollSpeedDpPerSecond = intent.getFloatExtra("exact_scroll_speed", 0f))
            afterFirstDraw { benchmark?.start() }
        }
        if (intent.getBooleanExtra("exact_startup", false)) {
            startup = StartupProbe(this, target, "compose", startupRows,
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
                compose.post {
                    compose.viewTreeObserver.removeOnDrawListener(this)
                    callback()
                }
            }
        }
        compose.viewTreeObserver.addOnDrawListener(listener)
    }

    override fun onDestroy() { benchmark?.close(); startup?.close(); super.onDestroy() }
}
