package com.exact.android

import android.content.Context
import android.system.Os
import java.io.File

/** Platform directories for Rust owners inside Android's actual app sandbox. */
internal object NativeEnvironment {
    fun configure(context: Context) {
        val app = context.applicationContext
        // Android does not populate the native process HOME. Supply its real
        // sandbox for consumers of that existing host protocol, without replacing
        // an explicit launch environment or changing the desktop build environment.
        if (Os.getenv("HOME").isNullOrEmpty()) Os.setenv("HOME", app.dataDir.absolutePath, true)
        for ((name, path) in listOf("XDG_DATA_HOME" to app.filesDir, "XDG_CACHE_HOME" to app.cacheDir)) {
            require(path.isAbsolute) { "Android app directory must be absolute" }
            Os.setenv(name, path.absolutePath, true)
        }
        require(File(checkNotNull(Os.getenv("HOME"))).isAbsolute) { "Native app HOME must be absolute" }
    }
}
