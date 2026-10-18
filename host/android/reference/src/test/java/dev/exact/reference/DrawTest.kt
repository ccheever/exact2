package dev.exact.reference

import android.os.Build
import androidx.compose.foundation.layout.calculateEndPadding
import androidx.compose.foundation.layout.calculateStartPadding
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.ColorScheme
import androidx.compose.material3.TextFieldDefaults
import androidx.compose.material3.darkColorScheme
import androidx.compose.material3.lightColorScheme
import androidx.compose.ui.geometry.Rect
import androidx.compose.ui.graphics.toArgb
import androidx.compose.ui.semantics.SemanticsActions
import androidx.compose.ui.semantics.SemanticsNode
import androidx.compose.ui.semantics.SemanticsProperties
import androidx.compose.ui.semantics.getOrNull
import androidx.compose.ui.test.junit4.createComposeRule
import androidx.compose.ui.test.onRoot
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.text.TextLayoutResult
import androidx.compose.ui.unit.LayoutDirection
import com.github.takahirom.roborazzi.captureRoboImage
import java.io.File
import org.json.JSONArray
import org.json.JSONObject
import org.junit.Assert.assertEquals
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.ParameterizedRobolectricTestRunner
import org.robolectric.annotation.Config
import org.robolectric.annotation.GraphicsMode

/**
 * Draws each specimen and each defect of `specimens.json` under each scheme, and writes beside each picture what
 * LLP 1115.001.001 D5 compares: the layout box, and every semantics node and text box under it, in dp from the
 * window's corner. The SDK is the pin's (`libs.versions.toml` `referenceSdk`, which the build passes as
 * `reference.sdk`); the window is the Pixel 10 Pro XL's, 1080 × 2404 pixels at 2.4375.
 */
@RunWith(ParameterizedRobolectricTestRunner::class)
@GraphicsMode(GraphicsMode.Mode.NATIVE)
@Config(sdk = [36], qualifiers = "w443dp-h986dp-390dpi")
class DrawTest(private val id: String, private val scheme: String) {
    @get:Rule val rule = createComposeRule()

    @Test fun draw() {
        assertEquals("the SDK pin", System.getProperty("reference.sdk"), Build.VERSION.SDK_INT.toString())
        val out = File(System.getProperty("reference.out")!!, "compose").apply { mkdirs() }
        val name = "$id.$scheme"
        rule.mainClock.autoAdvance = false
        rule.setContent { Specimen(id, schemeOf(scheme)) }
        // Long enough to settle a focus and a label's float (150 ms), and short of a caret's first blink (500 ms).
        rule.mainClock.advanceTimeBy(300)
        rule.onRoot().captureRoboImage(File(out, "$name.png").path)
        val density = rule.density.density
        val tagged = rule.onNodeWithTag(SPECIMEN, useUnmergedTree = true).fetchSemanticsNode()
        val nodes = JSONArray()
        val texts = JSONArray()
        fun walk(node: SemanticsNode) {
            val text = node.config.getOrNull(SemanticsProperties.EditableText)?.text
                ?: node.config.getOrNull(SemanticsProperties.Text)?.joinToString(" ") { it.text }
            nodes.put(JSONObject().put("box", box(node.boundsInRoot, density)).put("text", text)
                .put("role", node.config.getOrNull(SemanticsProperties.Role)?.toString()))
            if (text != null) {
                val layouts = mutableListOf<TextLayoutResult>()
                node.config.getOrNull(SemanticsActions.GetTextLayoutResult)?.action?.invoke(layouts)
                val l = layouts.firstOrNull()
                // A text field's node is its whole decoration box, and its layout is in the coordinates of its inner
                // text field: the one node under it that names no text (the clip round the value). Its value's box
                // is the first line's, there.
                val editable = node.config.getOrNull(SemanticsProperties.EditableText) != null
                val inner = if (!editable) node.boundsInRoot else node.children
                    .filter { it.config.getOrNull(SemanticsProperties.Text) == null }
                    .singleOrNull()?.boundsInRoot ?: error("$id: a text field with no single inner text field")
                val lines = JSONArray()
                if (l != null) for (i in 0 until l.lineCount) {
                    lines.put(JSONObject().put("top", (inner.top + l.getLineTop(i)) / density)
                        .put("bottom", (inner.top + l.getLineBottom(i)) / density)
                        .put("baseline", (inner.top + l.getLineBaseline(i)) / density))
                }
                val at = if (!editable || l == null || l.lineCount == 0) node.boundsInRoot else Rect(
                    inner.left + l.getLineLeft(0), inner.top + l.getLineTop(0),
                    inner.left + l.getLineRight(0), inner.top + l.getLineBottom(0))
                // An empty field's value is nothing to compare.
                if (text.isNotEmpty() || !editable) {
                    texts.put(JSONObject().put("text", text).put("box", box(at, density)).put("lines", lines))
                }
            }
            node.children.forEach(::walk)
        }
        walk(tagged)
        val facts = JSONObject().put("id", id).put("scheme", scheme).put("density", density.toDouble())
            .put("layout", box(tagged.boundsInRoot, density)).put("nodes", nodes).put("texts", texts)
        File(out, "$name.json").writeText(facts.toString(2))
    }

