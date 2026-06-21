plugins {
    id("com.android.application")
}

fun cargoPackageVersion(): String {
    val cargoToml = rootProject.projectDir.parentFile.resolve("Cargo.toml")
    var inPackage = false
    for (rawLine in cargoToml.readLines()) {
        val line = rawLine.substringBefore("#").trim()
        if (line.isEmpty()) {
            continue
        }
        if (line == "[package]") {
            inPackage = true
            continue
        }
        if (line.startsWith("[") && line.endsWith("]")) {
            inPackage = false
            continue
        }
        if (!inPackage) {
            continue
        }
        val parts = line.split("=", limit = 2)
        if (parts.size == 2 && parts[0].trim() == "version") {
            return parts[1].trim().trim('"', '\'')
        }
    }
    error("Cargo package version not found in ${cargoToml.absolutePath}")
}

fun cargoVersionCode(version: String): Int {
    val numbers = version.substringBefore("-").substringBefore("+").split(".")
    val major = numbers.getOrNull(0)?.toIntOrNull() ?: 0
    val minor = numbers.getOrNull(1)?.toIntOrNull() ?: 0
    val patch = numbers.getOrNull(2)?.toIntOrNull() ?: 0
    return (major * 10000 + minor * 100 + patch).coerceAtLeast(1)
}

val cargoVersion = cargoPackageVersion()

android {
    namespace = "net.mercen.desktopfushi"
    compileSdk = 35

    defaultConfig {
        applicationId = "net.mercen.desktopfushi"
        minSdk = 26
        targetSdk = 35
        versionCode = cargoVersionCode(cargoVersion)
        versionName = cargoVersion
        resValue("string", "app_version_name", cargoVersion)
        ndk {
            abiFilters += listOf("arm64-v8a", "armeabi-v7a", "x86_64")
        }
    }

    sourceSets {
        getByName("main") {
            jniLibs.srcDir("src/main/jniLibs")
        }
    }

    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_17
        targetCompatibility = JavaVersion.VERSION_17
    }
}

val requestedReleaseBuild = gradle.startParameter.taskNames.any {
    it.contains("Release", ignoreCase = true)
}
val defaultRustProfile = if (requestedReleaseBuild) "release" else "debug"
val rustProfile = providers.gradleProperty("desktopFushiRustProfile").orElse(defaultRustProfile)
val rustAbis = providers.gradleProperty("desktopFushiRustAbis").orElse("arm64-v8a,armeabi-v7a,x86_64")
val pythonExe = providers.environmentVariable("PYTHON")
    .orElse(if (System.getProperty("os.name").startsWith("Windows")) "python" else "python3")

val buildRustJni by tasks.registering(Exec::class) {
    onlyIf { System.getenv("DESKTOP_FUSHI_SKIP_RUST_BUILD") != "1" }
    workingDir = rootProject.projectDir.parentFile
    commandLine(
        pythonExe.get(),
        "scripts/build_android_rust.py",
        "--profile",
        rustProfile.get(),
        "--abis",
        rustAbis.get()
    )
}

tasks.named("preBuild") {
    dependsOn(buildRustJni)
}
