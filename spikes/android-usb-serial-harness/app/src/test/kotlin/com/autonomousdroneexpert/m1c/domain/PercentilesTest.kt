package com.autonomousdroneexpert.m1c.domain

import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Test

class PercentilesTest {

    @Test
    fun `empty input yields null, never a fabricated zero`() {
        assertNull(Percentiles.summarize(emptyList()))
    }

    @Test
    fun `single sample reports that value for every statistic`() {
        val s = Percentiles.summarize(listOf(250.0))!!
        assertEquals(1, s.samples)
        assertEquals(250.0, s.minMs, 0.0)
        assertEquals(250.0, s.medianMs, 0.0)
        assertEquals(250.0, s.p95Ms, 0.0)
        assertEquals(250.0, s.maxMs, 0.0)
    }

    @Test
    fun `nearest-rank percentiles on a known distribution`() {
        val values = (1..100).map { it.toDouble() }
        val s = Percentiles.summarize(values)!!
        assertEquals(100, s.samples)
        assertEquals(1.0, s.minMs, 0.0)
        assertEquals(100.0, s.maxMs, 0.0)
        // nearest-rank index = floor((n-1)*q)
        assertEquals(values.sorted()[49], s.medianMs, 0.0)
        assertEquals(values.sorted()[94], s.p95Ms, 0.0)
    }

    @Test
    fun `unsorted input is summarized as if sorted`() {
        val s = Percentiles.summarize(listOf(265.0, 250.0, 262.0, 264.0))!!
        assertEquals(250.0, s.minMs, 0.0)
        assertEquals(265.0, s.maxMs, 0.0)
    }
}
