plugins {
    id("com.android.application") version "9.3.0" apply false
}

buildscript {
    repositories {
        google()
        mavenCentral()
    }
    dependencies {
        classpath(kotlin("gradle-plugin", "2.4.10"))
    }
}
