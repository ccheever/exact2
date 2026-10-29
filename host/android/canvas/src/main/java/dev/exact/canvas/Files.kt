package dev.exact.canvas

import android.content.ContentProvider
import android.content.ContentValues
import android.content.Context
import android.database.Cursor
import android.database.MatrixCursor
import android.net.Uri
import android.os.ParcelFileDescriptor
import android.provider.OpenableColumns
import android.webkit.MimeTypeMap
import java.io.File
import java.io.FileNotFoundException

/**
 * The app's files as content URIs another app may read under a grant (LLP 1133.024 D22 item 2): what an intent carries
 * for a mail's attachments, as `androidx.core`'s `FileProvider` does, without the library. Unexported, so only a URI
 * the app granted with its intent opens, and only for reading; a path outside the app's own data directory opens
 * nothing. [uri] makes one; `AndroidManifest.xml` declares it as `<package>.exact.files`.
 */
class Files : ContentProvider() {
    override fun onCreate() = true

    private fun file(uri: Uri): File {
        val context = context ?: throw FileNotFoundException()
        val f = File(uri.path ?: "").canonicalFile
        if (!inside(context, f) || !f.isFile) throw FileNotFoundException()
        return f
    }

    override fun openFile(uri: Uri, mode: String): ParcelFileDescriptor {
        if (mode != "r") throw SecurityException("the app's files are shared for reading only")
        return ParcelFileDescriptor.open(file(uri), ParcelFileDescriptor.MODE_READ_ONLY)
    }

    /** The name and size a receiver shows ([OpenableColumns]). */
    override fun query(uri: Uri, projection: Array<out String>?, selection: String?, args: Array<out String>?, order: String?): Cursor {
        val f = file(uri)
        val columns = projection?.filter { it == OpenableColumns.DISPLAY_NAME || it == OpenableColumns.SIZE }?.toTypedArray()
            ?: arrayOf(OpenableColumns.DISPLAY_NAME, OpenableColumns.SIZE)
        return MatrixCursor(columns, 1).apply { addRow(columns.map { if (it == OpenableColumns.SIZE) f.length() else f.name }) }
    }

    override fun getType(uri: Uri): String = type(File(uri.path ?: ""))

    override fun insert(uri: Uri, values: ContentValues?): Uri? = throw UnsupportedOperationException()
    override fun update(uri: Uri, values: ContentValues?, selection: String?, args: Array<out String>?): Int = throw UnsupportedOperationException()
    override fun delete(uri: Uri, selection: String?, args: Array<out String>?): Int = throw UnsupportedOperationException()

    companion object {
        private fun inside(context: Context, f: File): Boolean = f.path.startsWith(context.dataDir.canonicalPath + File.separator)

        /** The content URI for [f], one of the app's own files, or null for any other. */
        fun uri(context: Context, f: File): Uri? {
            val canonical = f.canonicalFile
            if (!inside(context, canonical) || !canonical.isFile) return null
            return Uri.Builder().scheme("content").authority("${context.packageName}.exact.files").path(canonical.path).build()
        }

        /** A file's type by its extension, as a receiver reads it. */
        fun type(f: File): String = MimeTypeMap.getSingleton().getMimeTypeFromExtension(f.extension.lowercase()) ?: "application/octet-stream"
    }
}
