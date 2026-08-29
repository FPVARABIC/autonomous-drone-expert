package com.autonomousdroneexpert.m1c.domain

import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.currentCoroutineContext
import kotlinx.coroutines.delay
import kotlinx.coroutines.ensureActive

/**
 * Runs the read-only hardware stages against any [Openable] (real Android USB device in
 * the app; a fake in tests). Android-independent so it is fully JVM unit-testable.
 *
 * Cancellation: these are suspend functions that check coroutine cancellation between
 * reads via [ensureActive]. **Coroutine cancellation is NOT proof that the underlying
 * driver I/O was cancelled** -- an in-flight blocking read may still be running in the
 * platform layer until its own timeout. This distinction is deliberate and documented.
 * [CancellationException] is always re-thrown, never swallowed or mis-classified.
 *
 * @param clock injected elapsed-millis source so tests are deterministic (no wall clock).
 */
class ReadOnlyHardwareTestRunner(private val clock: () -> Long) {

    companion object {
        /** Default single-open dwell: hold the port open long enough to watch the device. */
        const val DEFAULT_SINGLE_OPEN_DWELL_MS = 3_000L
    }

    /**
     * Open once, then **hold the port open** for [dwellMs] before closing -- long enough for
     * the operator to watch LEDs / re-enumeration -- instead of an instantaneous open/close.
     * Records open latency, dwell, and the close outcome. Never a PASS: HARDWARE_OBSERVED only.
     *
     * @param onPortOpen invoked once the port is open and before the dwell, so the UI can
     *   show "the port is open now -- watch the device".
     */
    suspend fun singleOpen(
        target: Openable,
        baud: Int,
        timeoutMs: Int,
        dwellMs: Long = DEFAULT_SINGLE_OPEN_DWELL_MS,
        onPortOpen: (() -> Unit)? = null,
    ): HardwareObservation {
        val at = clock()
        val openStart = clock()
        return when (val r = safeOpen(target, baud, timeoutMs)) {
            is OpenResult.Opened -> {
                val openLatencyMs = clock() - openStart
                onPortOpen?.invoke()
                val session = r.session
                var dwellElapsedMs = 0L
                var closeOutcome: CloseOutcome = CloseOutcome.Clean
                try {
                    val dwellStart = clock()
                    // Real dwell: a cooperative suspension point, so Stop still cancels it.
                    delay(dwellMs)
                    dwellElapsedMs = clock() - dwellStart
                } finally {
                    // Always close (even on cancellation); never swallow a close failure.
                    closeOutcome = closeAndClassify(session)
                }
                val closeError = (closeOutcome as? CloseOutcome.Failed)?.error
                HardwareObservation(
                    stage = TestStage.SINGLE_OPEN,
                    status = if (closeError == null) ObservationStatus.OBSERVED else ObservationStatus.CLASSIFIED_ERROR,
                    detail = "port held open then closed; open_latency=${openLatencyMs}ms " +
                        "dwell=${dwellElapsedMs}ms " + closeText(closeOutcome) + "; " +
                        "observe LED/COM/DFU/behaviour on the device",
                    error = closeError?.error,
                    atElapsedMillis = at,
                )
            }
            is OpenResult.Failed -> errorObservation(TestStage.SINGLE_OPEN, r.error, at)
        }
    }

    suspend fun openCloseCycles(target: Openable, cycles: Int, baud: Int, timeoutMs: Int): HardwareObservation {
        val at = clock()
        var clean = 0
        var firstError: ClassifiedError? = null
        repeat(cycles) {
            currentCoroutineContext().ensureActive()
            when (val r = safeOpen(target, baud, timeoutMs)) {
                is OpenResult.Opened -> {
                    // A cycle is clean only if BOTH the open and the close succeed. A close
                    // failure is recorded (first error), never counted as a clean cycle.
                    when (val c = closeAndClassify(r.session)) {
                        is CloseOutcome.Failed -> if (firstError == null) firstError = c.error
                        CloseOutcome.Clean -> clean++
                    }
                }
                is OpenResult.Failed -> if (firstError == null) firstError = r.error
            }
        }
        return HardwareObservation(
            stage = TestStage.OPEN_CLOSE_CYCLES,
            status = if (firstError == null) ObservationStatus.OBSERVED else ObservationStatus.CLASSIFIED_ERROR,
            detail = "$clean/$cycles clean cycles" +
                (firstError?.let { "; first error ${it.error}: ${it.originalMessage}" } ?: ""),
            error = firstError?.error,
            atElapsedMillis = at,
        )
    }

    suspend fun readTimeoutAccuracy(target: Openable, timeoutMs: Int, samples: Int): HardwareObservation {
        val at = clock()
        return when (val r = safeOpen(target, 115_200, timeoutMs)) {
            is OpenResult.Failed -> errorObservation(TestStage.READ_TIMEOUT_ACCURACY, r.error, at)
            is OpenResult.Opened -> {
                val session = r.session
                val timeouts = ArrayList<Double>()
                var dataEvents = 0
                var inferred = 0
                var otherErrors = 0
                var firstError: ClassifiedError? = null
                var closeOutcome: CloseOutcome = CloseOutcome.Clean
                try {
                    repeat(samples) {
                        currentCoroutineContext().ensureActive()
                        when (val o = safeRead(session)) {
                            is ReadOutcome.TimedOut -> { timeouts.add(o.elapsedMs); if (o.inferred) inferred++ }
                            is ReadOutcome.Data -> dataEvents++ // count only; content never read
                            is ReadOutcome.Failed -> { otherErrors++; if (firstError == null) firstError = o.error }
                        }
                    }
                } finally {
                    // Never swallow a close failure; it is folded into status + error below.
                    closeOutcome = closeAndClassify(session)
                }
                val stats = Percentiles.summarize(timeouts)
                val closeError = (closeOutcome as? CloseOutcome.Failed)?.error
                // The original read evidence is preserved; a read error OR a close failure
                // raises the status so nothing looks silently clean.
                val reportedError = firstError ?: closeError
                HardwareObservation(
                    stage = TestStage.READ_TIMEOUT_ACCURACY,
                    status = if (otherErrors > 0 || closeError != null)
                        ObservationStatus.OBSERVED_WITH_ERRORS else ObservationStatus.OBSERVED,
                    detail = "target ${timeoutMs}ms; timeout_samples=${timeouts.size} inferred=$inferred " +
                        "data_events=$dataEvents other_errors=$otherErrors" +
                        (firstError?.let { "; first_error=${it.error}: ${it.originalMessage}" } ?: "") +
                        "; " + closeText(closeOutcome) +
                        "; basis: non-positive bulkTransfer is ambiguous on Android; inferred timeouts are labelled",
                    timeoutStats = stats,
                    error = reportedError?.error,
                    atElapsedMillis = at,
                )
            }
        }
    }

