android {
          signingConfigs {
              create("release") {
                  storeFile = file("../keystore/release.keystore")
                  storePassword = System.getenv("KEYSTORE_PASSWORD") ?: ""
                  keyAlias = "hometier"
                  keyPassword = System.getenv("KEY_PASSWORD") ?: ""
              }
          }
          buildTypes {
              release {
                  signingConfig = signingConfigs.getByName("release")
                  isMinifyEnabled = false
                  proguardFiles(getDefaultProguardFile("proguard-android-optimize.txt"), "proguard-rules.pro")
              }
          }
      }