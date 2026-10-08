@file:OptIn(androidx.compose.foundation.layout.ExperimentalLayoutApi::class)

package dev.exact.heavybench

import android.graphics.Bitmap
import android.os.Bundle
import android.os.SystemClock
import androidx.activity.ComponentActivity
import androidx.compose.foundation.Image
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.BoxWithConstraints
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.BasicText
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.draw.drawBehind
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.asImageBitmap
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.platform.ComposeView
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.platform.ViewCompositionStrategy
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.PlatformTextStyle
import androidx.compose.ui.text.SpanStyle
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.buildAnnotatedString
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.font.FontStyle
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.Hyphens
import androidx.compose.ui.text.style.LineBreak
import androidx.compose.ui.text.style.TextDecoration
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import java.util.Locale

class ComposeActivity : ComponentActivity() {
    private lateinit var images: HeavyImageLoader
    private lateinit var harness: HeavyBenchHarness
    override fun onCreate(state: Bundle?) {
        val created = SystemClock.elapsedRealtimeNanos()
        super.onCreate(state)
        HeavyWindow.configure(this)
        val store = HeavyStore(HeavyData.load(assets))
        images = HeavyImageLoader(this)
        harness = HeavyBenchHarness(this, "compose", created)
        val view = ComposeView(this).apply { setViewCompositionStrategy(ViewCompositionStrategy.DisposeOnViewTreeLifecycleDestroyed) }
        val enabled = HeavyOptions.boolean(intent, "BENCH_LIVE")
        val startIndex = HeavyOptions.int(intent, "BENCH_START_INDEX", 0).coerceIn(0, store.size - 1)
        view.setContent {
            val list = rememberLazyListState(initialFirstVisibleItemIndex = startIndex)
            val mounted = remember { LinkedHashSet<String>() }
            var insertionVersion by remember { mutableIntStateOf(0) }
            var seconds by remember { mutableIntStateOf(0) }
            var liveConfigured by remember { mutableStateOf(false) }
            val target = remember(list) { object : ScrollTarget {
                override fun scrollByPx(delta: Float): Float = list.dispatchRawDelta(delta)
                override fun visibleRows(): Int = list.layoutInfo.visibleItemsInfo.size
                override fun mountedRows(): Int = mounted.size
                override fun firstVisibleId(): String? = list.layoutInfo.visibleItemsInfo.firstOrNull()?.key as? String
                override val mountedRowKind: String get() = "active-row-compositions-including-prefetch"
            } }
            DisposableEffect(target) {
                harness.attach(view, target, { list.layoutInfo.visibleItemsInfo.isNotEmpty() }, {
                    store.metadata().put("images", images.diagnosticSnapshot()).put("list", "LazyColumn").put("font_padding", false)
                })
                val live = if (enabled) HeavyLiveLoop(store, { k ->
                    val index = list.firstVisibleItemIndex; val offset = list.firstVisibleItemScrollOffset
                    store.insert(k); insertionVersion++
                    list.requestScrollToItem(index + 1, offset)
                }, { seconds = it }, { liveConfigured = true }) else null
                live?.start()
                onDispose { live?.close(); harness.close() }
            }
            Column(Modifier.fillMaxSize().background(Color.White)) {
                Row(Modifier.fillMaxWidth().padding(horizontal = 16.dp, vertical = 10.dp), verticalAlignment = Alignment.CenterVertically) {
                    Label("Heavy list", 17, weight = FontWeight.SemiBold, modifier = Modifier.weight(1f))
                    Label(if (enabled && liveConfigured) "Live: on" else "Live: off", 13, Color(HeavyColors.secondary))
                }
                Box(Modifier.fillMaxWidth().height(.5.dp).background(Color(HeavyColors.hairline)))
                val total = remember(insertionVersion) { store.size }
                LazyColumn(Modifier.weight(1f).fillMaxWidth(), state = list) {
                    items(total, key = { store[it].id }, contentType = { "message" }) { index ->
                        val message = store[index]
                        DisposableEffect(message.id) { mounted.add(message.id); onDispose { mounted.remove(message.id) } }
                        MessageRow(message, store, images, seconds)
                    }
                }
            }
        }
        HeavyWindow.install(this, view)
    }
    override fun onDestroy() { if (::harness.isInitialized) harness.close(); if (::images.isInitialized) images.close(); super.onDestroy() }
}

