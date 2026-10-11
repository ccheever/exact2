plugins { id("com.android.library") }
android {
    namespace = "dev.exact.bench.exactcanvas"
    compileSdk { version = release(37) { minorApiLevel = 2 } }
    defaultConfig { minSdk = 31; consumerProguardFiles("proguard-rules.pro") }
    compileOptions { sourceCompatibility = JavaVersion.VERSION_17; targetCompatibility = JavaVersion.VERSION_17 }
}
dependencies {
    api(project(":bench"))
}
