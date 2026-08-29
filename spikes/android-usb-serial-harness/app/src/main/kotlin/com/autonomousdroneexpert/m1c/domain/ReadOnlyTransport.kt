package com.autonomousdroneexpert.m1c.domain

/**
 * The read-only transport contract. There is deliberately **no write method anywhere on
 * these types** -- the harness cannot send a payload byte because the capability is not
 * expressed in the interface it programs against.
 *
 * Opening a port is not assumed to be side-effect-free: configuring a USB serial port may
 * involve control transfers issued by Android or the driver. The harness sends no payload
 * bytes, but callers must record any observed reset/disconnect/re-enumeration.
 */
interface Openable {
    val info: UsbDeviceInfo
    /** Open at [baud] with the given read timeout. Never writes. */
    suspend fun open(baud: Int, readTimeoutMs: Int): OpenResult
}

sealed interface OpenResult {
    data class Opened(val session: ReadOnlySession) : OpenResult
    data class Failed(val error: ClassifiedError) : OpenResult
}

/** A live read-only session. `read` blocks up to the configured timeout. No write exists. */
interface ReadOnlySession {
    suspend fun read(): ReadOutcome
    /**
     * Close the session. Never throws: a release/close failure is reported as
     * [CloseOutcome.Failed] with the first safe classified error, never swallowed.
     */
    fun close(): CloseOutcome
}

/** The outcome of closing a session -- a close failure is evidence, not something to hide. */
sealed interface CloseOutcome {
    data object Clean : CloseOutcome
    data class Failed(val error: ClassifiedError) : CloseOutcome
}

/**
 * The result of one read. On [Data], only a byte **count** and elapsed time are carried --
 * never the bytes themselves.
 *
 * [TimedOut] is deliberately honest: on Android a non-positive `bulkTransfer` result cannot
 * be distinguished from an I/O error, so a timeout inferred from "waited ~ the full timeout
 * with the device still present" is marked [inferred] with a [basis] string. A determinate
 * (simulated) timeout leaves [inferred] false.
 *
 * [Failed] carries [elapsedMs] so disconnect/error timing is preserved in reports.
 */
sealed interface ReadOutcome {
    data class Data(val byteCount: Int, val elapsedMs: Double) : ReadOutcome
    data class TimedOut(
        val elapsedMs: Double,
        val inferred: Boolean = false,
        val basis: String = "",
    ) : ReadOutcome
    data class Failed(val error: ClassifiedError, val elapsedMs: Double = 0.0) : ReadOutcome
}
