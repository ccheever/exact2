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

// Each app builds into a directory of its own: two apps' libraries, assets and APKs never mix.
layout.buildDirectory = layout.projectDirectory.dir("build/$app")

android {
    namespace = "dev.exact.app"
    compileSdk { version = release(37) { minorApiLevel = 2 } }
    defaultConfig {
        // An Android package name's segments are Java identifiers: a manifest id's `-` becomes `_`.
        applicationId = (identity["id"] as String).replace('-', '_')
        minSdk = 31
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
            signingConfig = signingConfigs.getByName("debug")
        }
    }
    // The app's Rust library (as libexactcanvas.so) and its assets (the reader copies `app/` out of the APK once).
    sourceSets["main"].jniLibs.directories.add("build/$app/rust/jniLibs")
    sourceSets["main"].assets.directories.add("build/$app/rust/assets")
    sourceSets["main"].kotlin.directories.add(if (compose) "src/compose/java" else "src/nocompose/java")
    // -PmodulesDir lets a component fixture compile the app's actual provider, without copying its Kotlin.
    val modulesDir = providers.gradleProperty("modulesDir").orNull?.let { file(it) } ?: appDir.resolve("modules/android")
    modulesDir.takeIf { it.isDirectory }?.let { sourceSets["main"].kotlin.directories.add(it.path) }
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
        implementation(platform("androidx.compose:compose-bom:2026.09.00"))
        implementation("androidx.compose.material3:material3")
    }
}
