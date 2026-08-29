package com.autonomousdroneexpert.m1c.domain

import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test

class ErrorClassifierTest {
    @Test
    fun `classified error keeps the safe original message alongside the classification`() {
        val c = ClassifiedError(TransportError.PORT_BUSY, "claimInterface failed")
        assertEquals(TransportError.PORT_BUSY, c.error)
        assertTrue(c.originalMessage.isNotBlank())
    }

    @Test
    fun `every transport error is a distinct, named classification`() {
        val names = TransportError.entries.map { it.name }.toSet()
        assertEquals(TransportError.entries.size, names.size)
        assertTrue(names.contains("DEVICE_DISCONNECTED"))
        assertTrue(names.contains("OPERATION_CANCELLED"))
    }
}
