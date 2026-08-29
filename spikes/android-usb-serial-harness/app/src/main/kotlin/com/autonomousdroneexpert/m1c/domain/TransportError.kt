package com.autonomousdroneexpert.m1c.domain

/**
 * Android-independent error model for the read-only harness.
 *
 * The original OS/library message is never discarded: [ClassifiedError] keeps the
 * classification alongside a SAFE original message (never raw payload content).
 */
enum class TransportError {
    PERMISSION_DENIED,
    DEVICE_NOT_FOUND,
    PORT_BUSY,
    OPEN_FAILED,
    READ_TIMEOUT,
    OPERATION_CANCELLED,
    DEVICE_DISCONNECTED,
    DRIVER_UNSUPPORTED,
    UNKNOWN_IO_ERROR,
}

/** A classification plus the safe original detail. Raw payload content is never stored. */
data class ClassifiedError(
    val error: TransportError,
    val originalMessage: String,
)
