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
if (providers.gradleProperty("app").isPresent) include(":app")
