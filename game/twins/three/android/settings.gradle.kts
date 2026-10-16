// A full-screen system WebView over the three.js twin, for measuring it on a phone (game/bench/phone.mjs).
// Built with the reader's wrapper: host/android/gradlew -p game/twins/three/android :app:assembleRelease
pluginManagement { repositories { google(); mavenCentral(); gradlePluginPortal() } }
dependencyResolutionManagement {
    repositoriesMode.set(RepositoriesMode.FAIL_ON_PROJECT_REPOS)
    repositories { google(); mavenCentral() }
}
rootProject.name = "three-twin-webview"
include(":app")
