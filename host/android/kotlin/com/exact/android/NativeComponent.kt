package com.exact.android

import android.content.Context
import android.view.View
import org.json.JSONObject

/** An app-owned platform widget. Callbacks and lifetime stay on the UI thread. */
interface NativeComponent : AutoCloseable {
    val view: View
    fun update(props: JSONObject)
    override fun close() {}
}

/** Resolve the existing nativeViewModuleName/nativeViewProps Contract seam. */
fun interface NativeViewFactory {
    fun create(context: Context, name: String, props: JSONObject,
        message: (String) -> Unit, intrinsic: (Float, Float) -> Unit): NativeComponent
}
