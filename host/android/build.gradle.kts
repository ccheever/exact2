buildscript {
    // AGP 9's built-in Kotlin is 2.2; the reader is compiled with 2.4, as the benchmark that measured it compiles it.
    dependencies {
        classpath("org.jetbrains.kotlin:kotlin-gradle-plugin:2.4.10")
        // Applied only to an app whose plan has a date or time control (app/build.gradle.kts, `-Pcompose=1`).
        classpath("org.jetbrains.kotlin:compose-compiler-gradle-plugin:2.4.10")
    }
}
plugins {
    id("com.android.application") version "9.4.1" apply false
    id("com.android.library") version "9.4.1" apply false
}
