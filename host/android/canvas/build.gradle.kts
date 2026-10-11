plugins { id("com.android.library") }
android {
    namespace = "dev.exact.canvas"
    compileSdk { version = release(37) { minorApiLevel = 2 } }
    defaultConfig { minSdk = 31; consumerProguardFiles("proguard-rules.pro") }
    compileOptions { sourceCompatibility = JavaVersion.VERSION_17; targetCompatibility = JavaVersion.VERSION_17 }
}
