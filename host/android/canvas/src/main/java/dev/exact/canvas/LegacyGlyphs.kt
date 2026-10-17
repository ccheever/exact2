package dev.exact.canvas

import android.graphics.Canvas
import android.graphics.Paint
import android.graphics.Typeface
import android.graphics.fonts.Font
import android.graphics.fonts.FontFamily
import java.nio.ByteBuffer

/**
 * Glyph runs on Android 10 and 11, which have no `Canvas.drawGlyphs` (API 31) and no other public way to draw a glyph
 * by its id. Each glyph exact2 shaped is drawn as the character the font's `cmap` maps to it, alone, at the position
 * exact2 gave it, in a typeface of that one font: what Android draws for that character is the same glyph. A glyph no
 * character maps to (a ligature, a contextual or positional form) is not drawn. Entered only below API 31.
 */
internal object LegacyGlyphs {
    private class Face(val typeface: Typeface, val chars: IntArray)

    /** By identity: a [Font]'s own hash reads its whole buffer. The readers keep one [Font] per face they were told. */
    private val faces = java.util.IdentityHashMap<Font, Face>()

    fun draw(c: Canvas, ids: IntArray, positions: FloatArray, count: Int, font: Font, paint: Paint) {
        val face = synchronized(faces) { faces.getOrPut(font) { face(font) } }
        paint.typeface = face.typeface
        val one = CharArray(2)
        for (g in 0 until count) {
            val id = ids[g]
            val char = if (id > 0 && id < face.chars.size) face.chars[id] else 0
            if (char == 0) continue
            c.drawText(one, 0, Character.toChars(char, one, 0), positions[2 * g], positions[2 * g + 1], paint)
        }
    }

    private fun face(font: Font): Face {
        val typeface = Typeface.CustomFallbackBuilder(FontFamily.Builder(font).build()).build()
        val chars = try { chars(font.buffer.duplicate(), font.ttcIndex) } catch (e: IndexOutOfBoundsException) { IntArray(0) }
        return Face(typeface, chars)
    }

    private fun u16(b: ByteBuffer, at: Int) = b.getShort(at).toInt() and 0xFFFF

    /** Each glyph's character, the lowest the font's best Unicode `cmap` subtable maps to it (0: none), from the
     * OpenType tables of face [ttc] in [b]. */
    private fun chars(b: ByteBuffer, ttc: Int): IntArray {
        val dir = if (b.getInt(0) == 0x74746366) b.getInt(12 + 4 * ttc) else 0 // 'ttcf': a collection's offsets.
        var cmap = -1; var maxp = -1
        for (t in 0 until u16(b, dir + 4)) {
            val record = dir + 12 + 16 * t
            when (b.getInt(record)) { 0x636D6170 -> cmap = b.getInt(record + 8); 0x6D617870 -> maxp = b.getInt(record + 8) }
        }
        if (cmap < 0 || maxp < 0) return IntArray(0)
        val out = IntArray(u16(b, maxp + 4))
        fun put(char: Int, glyph: Int) { if (glyph in 1 until out.size && out[glyph] == 0) out[glyph] = char }
        // Format 12 (all of Unicode) over format 4 (the Basic Multilingual Plane).
        var table = -1; var format = 0
        for (k in 0 until u16(b, cmap + 2)) {
            val platform = u16(b, cmap + 4 + 8 * k); val encoding = u16(b, cmap + 6 + 8 * k)
            val at = cmap + b.getInt(cmap + 8 + 8 * k)
            val f = u16(b, at)
            val unicode = platform == 0 || (platform == 3 && (encoding == 1 || encoding == 10))
            if (unicode && (f == 12 || (f == 4 && format != 12))) { table = at; format = f }
        }
        if (format == 4) {
            val segments = u16(b, table + 6) / 2
            val ends = table + 14; val starts = ends + 2 * segments + 2
            val deltas = starts + 2 * segments; val ranges = deltas + 2 * segments
            for (s in 0 until segments) {
                val start = u16(b, starts + 2 * s); val end = minOf(u16(b, ends + 2 * s), 0xFFFE)
                val delta = b.getShort(deltas + 2 * s).toInt(); val range = u16(b, ranges + 2 * s)
                for (char in start..end) {
                    val glyph = if (range == 0) char + delta
                        else u16(b, ranges + 2 * s + range + 2 * (char - start)).let { if (it == 0) 0 else it + delta }
                    put(char, glyph and 0xFFFF)
                }
            }
        } else if (format == 12) {
            for (k in 0 until b.getInt(table + 12)) {
                val group = table + 16 + 12 * k
                val start = b.getInt(group); val first = b.getInt(group + 8)
                val end = minOf(b.getInt(group + 4), start + (out.size - 1 - first))
                for (char in start..end) put(char, first + (char - start))
            }
        }
        return out
    }
}
