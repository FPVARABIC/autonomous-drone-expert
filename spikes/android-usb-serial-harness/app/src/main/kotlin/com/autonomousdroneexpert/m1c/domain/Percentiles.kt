package com.autonomousdroneexpert.m1c.domain

/** Summary of timeout-slice durations, in milliseconds. */
data class TimeoutStats(
    val samples: Int,
    val minMs: Double,
    val medianMs: Double,
    val p95Ms: Double,
    val maxMs: Double,
)

object Percentiles {
    /** Nearest-rank percentile on a copy-sorted list. Returns null for an empty input. */
    fun summarize(valuesMs: List<Double>): TimeoutStats? {
        if (valuesMs.isEmpty()) return null
        val sorted = valuesMs.sorted()
        fun pick(q: Double): Double {
            val idx = ((sorted.size - 1) * q).toInt()
            return sorted[idx]
        }
        return TimeoutStats(
            samples = sorted.size,
            minMs = sorted.first(),
            medianMs = pick(0.5),
            p95Ms = pick(0.95),
            maxMs = sorted.last(),
        )
    }
}
