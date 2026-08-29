package com.autonomousdroneexpert.m1c.domain

/**
 * The safety gate. Every item must be individually accepted before any port-opening
 * action is permitted. [allAccepted] is the single source of truth the UI must consult
 * before enabling any open/test button.
 */
enum class SafetyItem {
    LIPO_DISCONNECTED,
    PROPELLERS_REMOVED,
    USB_ONLY,
    AIRCRAFT_SECURED,
    CONFIGURATORS_CLOSED,
    NO_FLIGHT_OR_MOTOR_TEST,
    NO_PAYLOAD_WRITES,
    OPEN_NOT_SIDE_EFFECT_FREE,
}

data class SafetyAttestation(
    val accepted: Set<SafetyItem> = emptySet(),
    /** Epoch millis when the gate became fully accepted; null until then. Injected, never wall-clock read in domain. */
    val attestedAtEpochMillis: Long? = null,
) {
    val allAccepted: Boolean get() = accepted.containsAll(SafetyItem.entries.toSet())

    fun toggle(item: SafetyItem, on: Boolean, nowEpochMillis: Long): SafetyAttestation {
        val next = if (on) accepted + item else accepted - item
        val complete = next.containsAll(SafetyItem.entries.toSet())
        return copy(
            accepted = next,
            attestedAtEpochMillis = if (complete) (attestedAtEpochMillis ?: nowEpochMillis) else null,
        )
    }
}
