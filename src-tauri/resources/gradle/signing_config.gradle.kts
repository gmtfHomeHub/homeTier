android {
    signingConfigs {
        val homeTierRelease = findByName("release") ?: create("release")
        homeTierRelease.storeFile = file("../keystore/release.keystore")
        homeTierRelease.storePassword = System.getenv("KEYSTORE_PASSWORD") ?: ""
        homeTierRelease.keyAlias = "hometier"
        homeTierRelease.keyPassword = System.getenv("KEY_PASSWORD") ?: ""
    }
    buildTypes {
        getByName("release") {
            signingConfig = signingConfigs.getByName("release")
            isMinifyEnabled = true
            isShrinkResources = true
            proguardFiles(getDefaultProguardFile("proguard-android-optimize.txt"), "proguard-rules.pro")
        }
    }
}
