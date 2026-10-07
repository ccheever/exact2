package com.exact.compose

import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.ScrollState
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.WindowInsets
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.safeDrawing
import androidx.compose.foundation.layout.windowInsetsPadding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.BasicText
import androidx.compose.foundation.text.BasicTextField
import androidx.compose.foundation.verticalScroll
import androidx.compose.runtime.Composable
import androidx.compose.runtime.SideEffect
import androidx.compose.runtime.Stable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.key
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.drawBehind
import androidx.compose.ui.geometry.CornerRadius
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.SolidColor
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.layout.LayoutCoordinates
import androidx.compose.ui.layout.onGloballyPositioned
import androidx.compose.ui.layout.positionInRoot
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.PlatformTextStyle
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import kotlin.math.roundToInt

/** State shared by the real UI handlers and the benchmark actions. */
@Stable
internal class ComposeScene {
    var count by mutableIntStateOf(0)
        private set
    var draft by mutableStateOf("")
        private set
    var rowCount by mutableIntStateOf(100)
        private set
    var highlighted by mutableStateOf(false)
        private set
    var moved by mutableStateOf(false)
        private set
    var heavy by mutableStateOf(false)
        private set

    var revision = 0L
        private set
    var appliedRevision = 0L
        private set
    private var pending = ""
    private var scroll: ScrollState? = null
    private var viewportCoordinates: LayoutCoordinates? = null
    private var contentCoordinates: LayoutCoordinates? = null
    val scrollPositionPx: Int get() = scroll?.value ?: 0
    val renderedScrollPositionPx: Int get() {
        val viewport = viewportCoordinates ?: return -1
        val content = contentCoordinates ?: return -1
        if (!viewport.isAttached || !content.isAttached) return -1
        return (viewport.positionInRoot().y - content.positionInRoot().y).roundToInt()
    }
    val scrollViewportPx: Int get() = scroll?.viewportSize ?: 0
    val scrollRangePx: Int get() {
        val state = scroll ?: return 0
        val range = state.maxValue
        return if (range == Int.MAX_VALUE) 0 else range
    }

    fun bindScroll(state: ScrollState) { scroll = state }
    fun bindScrollViewport(coordinates: LayoutCoordinates) { viewportCoordinates = coordinates }
    fun bindScrollContent(coordinates: LayoutCoordinates) { contentCoordinates = coordinates }
    // Programmatic placement only, matching ScrollView.scrollTo rather than a fling.
    fun scrollTo(offsetPx: Int) {
        val state = checkNotNull(scroll) { "The eager scroll scene has not been composed" }
        val target = offsetPx.coerceIn(0, state.maxValue)
        state.dispatchRawDelta((target - state.value).toFloat())
    }

    private fun changed(kind: String) {
        pending = kind
        revision++
    }
    fun heavyList() { changed("prepare"); heavy = true; rowCount = 1000; highlighted = false; moved = false }
    fun simpleList() { changed("prepare"); heavy = false }
    fun increment() { changed("counter"); count++ }
    fun edit(value: String) { changed("edit"); draft = value }
    fun toggleBackground() { changed("paint"); highlighted = !highlighted }
    fun toggleMove() { changed("transform"); moved = !moved }
    fun setRows(value: Int) {
        require(value == 1 || value == 100 || value == 1000)
        if (rowCount != value) { changed("prepare"); rowCount = value }
    }
    fun prepare(workload: String) {
        if (workload == "heavy-scroll-1000") heavyList()
        if (workload == "scroll-1000" || workload == "heavy-scroll-1000") {
            highlighted = false
            moved = false
            scroll?.let { it.dispatchRawDelta(-it.value.toFloat()) }
        }
        val value = when (workload) {
            "counter", "paint-1" -> 1
            "paint-100" -> 100
            "paint-1000", "transform-1000", "scroll-1000", "heavy-scroll-1000" -> 1000
            else -> error("Unknown workload '$workload'")
        }
        changed("prepare")
        if (rowCount == value) appliedRevision = revision else rowCount = value
    }
    fun performAction(workload: String) = when (workload) {
        "counter" -> increment()
        "paint-1", "paint-100", "paint-1000" -> toggleBackground()
        "transform-1000" -> toggleMove()
        else -> error("Unknown workload '$workload'")
    }

    // The harness combines this represented revision with the completed window
    // draw and its FrameMetrics report; mutation alone is not a rendered action.
    fun represented(kind: String) {
        if (pending == kind) appliedRevision = revision
    }
}

private val Ink = Color(0xff1a1c1e)
private val Blue = Color(0xff0061a4)
private val PaleBlue = Color(0xffd1e4ff)
private val Page = Color(0xfff8f9ff)
private val ButtonGrey = Color(0xffe1e2ec)
private val BaseText = TextStyle(color = Ink, fontSize = 16.sp,
    platformStyle = PlatformTextStyle(includeFontPadding = false))

