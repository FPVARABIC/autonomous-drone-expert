package com.autonomousdroneexpert.m1c.domain

/** The stage a result belongs to. */
enum class TestStage {
    SINGLE_OPEN,
    OPEN_CLOSE_CYCLES,
    READ_TIMEOUT_ACCURACY,
    UNPLUG_DETECTION,
}

/**
 * The outcome of one test stage. There is intentionally NO "PASS" or "READY" value: a
 * spike on a bench cannot certify readiness. Every result is an observation.
 */
enum class ObservationStatus {
    OBSERVED,             // the stage ran and produced evidence
    OBSERVED_WITH_ERRORS, // the stage ran but some reads returned classified errors
    CLASSIFIED_ERROR,     // the stage produced a classified error (still an observation)
    NOT_RUN,              // pending
}

data class HardwareObservation(
    val stage: TestStage,
    val status: ObservationStatus,
    val detail: String,
    val timeoutStats: TimeoutStats? = null,
    val error: TransportError? = null,
    val atElapsedMillis: Long,
) {
    init {
        // Guard in code, not just prose: no observation may claim a pass/ready verdict.
        val forbidden = listOf("READY", "PASS", "CERTIF")
        require(forbidden.none { detail.uppercase().contains(it) }) {
            "an observation detail must not claim readiness/pass: $detail"
        }
    }
}