    /** Reads until a non-timeout error appears (e.g. an unplug), recording slice + total timing. */
    suspend fun unplugDetection(target: Openable, timeoutMs: Int, maxSlices: Int): HardwareObservation {
        val at = clock()
        return when (val r = safeOpen(target, 115_200, timeoutMs)) {
            is OpenResult.Failed -> errorObservation(TestStage.UNPLUG_DETECTION, r.error, at)
            is OpenResult.Opened -> {
                val session = r.session
                val startedAt = clock()
                var slices = 0
                var disconnect: ClassifiedError? = null
                var sliceElapsedMs = 0.0
                var closeOutcome: CloseOutcome = CloseOutcome.Clean
                try {
                    // Capture the disconnect evidence but DON'T return early: the close must
                    // still run in finally and be reported alongside the disconnect.
                    for (s in 1..maxSlices) {
                        currentCoroutineContext().ensureActive()
                        slices = s
                        val o = safeRead(session)
                        if (o is ReadOutcome.Failed) {
                            disconnect = o.error
                            sliceElapsedMs = o.elapsedMs
                            break
                        }
                        // TimedOut / Data -> keep waiting for the unplug
                    }
                } finally {
                    closeOutcome = closeAndClassify(session)
                }
                val totalMs = clock() - startedAt
                val closeError = (closeOutcome as? CloseOutcome.Failed)?.error
                val disconnectError = disconnect
                if (disconnectError != null) {
                    HardwareObservation(
                        stage = TestStage.UNPLUG_DETECTION,
                        status = ObservationStatus.CLASSIFIED_ERROR,
                        detail = "surfaced ${disconnectError.error} on read after $slices slice(s); " +
                            "slice_elapsed=${sliceElapsedMs}ms total=${totalMs}ms; " +
                            "basis=${disconnectError.originalMessage}; " + closeText(closeOutcome) +
                            "; record what you saw physically",
                        // Disconnect stays the headline classification; a close failure is
                        // added to the detail without hiding the disconnect evidence.
                        error = disconnectError.error,
                        atElapsedMillis = at,
                    )
                } else {
                    HardwareObservation(
                        stage = TestStage.UNPLUG_DETECTION,
                        status = if (closeError == null) ObservationStatus.OBSERVED else ObservationStatus.OBSERVED_WITH_ERRORS,
                        detail = "no unplug observed within $maxSlices slices; total=${totalMs}ms; " +
                            closeText(closeOutcome),
                        error = closeError?.error,
                        atElapsedMillis = at,
                    )
                }
            }
        }
    }

    /** Open, mapping any unexpected throwable (never a [CancellationException]) to UNKNOWN_IO_ERROR. */
    private suspend fun safeOpen(target: Openable, baud: Int, timeoutMs: Int): OpenResult =
        try {
            target.open(baud, timeoutMs)
        } catch (e: CancellationException) {
            throw e
        } catch (t: Throwable) {
            OpenResult.Failed(TransportClassifiers.classifyThrowable(t))
        }

    /** Read, mapping any unexpected throwable (never a [CancellationException]) to UNKNOWN_IO_ERROR. */
    private suspend fun safeRead(session: ReadOnlySession): ReadOutcome =
        try {
            session.read()
        } catch (e: CancellationException) {
            throw e
        } catch (t: Throwable) {
            ReadOutcome.Failed(TransportClassifiers.classifyThrowable(t))
        }

    /**
     * Close and return the classified outcome. A well-behaved session already returns a
     * [CloseOutcome]; a session that unexpectedly throws is still mapped to a
     * [CloseOutcome.Failed] rather than crashing the stage. `close()` is not a suspension
     * point, so this never hides a [CancellationException] propagating from the try block.
     */
    private fun closeAndClassify(session: ReadOnlySession): CloseOutcome =
        try {
            session.close()
        } catch (t: Throwable) {
            CloseOutcome.Failed(TransportClassifiers.classifyThrowable(t))
        }

    /** Human-readable close status for observation details: `close=CLEAN` or `close_error:...`. */
    private fun closeText(outcome: CloseOutcome): String = when (outcome) {
        CloseOutcome.Clean -> "close=CLEAN"
        is CloseOutcome.Failed -> "close_error:${outcome.error.error}: ${outcome.error.originalMessage}"
    }

    private fun errorObservation(stage: TestStage, error: ClassifiedError, at: Long) =
        HardwareObservation(
            stage = stage,
            status = ObservationStatus.CLASSIFIED_ERROR,
            detail = "open failed: ${error.error}: ${error.originalMessage}",
            error = error.error,
            atElapsedMillis = at,
        )
}
