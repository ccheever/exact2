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

// Each app builds into a directory of its own: two apps' libraries, assets and APKs never mix.
layout.buildDirectory = layout.projectDirectory.dir("build/$app")

android {
    namespace = "dev.exact.app"
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
    sourceSets["main"].jniLibs.directories.add("build/$app/rust/jniLibs")
    sourceSets["main"].assets.directories.add("build/$app/rust/assets")
    sourceSets["main"].kotlin.directories.add(if (compose) "src/compose/java" else "src/nocompose/java")
    // The app's Kotlin hatches (LLP 1115.001.000 D4): `class exact.app.Hatches` in its `modules/android`.
    appDir.resolve("modules/android").takeIf { it.isDirectory }?.let { sourceSets["main"].kotlin.directories.add(it.path) }
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
dependencies {
    implementation(project(":canvas"))
    if (compose) {
        implementation(platform(libs.compose.bom))
        implementation(libs.compose.material3)
    }
}
