// The app shell: one activity over the reader, packaging the app `-Papp=<name>` names (apps/<name>, or the directory
// `-PappDir=` gives). The package id and the label are the app's own manifest's (app.json `app.id`, `app.name`).
// `bun scripts/agent-android.mjs build <name> --apk` builds the app's library (apps/<name>/android), puts it and the
// app's assets under this module's build directory, and runs `:app:assembleRelease`.
import groovy.json.JsonSlurper

plugins { id("com.android.application") }

// -Pcompose=1: the app's plan has a date or time control, and its picker is Compose Material 3's (LLP 1115.001.000
// D1-D2; `agent-android.mjs` reads the plan's uses). Without it the framework's dialogs show, and Compose is not linked.
val compose = providers.gradleProperty("compose").orNull == "1"
if (compose) apply(plugin = "org.jetbrains.kotlin.plugin.compose")

val app = providers.gradleProperty("app").get()
val appDir = providers.gradleProperty("appDir").orNull?.let { file(it) } ?: rootDir.resolve("../../apps/$app")
val manifest = JsonSlurper().parse(appDir.resolve("app.json")) as Map<*, *>
val identity = manifest["app"] as Map<*, *>
// The manifest's `orientation` as Chromium maps an installed web app's: `-primary` is the device's own way up, a
// bare `portrait` or `landscape` either way up by the sensor. `natural` is resolved in MainActivity.
val screenOrientation = mapOf(
    "any" to "fullUser", "natural" to "nosensor", "portrait" to "sensorPortrait", "portrait-primary" to "portrait",
    "portrait-secondary" to "reversePortrait", "landscape" to "sensorLandscape", "landscape-primary" to "landscape",
    "landscape-secondary" to "reverseLandscape",
)[manifest["orientation"]] ?: "unspecified"
// The manifest's `host.android.minimumOS`, the oldest Android the app runs on, as its API level. Below 12 the APK keeps
// the reader's Android 10 and 11 code (proguard-legacy.pro); otherwise R8 leaves none of it.
val minimumOS = (((manifest["host"] as Map<*, *>?)?.get("android") as Map<*, *>?)?.get("minimumOS") as String?) ?: "12"
val minimumApi = mapOf("10" to 29, "11" to 30, "12" to 31, "13" to 33, "14" to 34, "15" to 35, "16" to 36)[minimumOS]
    ?: throw GradleException("app.json host.android.minimumOS \"$minimumOS\": name an Android release from 10 to 16")

// Each app builds into a directory of its own (`-PappBuildDir`, else build/<app>): two apps' libraries, assets and APKs
// never mix, and two builds at once write nothing in common (agent-android.mjs gives the reader one too, `canvasBuildDir`).
val out = providers.gradleProperty("appBuildDir").orNull?.let { file(it) } ?: projectDir.resolve("build/$app")
layout.buildDirectory = out