/** Eager rows deliberately match Exact's retained list; this is not LazyColumn. */
@Composable
internal fun CoreScene(scene: ComposeScene) {
    if (scene.heavy) { HeavyScene(scene); return }
    Column(Modifier.fillMaxSize().background(Page).windowInsetsPadding(WindowInsets.safeDrawing)) {
        Column(Modifier.fillMaxWidth().padding(20.dp), verticalArrangement = Arrangement.spacedBy(14.dp)) {
            BasicText("Exact on Android", Modifier.testTag("heavy-list").clickable(onClick = scene::heavyList),
                style = BaseText.copy(fontSize = 28.sp, fontWeight = FontWeight.Bold))
            BasicText("One Contract, native text, retained Android views.",
                style = BaseText.copy(color = Color(0xff44474e), lineHeight = 22.sp))
            Row(horizontalArrangement = Arrangement.spacedBy(12.dp), verticalAlignment = Alignment.CenterVertically) {
                CoreButton("Increment", "increment", "Increment counter", Blue, Color.White, 12, 20, onClick = scene::increment)
                Counter(scene)
            }
            Draft(scene)
            PaintButton(scene)
            Row(horizontalArrangement = Arrangement.spacedBy(8.dp), verticalAlignment = Alignment.CenterVertically) {
                // Match the fixture's CSS flex widths at the benchmark viewport;
                // bounded weights retain wrapping instead of hiding the last button.
                CoreButton("1 row", "rows-1", "1 row", ButtonGrey, Ink, 8, 8, weight = 400,
                    modifier = Modifier.weight(53f)) { scene.setRows(1) }
                CoreButton("100 rows", "rows-100", "100 rows", ButtonGrey, Ink, 8, 8, weight = 400,
                    modifier = Modifier.weight(56f)) { scene.setRows(100) }
                CoreButton("1,000 rows", "rows-1000", "1,000 rows", ButtonGrey, Ink, 8, 8, weight = 400,
                    modifier = Modifier.weight(73f)) { scene.setRows(1000) }
                CoreButton("Move rows", "toggle-move", "Move rows", ButtonGrey, Ink, 8, 8, weight = 400,
                    modifier = Modifier.weight(74f), onClick = scene::toggleMove)
            }
        }
        EagerRows(scene, Modifier.weight(1f))
    }
}

@Composable
private fun CoreButton(text: String, tag: String, label: String, background: Color, foreground: Color,
    padding: Int, radius: Int, weight: Int = 600, modifier: Modifier = Modifier, onClick: () -> Unit) {
    BasicText(text, modifier.testTag(tag).semantics { contentDescription = label }
        .background(background, RoundedCornerShape(radius.dp)).clickable(role = Role.Button, onClick = onClick)
        .padding(padding.dp), style = BaseText.copy(color = foreground, fontWeight = FontWeight(weight)))
}

@Composable
private fun Counter(scene: ComposeScene) {
    BasicText("Count: ${scene.count}", Modifier.testTag("counter"), style = BaseText.copy(fontSize = 20.sp))
    SideEffect { scene.represented("counter") }
}

@Composable
private fun Draft(scene: ComposeScene) {
    BasicTextField(scene.draft, scene::edit, Modifier.fillMaxWidth().testTag("draft")
        .semantics { contentDescription = "Message" }.background(Color.White, RoundedCornerShape(8.dp))
        // Compose's border overlays content; CSS adds its 1px to the 12px padding.
        .border(1.dp, Color(0xff74777f), RoundedCornerShape(8.dp)).padding(13.dp),
        singleLine = true, textStyle = BaseText, cursorBrush = SolidColor(Blue),
        decorationBox = { inner ->
            Box {
                if (scene.draft.isEmpty()) BasicText("Type a message", style = BaseText.copy(color = Color(0xff74777f)))
                inner()
            }
        })
    BasicText("You typed: ${scene.draft}", Modifier.testTag("echo"), style = BaseText.copy(lineHeight = 22.sp))
    SideEffect { scene.represented("edit") }
}

@Composable
private fun PaintButton(scene: ComposeScene) {
    CoreButton("Change ${scene.rowCount} row backgrounds", "toggle-batch", "Change all row backgrounds",
        PaleBlue, Color(0xff001d36), 12, 20, modifier = Modifier.fillMaxWidth(), onClick = scene::toggleBackground)
}

@Composable
private fun EagerRows(scene: ComposeScene, modifier: Modifier) {
    val count = scene.rowCount
    val scroll = rememberScrollState()
    SideEffect { scene.bindScroll(scroll) }
    Column(modifier.fillMaxWidth().testTag("core-scroll")
        .onGloballyPositioned(scene::bindScrollViewport).verticalScroll(scroll)) {
        Column(Modifier.fillMaxWidth().testTag("rows-container")
            .onGloballyPositioned(scene::bindScrollContent)
            .graphicsLayer {
                translationX = if (scene.moved) 12.dp.toPx() else 0f
                scene.represented("transform")
            }.padding(start = 20.dp, end = 20.dp, bottom = 20.dp),
            verticalArrangement = Arrangement.spacedBy(6.dp)) {
            repeat(count) { index -> key(index) { CoreRow(scene, index) } }
        }
    }
    SideEffect { scene.represented("prepare") }
}

