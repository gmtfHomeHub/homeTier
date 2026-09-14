# ProGuard/R8 rules for homeTier Android
# Protects JNI interfaces, Tauri plugin classes, and reflection-used code

# ===== JNI / Rust-generated classes =====
# Rust JNI bindings use these package patterns
-keep class com.hometier.** { *; }
-keep class com.hometier.**$* { *; }

# ===== Tauri Android plugin classes =====
# Tauri plugin-generated classes must not be stripped
-keep class com.tauri.** { *; }
-keep class org.chromium.** { *; }

# ===== ML Kit / Barcode Scanner =====
# ML Kit classes used via reflection (scanner.process, etc.)
-keep class com.google.mlkit.** { *; }
-keep class com.google.android.gms.internal.mlkit_vision_barcode.** { *; }
-keep class com.google.android.gms.tasks.** { *; }

# ===== Kotlin stdlib / Coroutines =====
-keep class kotlin.** { *; }
-keep class kotlinx.coroutines.** { *; }

# ===== OkHttp / HTTP client (if used) =====
-keep class okhttp3.** { *; }
-keep class okio.** { *; }

# ===== JNI native methods =====
# Keep all native method declarations
-keepclasseswithmembers class * {
    native <methods>;
}

# ===== Serialization / Parcelable =====
-keep class * implements android.os.Parcelable {
    public static final ** CREATOR;
}

# ===== Keep enums =====
-keepclassmembers enum * {
    public static **[] values();
    public static ** valueOf(java.lang.String);
}

# ===== WebView / JavaScript interface =====
-keepclassmembers class * {
    @android.webkit.JavascriptInterface <methods>;
}

# ===== Prevent R8 from removing unused library classes =====
# (some libraries use reflection internally)
-dontwarn com.google.**
-dontwarn okhttp3.**
-dontwarn okio.**
-dontwarn kotlinx.coroutines.**

# ===== Keep annotations =====
-keepattributes *Annotation*
-keepattributes Signature
-keepattributes EnclosingMethod
-keepattributes InnerClasses

# ===== Disable optimizations that might break JNI =====
# (some JNI code relies on exact method signatures)
-optimizations !code/allocation/variable
-optimizations !code/allocation/field