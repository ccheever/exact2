plugins { id("com.android.application") }
android {
    namespace = "dev.exact.twin"
    compileSdk { version = release(37) { minorApiLevel = 2 } }
    defaultConfig { applicationId = "dev.exact.twin"; minSdk = 31; targetSdk = 36; versionCode = 1; versionName = "1"; ndk { abiFilters += "arm64-v8a" } }
    buildTypes { release { isMinifyEnabled = false; signingConfig = signingConfigs.getByName("debug") } }
    compileOptions { sourceCompatibility = JavaVersion.VERSION_17; targetCompatibility = JavaVersion.VERSION_17 }
}
