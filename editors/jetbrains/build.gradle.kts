plugins {
    id("java")
    id("org.jetbrains.intellij") version "1.17.4"
}

group = "com.lykaon"
version = "0.1.0"

repositories { mavenCentral() }

intellij {
    version.set("2023.2")
    type.set("IC")
}

java {
    toolchain { languageVersion.set(JavaLanguageVersion.of(17)) }
}