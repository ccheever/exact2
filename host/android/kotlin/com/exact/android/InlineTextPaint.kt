package com.exact.android

import org.json.JSONArray
import org.json.JSONObject

/** The shared Apple paragraph wire: textual descendants are values, not Views. */
internal object InlineTextPaint {
    private data class Row(
        val parent: Int, val text: String, val paints: Boolean, val color: InlinePaintColor?,
        val background: InlinePaintColor?, val decoration: Int, val hidden: Boolean
    )

    fun read(owner: Int, input: JSONArray): InlinePaintModel {
        val rows = LinkedHashMap<Int, Row>(input.length())
        val pieces = ArrayList<InlinePaintPiece>()
        repeat(input.length()) { index ->
            val value = input.getJSONObject(index)
            val id = value.getInt("id")
            val parent = value.getInt("parent")
            require(id != owner && !rows.containsKey(id)) { "duplicate Android inline text identity" }
            require(parent == owner || rows.containsKey(parent)) { "Android inline text parent must precede its child" }
            val props = value.getJSONObject("props")
            val style = value.getJSONObject("style")
            require(style.optString("text_transform", "none") == "none") { "Android inline text-transform is not implemented" }
            require(!style.has("text_shadow") || style.isNull("text_shadow") || style.optString("text_shadow") == "none") { "Android inline text-shadow is not implemented" }
            require(style.optDouble("text_stroke_width", 0.0) == 0.0) { "Android inline text stroke is not implemented" }
            require(style.optString("background_clip", "border-box") != "text") { "Android inline background-clip:text is not implemented" }
            require(!style.has("background_image") || style.isNull("background_image") || style.optString("background_image") == "none") { "Android inline background images are not implemented" }
            val row = Row(parent, props.optString("text", ""), value.getBoolean("paint"),
                color(style, "text_color"), color(style, "background_color"),
                InlinePaintModel.decoration(style.optString("text_decoration_line", "none")),
                style.optString("visibility", "visible") == "hidden")
            rows[id] = row
            if (!row.paints) return@repeat
            val backgrounds = ArrayList<InlinePaintColor>()
            var decoration = row.decoration
            var at: Row? = row
            while (at != null) {
                if (!at.hidden) at.background?.let { backgrounds.add(it) }
                // Decorations propagate through the inline tree, even though
                // text-decoration-line is not an inherited CSS property.
                if (!at.hidden) decoration = decoration or at.decoration
                at = rows[at.parent]
            }
            backgrounds.reverse()
            pieces.add(InlinePaintPiece(row.text, row.color, backgrounds, decoration, row.hidden))
        }
        return InlinePaintModel(pieces)
    }

    private fun color(style: JSONObject, key: String): InlinePaintColor? {
        if (!style.has(key) || style.isNull(key)) return null
        val values = style.getJSONArray(key)
        fun channels(values: JSONArray): Int {
            require(values.length() == 4) { "invalid Android inline color" }
            val c = IntArray(4) { index ->
                val value = values.getDouble(index)
                require(value.isFinite() && value in 0.0..255.0 && value == value.toInt().toDouble()) {
                    "Android inline wide-gamut colors are not implemented"
                }
                value.toInt()
            }
            return (c[3] shl 24) or (c[0] shl 16) or (c[1] shl 8) or c[2]
        }
        if (values.optJSONArray(0) != null) {
            require(values.length() == 2) { "invalid Android inline color pair" }
            return InlinePaintColor(channels(values.getJSONArray(0)), channels(values.getJSONArray(1)))
        }
        return InlinePaintColor(channels(values))
    }
}