android {
    namespace = "dev.exact.app"
    // The NDK the app's library was linked with (`-PndkPath`, agent-android.mjs), whose strip the APK's library goes
    // through; without it AGP looks for its own default NDK and, missing it, packages the library unstripped.
    providers.gradleProperty("ndkPath").orNull?.let { path ->
        ndkPath = path
        ndkVersion = file("$path/source.properties").readLines().first { it.startsWith("Pkg.Revision") }.substringAfter("=").trim()
    }
    compileSdk { version = release(37) { minorApiLevel = 2 } }
    defaultConfig {
        // An Android package name's segments are Java identifiers: a manifest id's `-` becomes `_`.
        applicationId = (identity["id"] as String).replace('-', '_')
        minSdk = minimumApi
        targetSdk = 36
        versionCode = 1
        // -PidSuffix=.dev: a side-by-side build (another package) that leaves the installed one alone.
        (project.findProperty("idSuffix") as String?)?.let { applicationIdSuffix = it }
        versionName = "1"
        ndk { abiFilters += "arm64-v8a" }
        manifestPlaceholders["appName"] = identity["name"] as String
        manifestPlaceholders["screenOrientation"] = screenOrientation
    }
    buildTypes {
        release {
            isMinifyEnabled = true
            proguardFiles(getDefaultProguardFile("proguard-android-optimize.txt"), "proguard-rules.pro")
            if (minimumApi < 31) proguardFiles("proguard-legacy.pro")
            signingConfig = signingConfigs.getByName("debug")
        }
    }
    // The app's Rust library (as libexactcanvas.so) and its assets (the reader copies `app/` out of the APK once).
    sourceSets["main"].jniLibs.directories.add(out.resolve("rust/jniLibs").path)
    sourceSets["main"].assets.directories.add(out.resolve("rust/assets").path)
    sourceSets["main"].kotlin.directories.add(if (compose) "src/compose/java" else "src/nocompose/java")
    // The app's Kotlin hatches and module roster (LLP 1115.001.000 D4) in its `modules/android`; -PmodulesDir lets a
    // component fixture compile the app's actual provider without copying its Kotlin.
    val modulesDir = providers.gradleProperty("modulesDir").orNull?.let { file(it) } ?: appDir.resolve("modules/android")
    modulesDir.takeIf { it.isDirectory }?.let { sourceSets["main"].kotlin.directories.add(it.path) }
    // The modules' implementations the app binds (LLP 1133.001 D5): their Kotlin, and the roster naming them that
    // `agent-android.mjs` writes (`-PextensionDirs`).
    providers.gradleProperty("extensionDirs").orNull?.split(',')?.filter { it.isNotEmpty() }?.forEach { sourceSets["main"].kotlin.directories.add(it) }
    // The runtime permissions the app's grant lines derive (`-Ppermissions`, LLP 1133.002 D6), and the queries and
    // components its packages' manifest fragments declare (`-PmanifestFragment`, checked and written by
    // scripts/android-manifest.mjs, LLP 1133.030 D9, LLP 1133.026 D19 item 3; with the share target's filters on the
    // app's activity, scripts/share-target.mjs, LLP 1133.024 D5), merged into the shell's manifest from the release source set's: an app that grants no device asks
    // for none, and one that uses no such package runs nothing beyond the shell's activity.
    val fragment = providers.gradleProperty("manifestFragment").orNull?.let { file(it).readText() }?.takeIf { it.isNotBlank() }
    // The hardware a grant line uses where present (`-PandroidFeatures`, LLP 1133.037 D17), never required.
    val lineFeatures = providers.gradleProperty("androidFeatures").orNull?.split(',')?.filter { it.isNotBlank() }.orEmpty()
    (providers.gradleProperty("permissions").orNull?.split(',')?.filter { it.isNotEmpty() } ?: emptyList())
        .takeIf { it.isNotEmpty() || fragment != null || lineFeatures.isNotEmpty() }?.let { names ->
        val permissions = layout.buildDirectory.file("permissions/AndroidManifest.xml").get().asFile
        permissions.parentFile.mkdirs()
        // CAMERA implies the back camera and autofocus as required features, which would filter the app from a
        // device without them; each is declared not required (LLP 1133.008 D6).
        val features = (if ("android.permission.CAMERA" in names) listOf("camera.any", "camera", "camera.autofocus", "camera.flash") else emptyList()) + lineFeatures
        // A permission a later API retired names its last level (`<name> maxSdkVersion=32`, LLP 1133.019 D4).
        val permission = { line: String ->
            val (name, last) = line.split(' ').let { it[0] to it.getOrNull(1)?.removePrefix("maxSdkVersion=") }
            "    <uses-permission android:name=\"$name\"${last?.let { " android:maxSdkVersion=\"$it\"" } ?: ""} />\n"
        }
        // The reader's foreground services the lines declare (`-PforegroundServices`, `{kind: {purpose, types}}`, LLP
        // 1133.033 D2), each with every type its lines give it: a type the manifest lacks throws at `startForeground`.
        // The purpose is the notification's title and its channel's name (D4).
        val escape = { t: String -> t.replace("&", "&amp;").replace("<", "&lt;").replace("\"", "&quot;") }
        @Suppress("UNCHECKED_CAST")
        val services = (providers.gradleProperty("foregroundServices").orNull?.let { JsonSlurper().parseText(it) } as Map<String, Map<String, Any?>>?).orEmpty()
        val service = { kind: String, declared: Map<String, Any?> ->
            "        <service android:name=\"dev.exact.canvas.ForegroundService\$${kind.replaceFirstChar { it.uppercase() }}\" android:exported=\"false\"\n" +
                "            android:foregroundServiceType=\"${(declared["types"] as List<*>).joinToString("|")}\">\n" +
                "            <meta-data android:name=\"dev.exact.purpose\" android:value=\"${escape(declared["purpose"]?.toString() ?: "")}\" />\n        </service>\n"
        }
        // The services join the fragment's `<application>` when it has one: a manifest holds one.
        val manifestChildren = { fragment: String?, services: String ->
            when {
                services.isEmpty() -> fragment?.let { "$it\n" } ?: ""
                fragment?.contains("    </application>") == true -> fragment.replace("    </application>", "$services    </application>") + "\n"
                else -> (fragment?.let { "$it\n" } ?: "") + "    <application>\n$services    </application>\n"
            }
        }
        permissions.writeText("<manifest xmlns:android=\"http://schemas.android.com/apk/res/android\" xmlns:tools=\"http://schemas.android.com/tools\">\n" +
            names.joinToString("") { permission(it) } +
            features.joinToString("") { "    <uses-feature android:name=\"android.hardware.$it\" android:required=\"false\" />\n" } +
            manifestChildren(fragment, services.entries.joinToString("") { service(it.key, it.value) }) + "</manifest>\n")
        sourceSets["release"].manifest.srcFile(permissions)
    }
    // The manifest's `shortcuts` as the launcher activity's static shortcuts (LLP 1133.023 D3, D17), in order: each
    // starts the activity with its location as the intent's data, which the reader takes as a link's. Labels and the
    // disabled message are string resources, as `shortcuts.xml` requires; an app with none has an empty list.
    run {
        val res = layout.buildDirectory.dir("shortcuts/res").get().asFile
        val items = (manifest["shortcuts"] as List<*>?).orEmpty().map { it as Map<*, *> }
        val pkg = (identity["id"] as String).replace('-', '_') + ((project.findProperty("idSuffix") as String?) ?: "")
        val attr = { v: String -> v.replace("&", "&amp;").replace("<", "&lt;").replace("\"", "&quot;") }
        // A string resource's own escapes: a backslash, an apostrophe, and `@` or `?` leading.
        val text = { v: String -> attr(v).replace("\\", "\\\\").replace("'", "\\'").replace(Regex("^([@?])"), "\\\\$1") }
        val strings = StringBuilder()
        val shortcuts = StringBuilder()
        items.forEachIndexed { i, item ->
            val name = item["name"] as String
            val disabled = (item["android"] as Map<*, *>?)?.get("disabledMessage") as String?
            strings.append("    <string name=\"exact_shortcut_${i}_short\">${text((item["short_name"] as String?) ?: name)}</string>\n")
            strings.append("    <string name=\"exact_shortcut_${i}_long\">${text(name)}</string>\n")
            disabled?.let { strings.append("    <string name=\"exact_shortcut_${i}_disabled\">${text(it)}</string>\n") }
            shortcuts.append("    <shortcut android:shortcutId=\"manifest-$i\" android:enabled=\"true\"\n" +
                "        android:shortcutShortLabel=\"@string/exact_shortcut_${i}_short\" android:shortcutLongLabel=\"@string/exact_shortcut_${i}_long\"" +
                (disabled?.let { "\n        android:shortcutDisabledMessage=\"@string/exact_shortcut_${i}_disabled\"" } ?: "") + ">\n" +
                "        <intent android:action=\"android.intent.action.VIEW\" android:targetPackage=\"$pkg\"\n" +
                "            android:targetClass=\"dev.exact.app.MainActivity\" android:data=\"${attr(item["url"] as String)}\" />\n" +
                "    </shortcut>\n")
        }
        res.resolve("xml").mkdirs()
        res.resolve("values").mkdirs()
        res.resolve("xml/exact_shortcuts.xml").writeText("<shortcuts xmlns:android=\"http://schemas.android.com/apk/res/android\">\n$shortcuts</shortcuts>\n")
        res.resolve("values/exact_shortcuts.xml").writeText("<resources>\n$strings</resources>\n")
        sourceSets["main"].res.directories.add(res.path)
    }
    // The token names `bun exact.mjs tamber gen kotlin` wrote (LLP 1121 D12; never committed).
    appDir.resolve(".exact/tamber/TamberTokens.kt").takeIf { it.isFile }?.let { sourceSets["main"].kotlin.directories.add(it.parent) }
    if (compose) buildFeatures { this.compose = true }
    // An app's GPU module (-PgpuModule=<file>) is opened by path from the extracted library directory and checked
    // against the digest its bake recorded, so it is extracted and left unstripped.
    val gpuModule = providers.gradleProperty("gpuModule").orNull
    packaging {
        jniLibs {
            useLegacyPackaging = gpuModule != null
            gpuModule?.let { keepDebugSymbols += "**/$it" }
        }
    }
    compileOptions { sourceCompatibility = JavaVersion.VERSION_17; targetCompatibility = JavaVersion.VERSION_17 }
}
// The Maven libraries the bound modules' implementations name (`-Pmaven`, LLP 1133.008 D8 item 5), resolved with the
// reader's own AndroidX in this one graph. The graph is locked to the app's lock file (`-PmavenLock`, beside its other
// locks) and verified against its checksums (`agent-android.mjs` puts the app's verification file in place and passes
// `--dependency-verification=strict`); `--write-locks` there writes both. An app with none resolves as before.
val maven = providers.gradleProperty("maven").orNull?.split(',')?.filter { it.isNotEmpty() }.orEmpty()
if (maven.isNotEmpty()) {
    dependencyLocking {
        lockAllConfigurations()
        lockMode.set(LockMode.STRICT)
        lockFile.set(file(providers.gradleProperty("mavenLock").get()))
    }
}
dependencies {
    maven.forEach { implementation(it) }
    implementation(project(":canvas"))
    if (compose) {
        implementation(platform(libs.compose.bom))
        implementation(libs.compose.material3)
    }
}
