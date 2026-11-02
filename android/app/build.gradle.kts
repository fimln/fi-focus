import java.util.Properties

plugins { id("com.android.application"); id("org.jetbrains.kotlin.android") }
val signingProperties = Properties().apply {
 val propertiesFile = rootProject.file("keystore.properties")
 if (propertiesFile.exists()) { propertiesFile.inputStream().use { load(it) } }
}
if (signingProperties.isNotEmpty()) {
 require(listOf("storeFile", "storePassword", "keyAlias", "keyPassword").all { !signingProperties.getProperty(it).isNullOrBlank() }) { "Lengkapi android/keystore.properties sesuai keystore.properties.example" }
}
android {
 compileSdk = 37
 namespace = "com.fi.focus"
 defaultConfig { applicationId = "com.fi.focus"; minSdk = 26; targetSdk = 37; versionCode = 1; versionName = "0.1.0"; ndk { abiFilters += "arm64-v8a" } }
 signingConfigs { if (signingProperties.isNotEmpty()) { create("release") { storeFile = file(signingProperties.getProperty("storeFile")); storePassword = signingProperties.getProperty("storePassword"); keyAlias = signingProperties.getProperty("keyAlias"); keyPassword = signingProperties.getProperty("keyPassword") } } }
 buildTypes { getByName("release") { signingConfig = signingConfigs.findByName("release"); isMinifyEnabled = true; isShrinkResources = true; proguardFiles(getDefaultProguardFile("proguard-android-optimize.txt"), "proguard-rules.pro") } }
 packaging { jniLibs { useLegacyPackaging = true } }
 compileOptions { sourceCompatibility = JavaVersion.VERSION_17; targetCompatibility = JavaVersion.VERSION_17 }
}
