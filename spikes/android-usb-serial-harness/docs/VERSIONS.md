# M1C Android spike — pinned version matrix

All versions are pinned exactly. This is a SPIKE; none of these is a production decision.

| Component | Version | Notes |
| --- | --- | --- |
| Gradle (wrapper) | 8.11.1 | `gradle/wrapper/gradle-wrapper.properties` |
| Android Gradle Plugin | 8.7.3 | requires Gradle 8.9+ |
| Kotlin | 2.0.21 | `org.jetbrains.kotlin.android` |
| Compose Compiler plugin | 2.0.21 | `org.jetbrains.kotlin.plugin.compose` (bundled with Kotlin 2.0+) |
| Compose BOM | 2024.10.01 | governs all `androidx.compose.*` versions |
| compileSdk / targetSdk | 35 | Android 15 |
| minSdk | 24 | Android 7.0; USB host API is available |
| JDK | 17 | build + `jvmTarget` |
| androidx.core:core-ktx | 1.13.1 | |
| androidx.activity:activity-compose | 1.9.3 | |
| androidx.lifecycle:* | 2.8.7 | runtime-ktx, viewmodel-compose |
| kotlinx-coroutines-android | 1.9.0 | + coroutines-test for unit tests |
| junit | 4.13.2 | JVM unit tests only (no instrumentation) |

## Licenses (all permissive; reviewed for the spike)

- Android Gradle Plugin, Kotlin, kotlinx-coroutines: **Apache-2.0**.
- AndroidX (core, activity, lifecycle, compose): **Apache-2.0**.
- JUnit 4: **EPL-1.0** (test-only dependency, not shipped in the APK).

No third-party USB-serial library is used. USB access is implemented directly on Android's
own `UsbManager`, behind our `PlatformTransport` interface, which keeps the read-only
guarantee trivially auditable and adds no external license to review. A USB-serial library
remains an option for a later, richer harness; it is deliberately not adopted here and is
**not** an Android backend decision.

## Dependency locking and verification

Every dependency version is pinned exactly above (reproducible by version). Committed
Gradle lockfiles and full checksum **dependency verification metadata** are **deferred**:
they cannot be bootstrapped in this environment, because the Google Maven repository is not
reachable from the authoring sandbox (only the CI runner resolves dependencies). Generating
and committing lock/verification state requires a build host with network access to
`dl.google.com` / `maven.google.com`, which is a follow-up on a machine that has it. This
is the honest "when practical" call; it is recorded rather than faked.
