package dev.exact.heavybench

import android.content.res.AssetManager
import android.graphics.Color
import android.os.Handler
import android.os.Looper
import android.os.SystemClock
import org.json.JSONArray
import org.json.JSONObject

data class TextRun(val text: String, val style: String?)
data class HeavyPhoto(val src: String, val width: Int, val height: Int)
data class HeavyLink(val thumb: String, val title: String, val description: String, val site: String)
data class HeavyQuote(val id: String, val author: String, val excerpt: String)
data class HeavyReaction(val emoji: String, val count: Int)
data class HeavyMessage(
    val id: String, val index: Int, val author: String, val avatar: String, val minutesAgo: Int,
    val paragraphs: List<List<TextRun>>, val photos: List<HeavyPhoto>, val link: HeavyLink?,
    val quote: HeavyQuote?, val reactions: List<HeavyReaction>,
) {
    val stableId: Long get() = index.toLong()
    fun time(seconds: Int): String {
        val minutes = minutesAgo + seconds.coerceAtLeast(0) / 60
        return when { minutes < 60 -> "${minutes}m ago"; minutes < 1440 -> "${minutes / 60}h ago"; else -> "${minutes / 1440}d ago" }
    }
}
data class HeavyDataset(val messages: List<HeavyMessage>, val jsonBytes: Int, val imageFiles: Int)

object HeavyData {
    fun load(assets: AssetManager): HeavyDataset {
        val bytes = assets.open("messages.json").use { it.readBytes() }
        val file = JSONObject(bytes.toString(Charsets.UTF_8))
        require(file.getInt("version") == 1)
        val source = file.getJSONArray("messages")
        require(source.length() == 10000) { "The canonical heavy-list fixture has 10000 rows" }
        val messages = ArrayList<HeavyMessage>(source.length())
        for (i in 0 until source.length()) {
            val m = source.getJSONObject(i)
            val paragraphs = m.getJSONArray("paragraphs").objects { p ->
                val runs = p as JSONArray
                runs.objects { r -> val run = r as JSONObject; TextRun(run.getString("t"), run.optString("s").ifEmpty { null }) }
            }
            val photos = m.optJSONArray("photos")?.objects { p ->
                val photo = p as JSONObject
                HeavyPhoto(photo.getString("src"), photo.getInt("w"), photo.getInt("h"))
            } ?: emptyList()
            val l = m.optJSONObject("link")
            val q = m.optJSONObject("quote")
            val reactions = m.optJSONArray("reactions")?.objects { r ->
                val reaction = r as JSONObject; HeavyReaction(reaction.getString("emoji"), reaction.getInt("count"))
            } ?: emptyList()
            messages += HeavyMessage(m.getString("id"), m.getInt("index"), m.getString("author"), m.getString("avatar"),
                m.getInt("minutesAgo"), paragraphs, photos,
                l?.let { HeavyLink(it.getString("thumb"), it.getString("title"), it.getString("description"), it.getString("site")) },
                q?.let { HeavyQuote(it.getString("id"), it.getString("author"), it.getString("excerpt")) }, reactions)
        }
        val images = assets.list("images")?.count { it.endsWith(".jpg") } ?: 0
        require(images == 104) { "The canonical heavy-list fixture has 104 JPEGs, got $images" }
        return HeavyDataset(messages, bytes.size, images)
    }
    private fun <T> JSONArray.objects(transform: (Any) -> T): List<T> = List(length()) { transform(get(it)) }
}

object HeavyColors {
    val hairline = Color.rgb(229, 229, 234)
    val secondary = Color.rgb(142, 142, 147)
    val quoteBar = Color.rgb(199, 199, 204)
    val fill = Color.rgb(242, 242, 247)
    val label2 = Color.rgb(60, 60, 67)
    val cardBorder = Color.rgb(209, 209, 214)
    val link = Color.rgb(0, 122, 255)
    val tag = Color.rgb(88, 86, 214)
}

/** Original models stay immutable. Only the bounded live prefix and keyed reaction deltas change. */
class HeavyStore(val data: HeavyDataset) {
    private val live = ArrayList<HeavyMessage>()
    private val bumps = HashMap<String, Int>()
    private val observers = HashMap<String, MutableSet<() -> Unit>>()
    val size: Int get() = live.size + data.messages.size
    operator fun get(position: Int): HeavyMessage = if (position < live.size) live[position] else data.messages[position - live.size]
    fun count(message: HeavyMessage, reaction: HeavyReaction): Int = reaction.count + (bumps[message.id + "|" + reaction.emoji] ?: 0)
    fun bump(message: HeavyMessage, reaction: HeavyReaction) {
        val key = message.id + "|" + reaction.emoji
        bumps[key] = (bumps[key] ?: 0) + 1
        observers[message.id]?.toList()?.forEach { it() }
    }
    fun insert(k: Int) {
        val original = data.messages[(k * 37) % data.messages.size]
        val reactions = original.reactions.map { it.copy(count = count(original, it)) }
        val m = original.copy(id = "live-$k", index = -k, minutesAgo = 0, reactions = reactions)
        live.add(0, m)
    }
    fun observe(id: String, callback: () -> Unit): () -> Unit {
        observers.getOrPut(id) { LinkedHashSet() }.add(callback)
        return { observers[id]?.let { it.remove(callback); if (it.isEmpty()) observers.remove(id) }; Unit }
    }
    fun metadata(): JSONObject = JSONObject().put("rows", data.messages.size).put("logical_rows", size)
        .put("json_bytes", data.jsonBytes).put("jpeg_files", data.imageFiles)
}

class HeavyLiveLoop(
    private val store: HeavyStore, private val insert: (Int) -> Unit, private val second: (Int) -> Unit,
    private val onConfigured: () -> Unit = {},
) {
    private val handler = Handler(Looper.getMainLooper())
    private var running = false
    private var k = 0
    private var configured = false
    private var startedUptimeMs = 0L
    var seconds = 0
        private set
    private val tick = object : Runnable {
        override fun run() {
            if (!running) return
            // Match the original Contract: tick one asks config; insertion begins at tick two.
            if (!configured) { configured = true; onConfigured() }
            else {
                k++; insert(k)
                val n = store.data.messages.size
                val start = (k * 101) % n
                for (offset in 0 until n) {
                    val selected = store.data.messages[(start + offset) % n]
                    if (selected.reactions.isNotEmpty()) {
                        store.bump(selected, selected.reactions[k % selected.reactions.size]); break
                    }
                }
            }
            handler.postDelayed(this, 250)
        }
    }
    private val secondTick = object : Runnable {
        override fun run() {
            if (!running) return
            seconds = ((SystemClock.uptimeMillis() - startedUptimeMs) / 1000).coerceIn(0, Int.MAX_VALUE.toLong()).toInt()
            if (configured) second(seconds)
            handler.postAtTime(this, startedUptimeMs + (seconds + 1L) * 1000)
        }
    }
    fun start() {
        if (!running) {
            running = true; startedUptimeMs = SystemClock.uptimeMillis()
            handler.postDelayed(tick, 250); handler.postAtTime(secondTick, startedUptimeMs + 1000)
        }
    }
    fun close() { running = false; handler.removeCallbacks(tick); handler.removeCallbacks(secondTick) }
}
