package dev.exact.heavybench

import android.app.Activity
import android.graphics.Color
import android.graphics.Typeface
import android.os.Bundle
import android.os.SystemClock
import android.view.Gravity
import android.view.View
import android.view.ViewGroup
import android.widget.LinearLayout
import android.widget.TextView
import androidx.recyclerview.widget.LinearLayoutManager
import androidx.recyclerview.widget.RecyclerView

class ViewsActivity : Activity() {
    private lateinit var images: HeavyImageLoader
    private lateinit var harness: HeavyBenchHarness
    private var live: HeavyLiveLoop? = null
    private var seconds = 0
    override fun onCreate(state: Bundle?) {
        val created = SystemClock.elapsedRealtimeNanos()
        super.onCreate(state)
        HeavyWindow.configure(this)
        val store = HeavyStore(HeavyData.load(assets))
        images = HeavyImageLoader(this)
        val root = LinearLayout(this).apply { orientation = LinearLayout.VERTICAL; setBackgroundColor(Color.WHITE) }
        val top = LinearLayout(this).apply { orientation = LinearLayout.HORIZONTAL; gravity = Gravity.CENTER_VERTICAL; setPadding(dp(16f), dp(10f), dp(16f), dp(10f)) }
        top.addView(TextView(this).apply { text = "Heavy list"; textSize = 17f; includeFontPadding = false; setTextColor(Color.BLACK); typeface = Typeface.create(Typeface.DEFAULT, 600, false) },
            LinearLayout.LayoutParams(0, ViewGroup.LayoutParams.WRAP_CONTENT, 1f))
        val enabled = HeavyOptions.boolean(intent, "BENCH_LIVE")
        val liveLabel = TextView(this).apply { text = "Live: off"; textSize = 13f; includeFontPadding = false; setTextColor(HeavyColors.secondary) }
        top.addView(liveLabel)
        root.addView(top)
        root.addView(View(this).apply { setBackgroundColor(HeavyColors.hairline) }, LinearLayout.LayoutParams(ViewGroup.LayoutParams.MATCH_PARENT, dp(.5f).coerceAtLeast(1)))
        val recycler = RecyclerView(this)
        val manager = LinearLayoutManager(this)
        recycler.layoutManager = manager
        recycler.itemAnimator = null
        val adapter = object : RecyclerView.Adapter<RowHolder>() {
            init { setHasStableIds(true) }
            override fun getItemCount(): Int = store.size
            override fun getItemId(position: Int): Long = store[position].stableId
            override fun onCreateViewHolder(parent: ViewGroup, viewType: Int): RowHolder = RowHolder(MessageRowView(this@ViewsActivity, store, images).apply {
                layoutParams = RecyclerView.LayoutParams(ViewGroup.LayoutParams.MATCH_PARENT, ViewGroup.LayoutParams.WRAP_CONTENT)
            })
            override fun onBindViewHolder(holder: RowHolder, position: Int) { holder.row.bind(store[position], seconds) }
        }
        recycler.adapter = adapter
        root.addView(recycler, LinearLayout.LayoutParams(ViewGroup.LayoutParams.MATCH_PARENT, 0, 1f))
        var consumed = 0
        recycler.addOnScrollListener(object : RecyclerView.OnScrollListener() {
            override fun onScrolled(view: RecyclerView, dx: Int, dy: Int) { consumed += dy }
        })
        val target = object : ScrollTarget {
            private var remainder = 0f
            override fun scrollByPx(delta: Float): Float {
                remainder += delta; val dy = remainder.toInt(); remainder -= dy
                consumed = 0; recycler.scrollBy(0, dy); return consumed.toFloat()
            }
            override fun visibleRows(): Int = (0 until recycler.childCount).count {
                val child = recycler.getChildAt(it); child.bottom > recycler.paddingTop && child.top < recycler.height - recycler.paddingBottom
            }
            override fun mountedRows(): Int = recycler.childCount
            override fun firstVisibleId(): String? = manager.findFirstVisibleItemPosition().takeIf { it in 0 until store.size }?.let { store[it].id }
        }
        live = if (enabled) HeavyLiveLoop(store, { k ->
            val position = manager.findFirstVisibleItemPosition()
            val offset = manager.findViewByPosition(position)?.top?.minus(recycler.paddingTop) ?: 0
            store.insert(k); adapter.notifyItemInserted(0)
            if (position >= 0) manager.scrollToPositionWithOffset(position + 1, offset)
        }, { value ->
            seconds = value
            for (i in 0 until recycler.childCount) (recycler.getChildAt(i) as? MessageRowView)?.updateTime(seconds)
        }, { liveLabel.text = "Live: on" }) else null
        HeavyWindow.install(this, root)
        manager.scrollToPositionWithOffset(HeavyOptions.int(intent, "BENCH_START_INDEX", 0).coerceIn(0, store.size - 1), 0)
        harness = HeavyBenchHarness(this, "views", created)
        harness.attach(root, target, { recycler.childCount > 0 && target.visibleRows() > 0 }, {
            store.metadata().put("images", images.diagnosticSnapshot()).put("list", "RecyclerView/LinearLayoutManager").put("font_padding", false)
        })
        live?.start()
    }
    override fun onDestroy() { live?.close(); if (::harness.isInitialized) harness.close(); if (::images.isInitialized) images.close(); super.onDestroy() }
    private class RowHolder(val row: MessageRowView) : RecyclerView.ViewHolder(row)
}
