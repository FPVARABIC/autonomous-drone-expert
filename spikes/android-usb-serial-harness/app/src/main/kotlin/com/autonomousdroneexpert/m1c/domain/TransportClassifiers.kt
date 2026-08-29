package com.autonomousdroneexpert.m1c.domain

/**
 * Pure, Android-independent classification of low-level transport results, so the honesty
 * of the mapping is unit-testable on the JVM without a device.
 *
 * The central fact this encodes: Android's `UsbDeviceConnection.bulkTransfer` returns a
 * single `Int`. A value `> 0` is a byte count; any value `<= 0` is **ambiguous** -- it may
 * be a read timeout OR an I/O error, and the API cannot tell them apart. We therefore never
 * assert a *confirmed* timeout from a non-positive result: we either infer one (clearly
 * labelled) or classify an honest error.
 */
object TransportClassifiers {

    /**
     * Fraction of the configured timeout at/after which a non-positive result, with the
     * device still enumerated, is treated as an INFERRED timeout rather than an I/O error.
     * Below it, a non-positive result is too early to be a timeout and is UNKNOWN_IO_ERROR.
     */
    const val TIMEOUT_INFERENCE_FRACTION = 0.8

    /**
     * Classify one `bulkTransfer` result. Carries only a byte **count**, never bytes.
     *
     * @param result the raw `bulkTransfer` return value.
     * @param elapsedMs measured wall time for the call.
     * @param configuredTimeoutMs the timeout the read was issued with.
     * @param deviceStillPresent whether the device is still enumerated after the call.
     */
    fun classifyBulkRead(
        result: Int,
        elapsedMs: Double,
        configuredTimeoutMs: Int,
        deviceStillPresent: Boolean,
    ): ReadOutcome {
        if (result > 0) return ReadOutcome.Data(byteCount = result, elapsedMs = elapsedMs)

        // result <= 0 : ambiguous at the Android API level.
        if (!deviceStillPresent) {
            return ReadOutcome.Failed(
                ClassifiedError(
                    TransportError.DEVICE_DISCONNECTED,
                    "device no longer enumerated after non-positive bulkTransfer (result=$result)",
                ),
                elapsedMs = elapsedMs,
            )
        }

        val inferThresholdMs = configuredTimeoutMs * TIMEOUT_INFERENCE_FRACTION
        return if (elapsedMs >= inferThresholdMs) {
            ReadOutcome.TimedOut(
                elapsedMs = elapsedMs,
                inferred = true,
                basis = "INFERRED_TIMEOUT: waited ${elapsedMs}ms (>= ${inferThresholdMs}ms of " +
                    "${configuredTimeoutMs}ms) with device present; Android bulkTransfer cannot " +
                    "distinguish a real timeout from an I/O error",
            )
        } else {
            ReadOutcome.Failed(
                ClassifiedError(
                    TransportError.UNKNOWN_IO_ERROR,
                    "non-positive bulkTransfer (result=$result) after ${elapsedMs}ms, well before " +
                        "the ${configuredTimeoutMs}ms timeout, device still present -- cannot be " +
                        "claimed as a timeout",
                ),
                elapsedMs = elapsedMs,
            )
        }
    }

    /** Map any unexpected throwable to UNKNOWN_IO_ERROR with a SAFE message (never payload). */
    fun classifyThrowable(t: Throwable): ClassifiedError =
        ClassifiedError(TransportError.UNKNOWN_IO_ERROR, safeMessage(t))

    private fun safeMessage(t: Throwable): String {
        val msg = t.message
        return if (msg.isNullOrBlank()) t.javaClass.simpleName else "${t.javaClass.simpleName}: $msg"
    }
}
