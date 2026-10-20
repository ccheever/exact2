pluginManagement {
    repositories { google(); mavenCentral(); gradlePluginPortal() }
}
dependencyResolutionManagement {
    repositoriesMode.set(RepositoriesMode.FAIL_ON_PROJECT_REPOS)
    repositories { google(); mavenCentral() }
}
rootProject.name = "exact-android"
// The reader (a library), and the shell that packages one app with it: `-Papp=<name>` names the app (app/build.gradle.kts).
include(":canvas")
// The Material reference of LLP 1115.001.001: Compose Material 3 drawn on the JVM, in no app.
include(":reference")
if (providers.gradleProperty("app").isPresent) include(":app")