    companion object {
        /**
         * Each specimen under each scheme, and each defect under the light one. The runner asks for these outside
         * Robolectric's sandbox, where `org.json` is the SDK's stub, so the manifest's ids are read as text.
         */
        @JvmStatic
        @ParameterizedRobolectricTestRunner.Parameters(name = "{0}.{1}")
        fun specimens(): List<Array<Any>> {
            val text = File(System.getProperty("reference.specimens")!!).readText()
            val ids = { section: String -> Regex(""""id": "([^"]+)"""").findAll(section).map { it.groupValues[1] }.toList() }
            val schemes = Regex(""""schemes": \[([^\]]*)\]""").find(text)!!.groupValues[1]
                .split(',').map { it.trim().trim('"') }
            val (specimens, defects) = text.indexOf("\"defects\"").let { ids(text.substring(0, it)) to ids(text.substring(it)) }
            return specimens.flatMap { id -> schemes.map { arrayOf<Any>(id, it) } } + defects.map { arrayOf<Any>(it, "light") }
        }

        fun schemeOf(name: String) = if (name == "dark") darkColorScheme() else lightColorScheme()
    }
}

/**
 * What crosses to Exact (D4: inputs held equal): each scheme's colours by Material's names, as `0xAARRGGBB`, and the
 * `*Defaults` values the painter reads that are not tokens (D2.2).
 */
@RunWith(org.robolectric.RobolectricTestRunner::class)
@GraphicsMode(GraphicsMode.Mode.NATIVE)
@Config(sdk = [36], qualifiers = "w443dp-h986dp-390dpi")
class InputsTest {
    @Test fun write() {
        val out = File(System.getProperty("reference.out")!!).apply { mkdirs() }
        val schemes = JSONObject()
        for (name in listOf("light", "dark")) schemes.put(name, scheme(DrawTest.schemeOf(name)))
        File(out, "schemes.json").writeText(schemes.toString(2))
        val padding = ButtonDefaults.ContentPadding
        val defaults = JSONObject()
            .put("TextFieldDefaults.MinHeight", TextFieldDefaults.MinHeight.value)
            .put("TextFieldDefaults.MinWidth", TextFieldDefaults.MinWidth.value)
            .put("TextFieldDefaults.FocusedIndicatorThickness", TextFieldDefaults.FocusedIndicatorThickness.value)
            .put("TextFieldDefaults.UnfocusedIndicatorThickness", TextFieldDefaults.UnfocusedIndicatorThickness.value)
            .put("ButtonDefaults.MinHeight", ButtonDefaults.MinHeight.value)
            .put("ButtonDefaults.MinWidth", ButtonDefaults.MinWidth.value)
            .put("ButtonDefaults.ContentPadding.start", padding.calculateStartPadding(LayoutDirection.Ltr).value)
            .put("ButtonDefaults.ContentPadding.top", padding.calculateTopPadding().value)
            .put("ButtonDefaults.ContentPadding.end", padding.calculateEndPadding(LayoutDirection.Ltr).value)
            .put("ButtonDefaults.ContentPadding.bottom", padding.calculateBottomPadding().value)
        File(out, "defaults.json").writeText(defaults.toString(2))
    }

    private fun scheme(s: ColorScheme) = JSONObject().apply {
        roles(s).forEach { (k, v) -> put(k, v.toArgb().toLong() and 0xFFFFFFFFL) }
    }
}

private fun box(r: Rect, density: Float) = JSONObject()
    .put("x", r.left / density).put("y", r.top / density).put("w", r.width / density).put("h", r.height / density)
