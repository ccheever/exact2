package dev.exact.canvas

import android.app.Activity
import android.content.Intent
import android.content.pm.PackageManager
import android.net.Uri
import android.os.Build
import android.provider.OpenableColumns
import android.util.Log
import org.json.JSONArray
import org.json.JSONObject

/**
 * A share from another app's share sheet (LLP 1133.024 D5): `ACTION_SEND` or `ACTION_SEND_MULTIPLE` on the app's
 * activity, whose filters the build derives from the manifest's `share_target` and whose `dev.exact.share_target`
 * meta-data carries that member. exact2 claims it as every host does (D2, D3): one location, with each file copied into
 * `app:/tmp/shared/` after it, off this thread.
 *
 * [Receive simple data](https://developer.android.com/training/sharing/receive): `EXTRA_SUBJECT` (or `EXTRA_TITLE`)
 * is the title, `EXTRA_TEXT` the text, where a shared link also is, and each `EXTRA_STREAM` a content URI the intent
 * grants this app for as long as the activity that received it lives.
 */
internal object Shares {
    /** Whether [intent] is a share. */
    fun isShare(intent: Intent?): Boolean = intent?.action == Intent.ACTION_SEND || intent?.action == Intent.ACTION_SEND_MULTIPLE

    /**
     * The share [intent] carries as exact2 reads it, `{"target", "share"}`, or null when it is no share or the app
     * declares no target. Each file is a descriptor opened here through the intent's grant and detached for exact2,
     * which copies from it and closes it (`/proc/self/fd/<n>`): opening is a call to the sender's provider, copying
     * is exact2's, after the location, from the descriptor (the path behind it is the sender's storage, which the grant
     * does not open). A stream that cannot be opened is counted as dropped.
     */
    fun request(activity: Activity, intent: Intent): String? {
        if (!isShare(intent)) return null
        val target = try {
            activity.packageManager.getActivityInfo(activity.componentName, PackageManager.GET_META_DATA).metaData?.getString(META)
        } catch (e: PackageManager.NameNotFoundException) { null } ?: return null
        val share = JSONObject()
        (intent.getStringExtra(Intent.EXTRA_SUBJECT) ?: intent.getStringExtra(Intent.EXTRA_TITLE))?.let { share.put("title", it) }
        intent.getCharSequenceExtra(Intent.EXTRA_TEXT)?.toString()?.let { share.put("text", it) }
        val files = JSONArray()
        var dropped = 0
        for (uri in streams(intent)) {
            val opened = try { activity.contentResolver.openFileDescriptor(uri, "r") } catch (e: Exception) { null }
            if (opened == null) { dropped++; Log.w(TAG, "share: a stream could not be opened"); continue }
            // The size from the descriptor: exact2 cannot stat the sender's path (-1 for a pipe, which it then reads).
            val size = opened.statSize
            val file = JSONObject().put("path", "/proc/self/fd/${opened.detachFd()}").put("name", name(activity, uri))
            if (size >= 0) file.put("size", size)
            (activity.contentResolver.getType(uri) ?: intent.type?.takeUnless { it.endsWith("/*") })?.let { file.put("type", it) }
            files.put(file)
        }
        if (files.length() > 0) share.put("files", files)
        if (dropped > 0) {
            val first = JSONObject(target).optJSONObject("params")?.optJSONArray("files")?.optJSONObject(0)?.optString("name")
            if (!first.isNullOrEmpty()) share.put("dropped", JSONObject().put(first, dropped))
        }
        return JSONObject().put("target", JSONObject(target)).put("share", share).toString()
    }

    /** The content URIs a share carries: `EXTRA_STREAM`, one or a list. */
    private fun streams(intent: Intent): List<Uri> = when (intent.action) {
        Intent.ACTION_SEND_MULTIPLE -> (if (Build.VERSION.SDK_INT >= 33) intent.getParcelableArrayListExtra(Intent.EXTRA_STREAM, Uri::class.java)
            else @Suppress("DEPRECATION") intent.getParcelableArrayListExtra(Intent.EXTRA_STREAM)).orEmpty()
        else -> listOfNotNull(if (Build.VERSION.SDK_INT >= 33) intent.getParcelableExtra(Intent.EXTRA_STREAM, Uri::class.java)
            else @Suppress("DEPRECATION") intent.getParcelableExtra(Intent.EXTRA_STREAM))
    }

    /** The sender's name for a stream, which exact2 keeps only as `Picked.name`; else its URI's last segment. */
    private fun name(activity: Activity, uri: Uri): String {
        try {
            activity.contentResolver.query(uri, arrayOf(OpenableColumns.DISPLAY_NAME), null, null, null)?.use { c ->
                if (c.moveToFirst() && !c.isNull(0)) return c.getString(0)
            }
        } catch (e: Exception) {}
        return uri.lastPathSegment ?: "shared"
    }

    private const val META = "dev.exact.share_target"
    private const val TAG = "exact"
}
