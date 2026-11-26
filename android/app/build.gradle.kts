import java.util.Properties

plugins {
    id("com.android.application")
    id("org.jetbrains.kotlin.android")
}

android {
    namespace = "app.syncroapp.umbral"
    compileSdk = 36

    defaultConfig {
        applicationId = "app.syncroapp.umbral"
        minSdk = 26
        targetSdk = 36
        versionCode = 14
        versionName = "0.7.8"
    }

    // La clave de firma NO está en el repositorio: cada quien genera la suya (ver docs/INSTALACION.md) y la describe en
    // android/keystore.properties (storeFile, storePassword, keyAlias, keyPassword). Sin ese archivo el release sale sin firmar.
    val ksFile = rootProject.file("keystore.properties")
    val ks = Properties().apply { if (ksFile.exists()) ksFile.inputStream().use { load(it) } }
    signingConfigs {
        if (ks.isNotEmpty()) {
            create("release") {
                storeFile = file(ks.getProperty("storeFile"))
                storePassword = ks.getProperty("storePassword")
                keyAlias = ks.getProperty("keyAlias")
                keyPassword = ks.getProperty("keyPassword")
            }
        }
    }

    buildTypes {
        debug {
            // Se instala junto a la versión de producción (otra app, otra vinculación): probar no rompe la que usas a diario.
            applicationIdSuffix = ".debug"
            versionNameSuffix = "-debug"
        }
        release {
            // Versión de producción: no depurable (nadie puede leer sus datos con adb run-as), código reducido y ofuscado.
            isDebuggable = false
            isMinifyEnabled = true
            isShrinkResources = true
            proguardFiles(getDefaultProguardFile("proguard-android-optimize.txt"), "proguard-rules.pro")
            signingConfig = signingConfigs.findByName("release")
        }
    }
    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_17
        targetCompatibility = JavaVersion.VERSION_17
    }
    kotlinOptions {
        jvmTarget = "17"
    }
}

dependencies {
    implementation("androidx.core:core-ktx:1.15.0")
    implementation("com.squareup.okhttp3:okhttp:4.12.0")
    implementation("com.google.android.gms:play-services-code-scanner:16.1.0")
}
