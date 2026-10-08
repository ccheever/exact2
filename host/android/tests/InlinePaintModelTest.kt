package com.exact.android

/** Standalone JVM checks against the CSS/browser cases in textflow/src/collapse.rs. */
fun main() {
    fun collapsed(source: List<String>, mode: Int, expected: List<String>) {
        check(InlinePaintModel.collapse(source, mode) == expected) { "white-space $mode: $source" }
    }
    collapsed(listOf(" a ", "  b\t"), 0, listOf("a ", "b"))
    collapsed(listOf("a\r\n", " b"), 0, listOf("a ", "b"))
    collapsed(listOf("中\n", "文"), 0, listOf("中 ", "文"))
    collapsed(listOf("a\u200b", "\nb"), 0, listOf("a\u200b", "b"))
    collapsed(listOf("a\n", "\u200bb"), 0, listOf("a", "\u200bb"))
    collapsed(listOf("a\u00a0\u00a0b\u000c"), 0, listOf("a\u00a0\u00a0b\u000c"))
    collapsed(listOf("a \n ", " \n b "), 3, listOf("a\n", "\nb"))
    collapsed(listOf("\n a\t \n"), 3, listOf("\na\n"))
    collapsed(listOf("a  ", "\tb\n"), 2, listOf("a ", "b"))
    for (mode in listOf(1, 4)) collapsed(listOf(" \na\t", " b "), mode, listOf(" \na\t", " b "))
    val red = InlinePaintColor(0xffff0000.toInt(), 0xff00ff00.toInt())
    val model = InlinePaintModel(listOf(
        InlinePaintPiece(" A😀 ", red, listOf(InlinePaintColor(0xff0000ff.toInt()), InlinePaintColor(0x80ff0000.toInt())), 1, false),
        InlinePaintPiece(" B ", null, emptyList(), 0, true)
    ))
    val light = model.ranges(0, false, 2)
    check(light.text == "A😀 B")
    check(light.ranges[0] == InlinePaintRange(0, 4, 0xffff0000.toInt(), 0xff80007f.toInt(), 3))
    check(light.ranges[1] == InlinePaintRange(4, 5, 0, 0, 0))
    check(model.ranges(0, true, 0).ranges[0].color == 0xff00ff00.toInt())
    check(InlinePaintModel.decoration("none") == 0)
    check(InlinePaintModel.decoration("underline line-through") == 3)
    check(runCatching { InlinePaintModel.decoration("overline") }.isFailure)
    println("InlinePaintModelTest: PASS (CSS collapse, run boundaries, UTF-16, scheme, layered background, visibility, decoration)")
}
