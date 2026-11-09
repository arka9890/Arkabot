plugins {
    id("com.android.application")
    id("org.jetbrains.kotlin.android")
    id("org.jetbrains.kotlin.plugin.compose")
}

val projectRoot = rootProject.projectDir

// Rust core: build the ffi crate for every Android ABI with cargo-ndk and
// copy the .so files into jniLibs. Also generates the uniffi Kotlin bindings.
tasks.register<Exec>("rustBuild") {
    commandLine("bash", projectRoot.absolutePath + "/scripts/build-rust-android.sh")
    environment("ANDROID_NDK_HOME", System.getenv("ANDROID_NDK_HOME") ?: "${System.getenv("ANDROID_HOME")}/ndk/27.0.12077973")
}

tasks.register<Exec>("uniffiKotlin") {
    commandLine("bash", projectRoot.absolutePath + "/scripts/gen-kotlin.sh")
}

android {
    namespace = "com.arkabot.android"
    compileSdk = 35

    defaultConfig {
        applicationId = "com.arkabot.android"
        minSdk = 26
        targetSdk = 35
        versionCode = 1
        versionName = "0.1.0"
    }

    signingConfigs {
        if (System.getenv("ANDROID_KEYSTORE_FILE")?.isNotEmpty() == true) {
            create("release") {
                storeFile = file(System.getenv("ANDROID_KEYSTORE_FILE"))
                storePassword = System.getenv("ANDROID_KEYSTORE_PASSWORD")
                keyAlias = System.getenv("ANDROID_KEY_ALIAS")
                keyPassword = System.getenv("ANDROID_KEY_PASSWORD")
            }
        }
    }

    buildTypes {
        release {
            isMinifyEnabled = false
            signingConfig = signingConfigs.findByName("release")
        }
    }

    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_17
        targetCompatibility = JavaVersion.VERSION_17
    }
    kotlinOptions { jvmTarget = "17" }
    buildFeatures { compose = true }

    sourceSets["main"].kotlin.srcDir("src/main/rust")
    sourceSets["main"].jniLibs.srcDir("src/main/jniLibs")

    ndkVersion = "27.0.12077973"
}

dependencies {
    implementation(platform("androidx.compose:compose-bom:2025.02.00"))
    implementation("androidx.core:core-ktx:1.15.0")
    implementation("androidx.activity:activity-compose:1.9.3")
    implementation("androidx.lifecycle:lifecycle-runtime-ktx:2.8.7")
    implementation("androidx.lifecycle:lifecycle-viewmodel-compose:2.8.7")
    implementation("androidx.navigation:navigation-compose:2.8.5")
    implementation("androidx.compose.ui:ui")
    implementation("androidx.compose.ui:ui-graphics")
    implementation("androidx.compose.ui:ui-tooling-preview")
    implementation("androidx.compose.material3:material3")
    implementation("androidx.compose.material:material-icons-extended")
    implementation("org.jetbrains.kotlinx:kotlinx-coroutines-android:1.9.0")
    // uniffi Kotlin bindings talk to the Rust library through JNA
    implementation("net.java.dev.jna:jna:5.14.0")
    debugImplementation("androidx.compose.ui:ui-tooling")
}

tasks.named("preBuild") {
    dependsOn("uniffiKotlin")
    dependsOn("rustBuild")
}