@Composable
private fun CoreRow(scene: ComposeScene, index: Int) {
    BasicText("Native row ${index + 1}", Modifier.fillMaxWidth().testTag("row-$index")
        .drawBehind {
            drawRoundRect(if (scene.highlighted) PaleBlue else Color.White,
                cornerRadius = CornerRadius(8.dp.toPx()))
            scene.represented("paint")
        }.padding(12.dp), style = BaseText)
}

/** Same rich, fully retained rows as Contract and the platform Views reference. */
@Composable
private fun HeavyScene(scene: ComposeScene) {
    val scroll = rememberScrollState()
    SideEffect { scene.bindScroll(scroll); scene.represented("prepare") }
    Column(Modifier.fillMaxSize().background(Page).windowInsetsPadding(WindowInsets.safeDrawing)) {
        Row(Modifier.fillMaxWidth().height(64.dp).padding(16.dp),
            horizontalArrangement = Arrangement.SpaceBetween, verticalAlignment = Alignment.CenterVertically) {
            BasicText("Heavy list · 1,000", style = BaseText.copy(fontSize = 18.sp,
                fontWeight = FontWeight.Bold, lineHeight = 22.sp))
            BasicText("Simple", Modifier.testTag("simple-list").background(ButtonGrey, RoundedCornerShape(8.dp))
                .clickable(onClick = scene::simpleList).padding(horizontal = 8.dp, vertical = 6.dp),
                style = BaseText.copy(fontSize = 14.sp, lineHeight = 20.sp))
        }
        Column(Modifier.weight(1f).fillMaxWidth().testTag("core-scroll")
            .onGloballyPositioned(scene::bindScrollViewport).verticalScroll(scroll)) {
            Column(Modifier.fillMaxWidth().testTag("rows-container")
                .onGloballyPositioned(scene::bindScrollContent)
                .padding(start = 20.dp, end = 20.dp, bottom = 20.dp),
                verticalArrangement = Arrangement.spacedBy(8.dp)) {
                repeat(1000) { index -> key(index) { HeavyCard(index) } }
            }
        }
    }
}

@Composable
private fun HeavyCard(index: Int) {
    Column(Modifier.fillMaxWidth().height(200.dp).testTag("row-$index")
        .background(Color.White, RoundedCornerShape(12.dp))
        .border(1.dp, Color(0xffd9dce3), RoundedCornerShape(12.dp))
        .graphicsLayer { clip = true; shape = RoundedCornerShape(12.dp) }.padding(13.dp),
        verticalArrangement = Arrangement.spacedBy(8.dp)) {
        Row(Modifier.height(40.dp), horizontalArrangement = Arrangement.spacedBy(10.dp),
            verticalAlignment = Alignment.CenterVertically) {
            BasicText("EX", Modifier.width(40.dp).height(40.dp).background(PaleBlue, RoundedCornerShape(20.dp)),
                style = BaseText.copy(fontSize = 22.sp, lineHeight = 40.sp,
                    textAlign = androidx.compose.ui.text.style.TextAlign.Center))
            Column(Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(2.dp)) {
                BasicText("Heavy row ${index + 1}", style = BaseText.copy(fontSize = 16.sp,
                    fontWeight = FontWeight.Bold, lineHeight = 18.sp))
                BasicText("Native rendering / retained content", style = BaseText.copy(fontSize = 12.sp,
                    lineHeight = 16.sp, color = Color(0xff44474e)))
            }
        }
        BasicText("Card ${index + 1}: Retained content with a detailed status update. This card wraps text across several lines while the long list moves.",
            Modifier.fillMaxWidth().height(60.dp).graphicsLayer { clip = true },
            style = BaseText.copy(fontSize = 14.sp, lineHeight = 20.sp))
        Row(Modifier.fillMaxWidth().height(18.dp), horizontalArrangement = Arrangement.SpaceBetween,
            verticalAlignment = Alignment.CenterVertically) {
            BasicText("- 128 / 32 + 256", style = BaseText.copy(fontSize = 12.sp, lineHeight = 18.sp,
                color = Color(0xff44474e)))
            BasicText("12:34", style = BaseText.copy(fontSize = 12.sp, lineHeight = 18.sp, color = Color(0xff44474e)))
        }
        Row(Modifier.height(28.dp), horizontalArrangement = Arrangement.spacedBy(8.dp),
            verticalAlignment = Alignment.CenterVertically) {
            BasicText("Android", Modifier.background(ButtonGrey, RoundedCornerShape(6.dp))
                .padding(horizontal = 8.dp, vertical = 6.dp), style = BaseText.copy(fontSize = 12.sp, lineHeight = 16.sp))
            BasicText("Row ${index + 1}", Modifier.background(PaleBlue, RoundedCornerShape(6.dp))
                .padding(horizontal = 8.dp, vertical = 6.dp), style = BaseText.copy(fontSize = 12.sp,
                    lineHeight = 16.sp, color = Color(0xff001d36)))
        }
    }
}
