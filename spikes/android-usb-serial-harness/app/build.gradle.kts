plugins {
    id("com.android.application")
    id("org.jetbrains.kotlin.android")
    id("org.jetbrains.kotlin.plugin.compose")
}

android {
    namespace = "com.autonomousdroneexpert.m1c"
    compileSdk = 35

    defaultConfig {
        applicationId = "com.autonomousdroneexpert.m1c"
        minSdk = 24
        targetSdk = 35
        versionCode = 1
        versionName = "0.1.0-spike"

        // Source commit is injected by CI (-PsourceSha=<full sha>); "local-dev" otherwise.
        val sourceSha = (project.findProperty("sourceSha") as String?) ?: "local-dev"
        buildConfigField("String", "SOURCE_SHA", "\"$sourceSha\"")
        buildConfigField(
            "String",
            "SPIKE_LABEL",
            "\"SPIKE — REQUIRES HARDWARE TEST — DO NOT USE FOR FLIGHT CONFIGURATION\""
        )
    }

    buildTypes {
        // Debug review build only. No release signing, no signing config, no secrets.
        getByName("debug") {
            // Debug review build. No release signing config, no secrets. Application ID is
            // kept exactly as specified (no suffix) so BUILD-INFO reports it unambiguously.
            isMinifyEnabled = false
        }
    }

    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_17
        targetCompatibility = JavaVersion.VERSION_17
    }
    kotlinOptions {
        jvmTarget = "17"
    }
    buildFeatures {
        compose = true
        buildConfig = true
    }
}

dependencies {
    val composeBom = platform("androidx.compose:compose-bom:2024.10.01")
    implementation(composeBom)
    implementation("androidx.core:core-ktx:1.13.1")
    implementation("androidx.activity:activity-compose:1.9.3")
    implementation("androidx.lifecycle:lifecycle-runtime-ktx:2.8.7")
    implementation("androidx.lifecycle:lifecycle-viewmodel-compose:2.8.7")
    implementation("androidx.compose.ui:ui")
    implementation("androidx.compose.ui:ui-graphics")
    implementation("androidx.compose.ui:ui-tooling-preview")
    implementation("androidx.compose.material3:material3")
    implementation("org.jetbrains.kotlinx:kotlinx-coroutines-android:1.9.0")

    debugImplementation("androidx.compose.ui:ui-tooling")

    testImplementation("junit:junit:4.13.2")
    testImplementation("org.jetbrains.kotlinx:kotlinx-coroutines-test:1.9.0")
}