private fun style(size: Int, color: Color = Color.Black, weight: FontWeight = FontWeight.Normal): TextStyle = TextStyle(
    color = color, fontSize = size.sp, fontFamily = FontFamily.Default, fontWeight = weight,
    platformStyle = PlatformTextStyle(includeFontPadding = false), lineBreak = LineBreak.Simple, hyphens = Hyphens.None,
)
@Composable private fun Label(text: String, size: Int, color: Color = Color.Black, weight: FontWeight = FontWeight.Normal,
                              maxLines: Int = Int.MAX_VALUE, modifier: Modifier = Modifier) {
    BasicText(text, modifier, style(size, color, weight), maxLines = maxLines, overflow = if (maxLines < Int.MAX_VALUE) TextOverflow.Ellipsis else TextOverflow.Clip)
}

@Composable private fun Paragraph(runs: List<TextRun>) {
    val text = remember(runs) {
        buildAnnotatedString {
            for (run in runs) {
                val start = length
                append(run.text)
                val span = when (run.style) {
                    "bold" -> SpanStyle(fontWeight = FontWeight.SemiBold)
                    "italic" -> SpanStyle(fontStyle = FontStyle.Italic)
                    "code" -> SpanStyle(fontFamily = FontFamily.Monospace, fontSize = 15.sp, background = Color(HeavyColors.fill))
                    "link" -> SpanStyle(color = Color(HeavyColors.link), textDecoration = TextDecoration.Underline)
                    "mention" -> SpanStyle(color = Color(HeavyColors.link), fontWeight = FontWeight.SemiBold)
                    "tag" -> SpanStyle(color = Color(HeavyColors.tag))
                    else -> SpanStyle()
                }
                addStyle(span, start, length)
            }
        }
    }
    BasicText(text, Modifier.fillMaxWidth(), style(16))
}

@Composable private fun Picture(file: String, width: Dp, height: Dp, loader: HeavyImageLoader, modifier: Modifier = Modifier) {
    val density = LocalDensity.current
    val w = with(density) { width.roundToPx().coerceAtLeast(1) }
    val h = with(density) { height.roundToPx().coerceAtLeast(1) }
    var bitmap by remember(file, w, h) { mutableStateOf<Bitmap?>(null) }
    DisposableEffect(file, w, h) { val cancel = loader.request(file, w, h) { bitmap = it }; onDispose { cancel() } }
    Box(modifier.size(width, height).background(Color(HeavyColors.hairline))) {
        bitmap?.let { Image(it.asImageBitmap(), contentDescription = null, contentScale = ContentScale.Crop, modifier = Modifier.fillMaxSize()) }
    }
}

