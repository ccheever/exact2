// The app shell: one activity over the reader, packaging the app `-Papp=<name>` names (apps/<name>, or the directory
// `-PappDir=` gives). The package id and the label are the app's own manifest's (app.json `app.id`, `app.name`).
// `bun scripts/agent-android.mjs build <name> --apk` builds the app's library (apps/<name>/android), puts it and the
// app's assets under this module's build directory, and runs `:app:assembleRelease`.
import groovy.json.JsonSlurper

plugins { id("com.android.application") }

val app = providers.gradleProperty("app").get()
val appDir = providers.gradleProperty("appDir").orNull?.let { file(it) } ?: rootDir.resolve("../../apps/$app")
val identity = (JsonSlurper().parse(appDir.resolve("app.json")) as Map<*, *>)["app"] as Map<*, *>

// Each app builds into a directory of its own: two apps' libraries, assets and APKs never mix.
layout.buildDirectory = layout.projectDirectory.dir("build/$app")

android {
    namespace = "dev.exact.app"
    compileSdk { version = release(37) { minorApiLevel = 2 } }
    defaultConfig {
        applicationId = identity["id"] as String
        minSdk = 31
        targetSdk = 36
        versionCode = 1
        // -PidSuffix=.dev: a side-by-side build (another package) that leaves the installed one alone.
        (project.findProperty("idSuffix") as String?)?.let { applicationIdSuffix = it }
        versionName = "1"
        ndk { abiFilters += "arm64-v8a" }
        manifestPlaceholders["appName"] = identity["name"] as String
    }
    buildTypes {
        release {
            isMinifyEnabled = true
            proguardFiles(getDefaultProguardFile("proguard-android-optimize.txt"))
            signingConfig = signingConfigs.getByName("debug")
        }
    }
    // The app's Rust library (as libexactcanvas.so) and its assets (the reader copies `app/` out of the APK once).
    sourceSets["main"].jniLibs.directories.add("build/$app/rust/jniLibs")
    sourceSets["main"].assets.directories.add("build/$app/rust/assets")
    packaging { jniLibs { useLegacyPackaging = false } }
    compileOptions { sourceCompatibility = JavaVersion.VERSION_17; targetCompatibility = JavaVersion.VERSION_17 }
}
dependencies { implementation(project(":canvas")) }
