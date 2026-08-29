package com.autonomousdroneexpert.m1c.domain

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * The honesty of the bulk-read classification is the whole point of this stage: on Android a
 * non-positive `bulkTransfer` result cannot be told apart from an I/O error, so we must never
 * assert a *confirmed* timeout from it. These tests pin that behaviour.
 */
class TransportClassifiersTest {

    @Test
    fun `positive result is data carrying a byte count only`() {
        val outcome = TransportClassifiers.classifyBulkRead(
            result = 8, elapsedMs = 5.0, configuredTimeoutMs = 250, deviceStillPresent = true,
        )
        assertTrue(outcome is ReadOutcome.Data)
        assertEquals(8, (outcome as ReadOutcome.Data).byteCount)
    }

    @Test
    fun `non-positive result early with device present is UNKNOWN_IO_ERROR, never a timeout`() {
        val outcome = TransportClassifiers.classifyBulkRead(
            result = -1, elapsedMs = 5.0, configuredTimeoutMs = 250, deviceStillPresent = true,
        )
        assertTrue("must not be classified as a timeout", outcome is ReadOutcome.Failed)
        val failed = outcome as ReadOutcome.Failed
        assertEquals(TransportError.UNKNOWN_IO_ERROR, failed.error.error)
        assertEquals("carries elapsed timing", 5.0, failed.elapsedMs, 0.0)
    }

    @Test
    fun `non-positive result near the timeout with device present is an INFERRED timeout, clearly labelled`() {
        val outcome = TransportClassifiers.classifyBulkRead(
            result = -1, elapsedMs = 249.0, configuredTimeoutMs = 250, deviceStillPresent = true,
        )
        assertTrue(outcome is ReadOutcome.TimedOut)
        val t = outcome as ReadOutcome.TimedOut
        assertTrue("timeout must be marked inferred, not confirmed", t.inferred)
        assertTrue("basis names it INFERRED_TIMEOUT", t.basis.contains("INFERRED_TIMEOUT"))
    }

    @Test
    fun `non-positive result with device gone is a disconnect carrying elapsed timing`() {
        val outcome = TransportClassifiers.classifyBulkRead(
            result = -1, elapsedMs = 17.0, configuredTimeoutMs = 250, deviceStillPresent = false,
        )
        assertTrue(outcome is ReadOutcome.Failed)
        val failed = outcome as ReadOutcome.Failed
        assertEquals(TransportError.DEVICE_DISCONNECTED, failed.error.error)
        assertEquals(17.0, failed.elapsedMs, 0.0)
    }

    @Test
    fun `an unexpected throwable maps to UNKNOWN_IO_ERROR with a safe, non-empty message`() {
        val classified = TransportClassifiers.classifyThrowable(IllegalStateException("bulk failed"))
        assertEquals(TransportError.UNKNOWN_IO_ERROR, classified.error)
        assertTrue(classified.originalMessage.isNotBlank())
        assertTrue(classified.originalMessage.contains("bulk failed"))
    }

    @Test
    fun `a throwable with no message still yields a non-empty classification`() {
        val classified = TransportClassifiers.classifyThrowable(RuntimeException())
        assertEquals(TransportError.UNKNOWN_IO_ERROR, classified.error)
        assertFalse(classified.originalMessage.isBlank())
    }
}
