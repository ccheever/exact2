// D8's JSON Schema subset on Android (LLP 1133.016): the checks every target shares (web/schema.js and
// LanguageModelSchema.swift say the same), and the validation Android's structured output is (D8: ML Kit takes a
// schema only from KSP-generated classes, so the schema goes into the prompt and the answer is checked here).
//
// An object's keys keep the order org.json read them in; an instance is written in the order of its schema's
// `required`, which the bake writes in the shape's order.
package exact.languagemodel

import org.json.JSONArray
import org.json.JSONObject
import org.json.JSONTokener

object LanguageModelSchema {
    private val KEYS = mapOf(
        "object" to listOf("properties", "required"), "string" to listOf("enum", "pattern"),
        "number" to listOf("minimum", "maximum"), "integer" to listOf("minimum", "maximum"), "boolean" to emptyList(),
        "array" to listOf("items", "minItems", "maxItems"),
    )
    private val ANY = listOf("type", "description", "\$defs", "\$schema", "title")

    fun number(v: Any?): Double? = if (v is Number) v.toDouble() else null

    private fun resolve(ref: Any?, root: JSONObject): JSONObject? {
        val r = ref as? String ?: return null
        if (!r.startsWith("#/\$defs/")) return null
        return root.optJSONObject("\$defs")?.optJSONObject(r.removePrefix("#/\$defs/"))
    }

    private fun keys(o: JSONObject): List<String> = o.keys().asSequence().toList()

    /** Why `schema` is outside the subset, or null. */
    fun refusal(schema: Any?, root: JSONObject? = null, at: String = ""): String? {
        val place = at.ifEmpty { "the schema" }
        val s = schema as? JSONObject ?: return "$place is not an object"
        val top = root ?: s
        if (s.has("\$ref")) return if (resolve(s.opt("\$ref"), top) != null) null else "$at: `${s.opt("\$ref")}` names nothing in `\$defs`"
        val type = s.optString("type")
        val allowed = KEYS[type] ?: return "$place: type `${type.ifEmpty { "none" }}` is outside D8's subset"
        for (k in keys(s).sorted()) if (k !in allowed && k !in ANY) return "$place: `$k` is outside D8's subset"
        s.optJSONObject("\$defs")?.let { defs -> for (name in keys(defs).sorted()) refusal(defs.opt(name), top, "\$defs.$name")?.let { return it } }
        if (type == "object") s.optJSONObject("properties")?.let { p ->
            for (name in keys(p).sorted()) refusal(p.opt(name), top, if (at.isEmpty()) name else "$at.$name")?.let { return it }
        }
        if (type == "array" && s.has("items")) return refusal(s.opt("items"), top, "$at[]")
        return null
    }

    /** Why `v` does not satisfy `schema`, or null. */
    fun violation(schema: JSONObject, v: Any?, root: JSONObject? = null, at: String = ""): String? {
        val top = root ?: schema
        if (schema.has("\$ref")) return violation(resolve(schema.opt("\$ref"), top) ?: JSONObject(), v, top, at)
        val place = at.ifEmpty { "the value" }
        when (val type = schema.optString("type")) {
            "object" -> {
                val o = v as? JSONObject ?: return "$place is not an object"
                schema.optJSONArray("required")?.let { r -> for (i in 0 until r.length()) if (!o.has(r.getString(i))) return "$place has no `${r.getString(i)}`" }
                val properties = schema.optJSONObject("properties") ?: JSONObject()
                for (k in keys(o).sorted()) {
                    val p = properties.optJSONObject(k) ?: return "$place has `$k`, which the schema does not"
                    violation(p, o.opt(k), top, if (at.isEmpty()) k else "$at.$k")?.let { return it }
                }
                return null
            }
            "string" -> {
                val s = v as? String ?: return "$place is not a string"
                schema.optJSONArray("enum")?.let { e ->
                    val choices = (0 until e.length()).map { e.getString(it) }
                    if (s !in choices) return "$place is not one of ${choices.joinToString(", ")}"
                }
                schema.optString("pattern").takeIf { it.isNotEmpty() }?.let { if (!Regex(it).containsMatchIn(s)) return "$place does not match $it" }
                return null
            }
            "number", "integer" -> {
                val integer = type == "integer"
                val n = number(v)
                if (n == null || !n.isFinite() || (integer && n != Math.rint(n))) return "$place is not ${if (integer) "an integer" else "a number"}"
                number(schema.opt("minimum"))?.let { if (n < it) return "$place is under ${text(it)}" }
                number(schema.opt("maximum"))?.let { if (n > it) return "$place is over ${text(it)}" }
                return null
            }
            "boolean" -> return if (v is Boolean) null else "$place is not a bool"
            "array" -> {
                val a = v as? JSONArray ?: return "$place is not an array"
                number(schema.opt("minItems"))?.let { if (a.length() < it) return "$place has fewer than ${text(it)} items" }
                number(schema.opt("maxItems"))?.let { if (a.length() > it) return "$place has more than ${text(it)} items" }
                schema.optJSONObject("items")?.let { items -> for (i in 0 until a.length()) violation(items, a.opt(i), top, "$place[$i]")?.let { return it } }
                return null
            }
            else -> return null
        }
    }

    /** §9's first instance as JSON text: each required property in order, a string's first `enum` value or `""`, a
     * number's `minimum` or 0, `false`, and an array of `minItems` first instances. */
    fun firstInstance(schema: JSONObject, root: JSONObject? = null): String {
        val top = root ?: schema
        if (schema.has("\$ref")) return firstInstance(resolve(schema.opt("\$ref"), top) ?: JSONObject(), top)
        return when (schema.optString("type")) {
            "object" -> {
                val properties = schema.optJSONObject("properties") ?: JSONObject()
                val required = schema.optJSONArray("required") ?: JSONArray()
                (0 until required.length()).map { required.getString(it) }.mapNotNull { k ->
                    properties.optJSONObject(k)?.let { "${JSONObject.quote(k)}:${firstInstance(it, top)}" }
                }.joinToString(",", "{", "}")
            }
            "string" -> JSONObject.quote(schema.optJSONArray("enum")?.optString(0) ?: "")
            "number", "integer" -> number(schema.opt("minimum"))?.let(::text) ?: "0"
            "boolean" -> "false"
            "array" -> {
                val n = number(schema.opt("minItems"))?.toInt() ?: 0
                val item = firstInstance(schema.optJSONObject("items") ?: JSONObject(), top)
                List(maxOf(0, n)) { item }.joinToString(",", "[", "]")
            }
            else -> "null"
        }
    }

    /** A number as JavaScript writes it: no `.0` on an integer. */
    fun text(n: Double): String = if (n == Math.rint(n) && Math.abs(n) < 1e15) n.toLong().toString() else n.toString()

    /** `v` as JSON text, slashes unescaped, as JavaScript writes it. */
    fun json(v: Any?): String = when (v) {
        null, JSONObject.NULL -> "null"
        is String -> JSONObject.quote(v).replace("\\/", "/")
        is JSONObject, is JSONArray -> v.toString().replace("\\/", "/")
        is Number -> number(v)?.let(::text) ?: "null"
        else -> v.toString()
    }

    /** JSON text's value, or null when it is not JSON. */
    fun parse(text: String): Any? = try {
        // org.json reads a bare word as a string and stops at the value's end: neither is JSON.
        val t = JSONTokener(text)
        t.nextValue().takeIf { (it !is String || text.trimStart().startsWith("\"")) && t.nextClean() == 0.toChar() }
    } catch (e: Exception) { null }
}
