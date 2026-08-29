package com.autonomousdroneexpert.m1c.domain

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertTrue
import org.junit.Test

class SafetyAttestationTest {

    @Test
    fun `gate blocks until every item is accepted`() {
        var a = SafetyAttestation()
        assertFalse(a.allAccepted)
        val items = SafetyItem.entries
        for ((i, item) in items.withIndex()) {
            a = a.toggle(item, on = true, nowEpochMillis = 1000L + i)
            val expected = i == items.lastIndex
            assertEquals("after accepting ${i + 1}/${items.size}", expected, a.allAccepted)
        }
        assertTrue(a.allAccepted)
    }

    @Test
    fun `unchecking any item re-blocks the gate and clears the timestamp`() {
        var a = SafetyAttestation()
        for (item in SafetyItem.entries) a = a.toggle(item, true, 1L)
        assertTrue(a.allAccepted)
        assertNotNull(a.attestedAtEpochMillis)

        a = a.toggle(SafetyItem.USB_ONLY, on = false, nowEpochMillis = 2L)
        assertFalse(a.allAccepted)
        assertNull(a.attestedAtEpochMillis)
    }

    @Test
    fun `attestation timestamp is stamped once when the gate first completes`() {
        var a = SafetyAttestation()
        val items = SafetyItem.entries
        for ((i, item) in items.dropLast(1).withIndex()) a = a.toggle(item, true, 10L + i)
        assertNull(a.attestedAtEpochMillis)
        a = a.toggle(items.last(), true, 999L)
        assertEquals(999L, a.attestedAtEpochMillis)
        // Re-toggling an already-accepted item does not move the original stamp.
        a = a.toggle(items.first(), true, 1234L)
        assertEquals(999L, a.attestedAtEpochMillis)
    }
}
