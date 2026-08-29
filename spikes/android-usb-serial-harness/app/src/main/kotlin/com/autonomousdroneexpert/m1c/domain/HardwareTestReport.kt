package com.autonomousdroneexpert.m1c.domain

/** Environment facts captured for the export. No payload content, no secrets. */
data class ReportEnvironment(
    val appVersion: String,
    val sourceSha: String,
    val androidVersion: String,
    val phoneModel: String,
    val applicationId: String,
)

/**
 * The exported report. Its overall status is fixed at `REQUIRES HARDWARE TEST` for a
 * spike -- there is no code path that promotes it to READY or PASS.
 */
data class HardwareTestReport(
    val environment: ReportEnvironment,
    val device: UsbDeviceInfo?,
    val safetyAttestedAtEpochMillis: Long?,
    val testParameters: Map<String, String>,
    val observations: List<HardwareObservation>,
) {
    val overallStatus: String = "REQUIRES HARDWARE TEST — SPIKE"

    fun toJson(): String = Json.obj(
        "spike" to Json.str("M1C — Android USB Serial Hardware Harness"),
        "overallStatus" to Json.str(overallStatus),
        "warning" to Json.str("SPIKE — DO NOT USE FOR FLIGHT CONFIGURATION"),
        "environment" to Json.obj(
            "appVersion" to Json.str(environment.appVersion),
            "sourceSha" to Json.str(environment.sourceSha),
            "androidVersion" to Json.str(environment.androidVersion),
            "phoneModel" to Json.str(environment.phoneModel),
            "applicationId" to Json.str(environment.applicationId),
        ),
        "device" to (device?.let {
            Json.obj(
                "deviceName" to Json.str(it.deviceName),
                "vid" to Json.str(it.vidHex()),
                "pid" to Json.str(it.pidHex()),
                "manufacturer" to Json.strOrNull(it.manufacturer),
                "product" to Json.strOrNull(it.product),
                "serial" to Json.strOrNull(it.serial),
                "androidDeviceId" to Json.num(it.androidDeviceId.toLong()),
                "permissionGranted" to Json.bool(it.permissionGranted),
                "driverMatch" to Json.str(it.driverMatch),
                "note" to Json.str("USB descriptor metadata only; not a firmware identity"),
            )
        } ?: Json.NULL),
        "safetyAttestedAtEpochMillis" to
            (safetyAttestedAtEpochMillis?.let { Json.num(it) } ?: Json.NULL),
        "testParameters" to Json.obj(
            *testParameters.entries.map { it.key to Json.str(it.value) }.toTypedArray()
        ),
        "observations" to Json.arr(
            *observations.map { o ->
                Json.obj(
                    "stage" to Json.str(o.stage.name),
                    "status" to Json.str(o.status.name),
                    "detail" to Json.str(o.detail),
                    "error" to Json.strOrNull(o.error?.name),
                    "atElapsedMillis" to Json.num(o.atElapsedMillis),
                    "timeoutStats" to (o.timeoutStats?.let { s ->
                        Json.obj(
                            "samples" to Json.num(s.samples.toLong()),
                            "minMs" to Json.numD(s.minMs),
                            "medianMs" to Json.numD(s.medianMs),
                            "p95Ms" to Json.numD(s.p95Ms),
                            "maxMs" to Json.numD(s.maxMs),
                        )
                    } ?: Json.NULL),
                )
            }.toTypedArray()
        ),
    )

    fun toPlainText(): String = buildString {
        appendLine("M1C — Android USB Serial Hardware Harness")
        appendLine("SPIKE — DO NOT USE FOR FLIGHT CONFIGURATION")
        appendLine("Overall status: $overallStatus")
        appendLine()
        appendLine("app version : ${environment.appVersion}")
        appendLine("source sha  : ${environment.sourceSha}")
        appendLine("android     : ${environment.androidVersion}")
        appendLine("phone       : ${environment.phoneModel}")
        appendLine("application : ${environment.applicationId}")
        appendLine("safety attested at (epoch ms): ${safetyAttestedAtEpochMillis ?: "-"}")
        appendLine()
        if (device != null) {
            appendLine("device: ${device.deviceName} vid=${device.vidHex()} pid=${device.pidHex()} " +
                "serial=${device.serial ?: "-"} (USB descriptor metadata only)")
        } else {
            appendLine("device: none")
        }
        appendLine()
        appendLine("parameters:")
        testParameters.forEach { (k, v) -> appendLine("  $k = $v") }
        appendLine()
        appendLine("observations:")
        observations.forEach { o ->
            append("  [${o.status}] ${o.stage}: ${o.detail}")
            o.timeoutStats?.let {
                append(" (min=${it.minMs} median=${it.medianMs} p95=${it.p95Ms} max=${it.maxMs} n=${it.samples})")
            }
            appendLine()
        }
    }
}
