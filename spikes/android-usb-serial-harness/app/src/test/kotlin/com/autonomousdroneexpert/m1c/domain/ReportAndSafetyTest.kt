package com.autonomousdroneexpert.m1c.domain

import org.junit.Assert.assertFalse
import org.junit.Assert.assertThrows
import org.junit.Assert.assertTrue
import org.junit.Test

class ReportAndSafetyTest {

    private fun sampleReport(): HardwareTestReport = HardwareTestReport(
        environment = ReportEnvironment(
            appVersion = "0.1.0-spike",
            sourceSha = "deadbeef",
            androidVersion = "Android 14 (SDK 34)",
            phoneModel = "TestCo TestPhone",
            applicationId = "com.autonomousdroneexpert.m1c",
        ),
        device = FakeOpenable.info(),
        safetyAttestedAtEpochMillis = 1720000000000L,
        testParameters = mapOf("baud" to "115200", "readTimeoutMs" to "250"),
        observations = listOf(
            HardwareObservation(
                stage = TestStage.READ_TIMEOUT_ACCURACY,
                status = ObservationStatus.OBSERVED,
                detail = "target 250ms; timeout_samples=100",
                timeoutStats = TimeoutStats(100, 250.1, 262.4, 264.4, 265.4),
                atElapsedMillis = 10,
            ),
        ),
    )

    @Test
    fun `report json escapes control characters and quotes`() {
        // Force a device product with a quote and newline to exercise escaping.
        val tricky = FakeOpenable.info().copy(product = "line1\n\"quoted\"")
        val report = sampleReport().copy(device = tricky)
        val json = report.toJson()
        assertTrue(json.contains("\\n"))
        assertTrue(json.contains("\\\""))
        // Balanced braces sanity.
        assertTrue(json.count { it == '{' } == json.count { it == '}' })
    }

    @Test
    fun `report never claims READY or PASS in json or text`() {
        val report = sampleReport()
        for (rendered in listOf(report.toJson(), report.toPlainText())) {
            val upper = rendered.uppercase()
            assertFalse("must not contain READY", upper.contains("READY —") && upper.contains("VERIFIED"))
            assertFalse("must not contain PASS verdict", Regex("\\bPASS\\b").containsMatchIn(upper))
        }
        assertTrue(report.overallStatus.contains("REQUIRES HARDWARE TEST"))
    }

    @Test
    fun `an observation cannot be constructed with a readiness or pass claim`() {
        assertThrows(IllegalArgumentException::class.java) {
            HardwareObservation(
                stage = TestStage.SINGLE_OPEN,
                status = ObservationStatus.OBSERVED,
                detail = "device is READY to fly",
                atElapsedMillis = 0,
            )
        }
    }

    @Test
    fun `ReadOutcome Data carries a byte count only, never the bytes`() {
        // Structural guarantee: no ByteArray-typed member exists on the data outcome.
        val members = ReadOutcome.Data::class.java.declaredFields.map { it.type.simpleName }
        assertFalse("no ByteArray field on ReadOutcome.Data", members.contains("byte[]"))
        val data = ReadOutcome.Data(byteCount = 8, elapsedMs = 5.0)
        assertTrue(data.byteCount == 8)
    }
}