@Composable private fun MessageRow(message: HeavyMessage, store: HeavyStore, images: HeavyImageLoader, seconds: Int) {
    val time = message.time(seconds)
    var revision by remember(message.id) { mutableIntStateOf(0) }
    DisposableEffect(message.id) { val cancel = store.observe(message.id) { revision++ }; onDispose { cancel() } }
    val counts = remember(message.id, revision) { message.reactions.map { store.count(message, it) } }
    Row(Modifier.fillMaxWidth().background(Color.White).drawBehind {
        val y = size.height - .25.dp.toPx()
        drawLine(Color(HeavyColors.hairline), Offset(68.dp.toPx(), y), Offset(size.width - 16.dp.toPx(), y), .5.dp.toPx())
    }.padding(horizontal = 16.dp, vertical = 12.dp).semantics(mergeDescendants = true) { contentDescription = "${message.author}, $time" },
        verticalAlignment = Alignment.Top) {
        Picture(message.avatar, 40.dp, 40.dp, images, Modifier.clip(CircleShape))
        Spacer(Modifier.width(12.dp))
        BoxWithConstraints(Modifier.weight(1f)) {
            val width = maxWidth
            Column(Modifier.fillMaxWidth()) {
                Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.spacedBy(6.dp)) {
                    Label(message.author, 15, weight = FontWeight.SemiBold, maxLines = 1, modifier = Modifier.alignByBaseline())
                    Label(time, 13, Color(HeavyColors.secondary), maxLines = 1, modifier = Modifier.alignByBaseline())
                }
                message.quote?.let { quote ->
                    Box(Modifier.fillMaxWidth().padding(top = 6.dp).clip(RoundedCornerShape(8.dp)).background(Color(HeavyColors.fill)).drawBehind {
                        drawRect(Color(HeavyColors.quoteBar), size = Size(3.dp.toPx(), size.height))
                    }) {
                        Column(Modifier.padding(start = 11.dp, end = 8.dp, top = 8.dp, bottom = 8.dp)) {
                            Label(quote.author, 13, Color(HeavyColors.label2), FontWeight.SemiBold)
                            Label(quote.excerpt, 13, Color(HeavyColors.label2), maxLines = 2)
                        }
                    }
                }
                Spacer(Modifier.height(6.dp))
                message.paragraphs.forEachIndexed { i, paragraph -> if (i > 0) Spacer(Modifier.height(8.dp)); Paragraph(paragraph) }
                if (message.photos.isNotEmpty()) { Spacer(Modifier.height(8.dp)); Photos(message.photos, width, images) }
                message.link?.let { link ->
                    Spacer(Modifier.height(8.dp))
                    val shape = RoundedCornerShape(12.dp)
                    Column(Modifier.fillMaxWidth().clip(shape).border(.5.dp, Color(HeavyColors.cardBorder), shape)) {
                        Picture(link.thumb, width, 140.dp, images)
                        Column(Modifier.padding(10.dp)) {
                            Label(link.site.uppercase(Locale.ROOT), 12, Color(HeavyColors.secondary))
                            Label(link.title, 15, weight = FontWeight.SemiBold, maxLines = 2)
                            Label(link.description, 13, Color(HeavyColors.label2), maxLines = 2)
                        }
                    }
                }
                if (message.reactions.isNotEmpty()) {
                    Spacer(Modifier.height(8.dp))
                    FlowRow(horizontalArrangement = Arrangement.spacedBy(6.dp), verticalArrangement = Arrangement.spacedBy(6.dp)) {
                        message.reactions.forEachIndexed { i, reaction ->
                            Row(Modifier.height(28.dp).clip(RoundedCornerShape(14.dp)).background(Color(HeavyColors.fill))
                                .clickable(role = Role.Button) { store.bump(message, reaction) }.padding(horizontal = 10.dp), verticalAlignment = Alignment.CenterVertically) {
                                Label(reaction.emoji, 14); Spacer(Modifier.width(4.dp)); Label(counts[i].toString(), 13, Color(HeavyColors.label2), FontWeight.SemiBold)
                            }
                        }
                    }
                }
            }
        }
    }
}

@Composable private fun Photos(photos: List<HeavyPhoto>, width: Dp, loader: HeavyImageLoader) {
    Box(Modifier.clip(RoundedCornerShape(12.dp))) {
        when (photos.size) {
            1 -> Picture(photos[0].src, width, minOf(width * photos[0].height.toFloat() / photos[0].width, 320.dp), loader)
            2 -> { val side = (width - 4.dp) / 2; Row(horizontalArrangement = Arrangement.spacedBy(4.dp)) { photos.forEach { Picture(it.src, side, side, loader) } } }
            3 -> {
                val small = (width - 4.dp) / 3; val big = width - 4.dp - small; val short = (big - 4.dp) / 2
                Row(horizontalArrangement = Arrangement.spacedBy(4.dp)) {
                    Picture(photos[0].src, big, big, loader)
                    Column(verticalArrangement = Arrangement.spacedBy(4.dp)) { Picture(photos[1].src, small, short, loader); Picture(photos[2].src, small, short, loader) }
                }
            }
            else -> {
                val side = (width - 4.dp) / 2
                Column(verticalArrangement = Arrangement.spacedBy(4.dp)) {
                    Row(horizontalArrangement = Arrangement.spacedBy(4.dp)) { Picture(photos[0].src, side, side, loader); Picture(photos[1].src, side, side, loader) }
                    Row(horizontalArrangement = Arrangement.spacedBy(4.dp)) { Picture(photos[2].src, side, side, loader); Picture(photos[3].src, side, side, loader) }
                }
            }
        }
    }
}
