// The Material reference (LLP 1115.001.001 D3, D4): each specimen of `specimens.json` drawn by Compose Material 3 on
// Robolectric's native graphics, at the pinned SDK and the Pixel's density, with what the oracle compares beside each
// picture. Nothing links this module; `./gradlew :reference:draw -Preference.out=<dir>` runs it, and
// `./gradlew :reference:assembleDebug` builds the catalogue a phone shows (`Catalogue.kt`).
import org.gradle.api.artifacts.component.ModuleComponentIdentifier

plugins { id("com.android.application") }
apply(plugin = "org.jetbrains.kotlin.plugin.compose")

android {
    namespace = "dev.exact.reference"
    compileSdk { version = release(37) { minorApiLevel = 2 } }
    defaultConfig { applicationId = "dev.exact.reference"; minSdk = 31; targetSdk = libs.versions.referenceSdk.get().toInt() }
    buildFeatures { compose = true }
    testOptions { unitTests { isIncludeAndroidResources = true } }
    compileOptions { sourceCompatibility = JavaVersion.VERSION_17; targetCompatibility = JavaVersion.VERSION_17 }
}

dependencies {
    implementation(platform(libs.compose.bom))
    implementation(libs.compose.material3)
    implementation(libs.compose.icons)
    implementation(libs.compose.activity)
    testImplementation(platform(libs.compose.bom))
    testImplementation(libs.compose.test.junit4)
    debugImplementation(libs.compose.test.manifest)
    testImplementation(libs.junit)
    testImplementation(libs.robolectric)
    testImplementation(libs.roborazzi)
    testImplementation(libs.roborazzi.compose)
}

// The BOM decides `material3`; the catalogue names the version it decides (D1), and this says when they part.
val checkPins by tasks.registering {
    val classpath = configurations.named("debugRuntimeClasspath")
    val pinned = libs.versions.material3.get()
    doLast {
        val resolved = classpath.get().incoming.resolutionResult.allComponents
            .mapNotNull { it.id as? ModuleComponentIdentifier }
            .filter { it.group == "androidx.compose.material3" && it.module == "material3" }
            .map { it.version }.distinct()
        if (resolved != listOf(pinned)) {
            throw GradleException("libs.versions.toml pins material3 $pinned; the BOM resolves $resolved")
        }
    }
}

val out = providers.gradleProperty("reference.out").orElse(layout.buildDirectory.dir("reference").map { it.asFile.path })
tasks.withType<Test>().configureEach {
    dependsOn(checkPins)
    // Robolectric's sandbox for SDK 36 reads the JDK's file descriptors through this package (Java 21).
    jvmArgs("--add-exports=java.base/jdk.internal.access=ALL-UNNAMED", "--add-opens=java.base/jdk.internal.access=ALL-UNNAMED")
    systemProperty("roborazzi.test.record", "true")
    systemProperty("reference.specimens", file("specimens.json").path)
    systemProperty("reference.sdk", libs.versions.referenceSdk.get())
    systemProperty("reference.out", out.get())
    outputs.upToDateWhen { false }
}
tasks.register("draw") { dependsOn("testDebugUnitTest") }
