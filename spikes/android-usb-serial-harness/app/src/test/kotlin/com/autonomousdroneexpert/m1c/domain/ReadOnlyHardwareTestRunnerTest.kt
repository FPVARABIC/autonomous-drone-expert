package com.autonomousdroneexpert.m1c.domain

import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.Job
import kotlinx.coroutines.delay
import kotlinx.coroutines.launch
import kotlinx.coroutines.test.runTest
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

class ReadOnlyHardwareTestRunnerTest {
    private val runner = ReadOnlyHardwareTestRunner(clock = { 0L })

    @Test
    fun `permission denied on open is a classified error, not a crash`() = runTest {
        val target = FakeOpenable(
            FakeOpenable.info(),
            openError = ClassifiedError(TransportError.PERMISSION_DENIED, "no permission"),
        )
        val obs = runner.singleOpen(target, 115_200, 250)
        assertEquals(ObservationStatus.CLASSIFIED_ERROR, obs.status)
        assertEquals(TransportError.PERMISSION_DENIED, obs.error)
    }

    @Test
    fun `device disconnected during unplug detection is surfaced and classified`() = runTest {
        val target = FakeOpenable(FakeOpenable.info(), readScript = {
            mutableListOf(
                ReadOutcome.TimedOut(1000.0),
                ReadOutcome.TimedOut(1000.0),
                ReadOutcome.Failed(ClassifiedError(TransportError.DEVICE_DISCONNECTED, "gone")),
            )
        })
        val obs = runner.unplugDetection(target, timeoutMs = 1000, maxSlices = 10)
        assertEquals(ObservationStatus.CLASSIFIED_ERROR, obs.status)
        assertEquals(TransportError.DEVICE_DISCONNECTED, obs.error)
    }

    @Test
    fun `read-timeout accuracy computes statistics from timeout slices only`() = runTest {
        val target = FakeOpenable(FakeOpenable.info(), readScript = {
            mutableListOf(
                ReadOutcome.TimedOut(250.0),
                ReadOutcome.Data(byteCount = 8, elapsedMs = 5.0), // counted, not in stats
                ReadOutcome.TimedOut(262.0),
                ReadOutcome.TimedOut(264.0),
            )
        })
        val obs = runner.readTimeoutAccuracy(target, timeoutMs = 250, samples = 4)
        assertEquals(ObservationStatus.OBSERVED, obs.status)
        assertNotNull(obs.timeoutStats)
        val stats = obs.timeoutStats!!
        assertEquals(3, stats.samples) // only the 3 timeouts
        assertEquals(250.0, stats.minMs, 0.0)
        assertEquals(264.0, stats.maxMs, 0.0)
        assertTrue("detail mentions data_events", obs.detail.contains("data_events=1"))
    }

    @Test
    fun `open-close cycles count clean cycles`() = runTest {
        val target = FakeOpenable(FakeOpenable.info())
        val obs = runner.openCloseCycles(target, cycles = 20, baud = 115_200, timeoutMs = 250)
        assertEquals(ObservationStatus.OBSERVED, obs.status)
        assertEquals(20, target.opens)
        assertTrue(obs.detail.startsWith("20/20 clean"))
    }

    @Test
    fun `coroutine cancellation stops a long open-close run`() = runTest {
        // A target whose open never lets the loop finish quickly.
        val target = object : Openable {
            override val info = FakeOpenable.info()
            override suspend fun open(baud: Int, readTimeoutMs: Int): OpenResult {
                delay(1_000) // suspension point -> cooperatively cancellable
                return OpenResult.Opened(FakeReadOnlySession(mutableListOf()))
            }
        }
        var caught = false
        val job: Job = launch {
            try {
                runner.openCloseCycles(target, cycles = 1000, baud = 115_200, timeoutMs = 250)
            } catch (_: CancellationException) {
                caught = true
            }
        }
        // Let it start, then cancel.
        kotlinx.coroutines.yield()
        job.cancel()
        job.join()
        assertTrue("cancellation propagated", caught || job.isCancelled)
    }

    @Test
    fun `single open holds the port open for the dwell then closes cleanly`() = runTest {
        val session = FakeReadOnlySession(mutableListOf())
        val target = object : Openable {
            override val info = FakeOpenable.info()
            override suspend fun open(baud: Int, readTimeoutMs: Int) = OpenResult.Opened(session)
        }
        // Clock tied to virtual time so the dwell is measurable and deterministic.
        val timed = ReadOnlyHardwareTestRunner(clock = { testScheduler.currentTime })
        var openWhileNotClosed = false
        val obs = timed.singleOpen(target, 115_200, 250, dwellMs = 3_000, onPortOpen = {
            openWhileNotClosed = !session.closed
        })
        assertEquals(ObservationStatus.OBSERVED, obs.status)
        assertTrue("callback fired while port still open", openWhileNotClosed)
        assertTrue("port closed after the dwell", session.closed)
        assertTrue("records dwell duration", obs.detail.contains("dwell=3000"))
        assertTrue("records open latency", obs.detail.contains("open_latency="))
        assertTrue("records close outcome", obs.detail.contains("close=CLEAN"))
    }

    @Test
    fun `unexpected exception on open is classified UNKNOWN_IO_ERROR and still yields an observation`() = runTest {
        val target = object : Openable {
            override val info = FakeOpenable.info()
            override suspend fun open(baud: Int, readTimeoutMs: Int): OpenResult =
                throw IllegalStateException("driver blew up")
        }
        val obs = runner.singleOpen(target, 115_200, 250, dwellMs = 0)
        assertEquals(ObservationStatus.CLASSIFIED_ERROR, obs.status)
        assertEquals(TransportError.UNKNOWN_IO_ERROR, obs.error)
    }

    @Test
    fun `read-timeout accuracy surfaces OBSERVED_WITH_ERRORS when a read fails, not a clean result`() = runTest {
        val target = FakeOpenable(FakeOpenable.info(), readScript = {
            mutableListOf(
                ReadOutcome.TimedOut(250.0),
                ReadOutcome.Failed(ClassifiedError(TransportError.UNKNOWN_IO_ERROR, "io glitch"), elapsedMs = 12.0),
                ReadOutcome.TimedOut(262.0),
            )
        })
        val obs = runner.readTimeoutAccuracy(target, timeoutMs = 250, samples = 3)
        assertEquals(ObservationStatus.OBSERVED_WITH_ERRORS, obs.status)
        assertEquals(TransportError.UNKNOWN_IO_ERROR, obs.error)
        assertTrue(obs.detail.contains("other_errors=1"))
    }

    @Test
    fun `unplug detection records slice and total timing on the surfaced error`() = runTest {
        val target = FakeOpenable(FakeOpenable.info(), readScript = {
            mutableListOf(
                ReadOutcome.TimedOut(1000.0),
                ReadOutcome.Failed(ClassifiedError(TransportError.DEVICE_DISCONNECTED, "gone"), elapsedMs = 42.0),
            )
        })
        val obs = runner.unplugDetection(target, timeoutMs = 1000, maxSlices = 10)
        assertEquals(ObservationStatus.CLASSIFIED_ERROR, obs.status)
        assertEquals(TransportError.DEVICE_DISCONNECTED, obs.error)
        assertTrue("slice timing recorded", obs.detail.contains("slice_elapsed=42.0"))
        assertTrue("total timing recorded", obs.detail.contains("total="))
    }

    @Test
    fun `close happens in finally even when a read throws unexpectedly`() = runTest {
        var closed = false
        val throwingSession = object : ReadOnlySession {
            override suspend fun read(): ReadOutcome = throw IllegalStateException("read boom")
            override fun close(): CloseOutcome { closed = true; return CloseOutcome.Clean }
        }
        val target = object : Openable {
            override val info = FakeOpenable.info()
            override suspend fun open(baud: Int, readTimeoutMs: Int) = OpenResult.Opened(throwingSession)
        }
        val obs = runner.readTimeoutAccuracy(target, timeoutMs = 250, samples = 2)
        assertTrue("session closed in finally", closed)
        // The thrown read is classified (UNKNOWN_IO_ERROR), never crashes the stage.
        assertEquals(ObservationStatus.OBSERVED_WITH_ERRORS, obs.status)
    }

    // ---- review round 2: close-failure is evidence, never swallowed or counted clean ----

    private fun closeFail(message: String = "close boom") =
        CloseOutcome.Failed(ClassifiedError(TransportError.UNKNOWN_IO_ERROR, message))

    @Test
    fun `a close failure is not counted as a clean cycle`() = runTest {
        // 20 opens: the 20th cycle's close fails, so at most 19 can be clean.
        val target = FakeOpenable(
            FakeOpenable.info(),
            closeOutcome = { i -> if (i == 20) closeFail("release failed on cycle 20") else CloseOutcome.Clean },
        )
        val obs = runner.openCloseCycles(target, cycles = 20, baud = 115_200, timeoutMs = 250)
        assertEquals(20, target.opens)
        assertEquals("19+1 close-fail must not read 20/20 clean",
            ObservationStatus.CLASSIFIED_ERROR, obs.status)
        assertEquals(TransportError.UNKNOWN_IO_ERROR, obs.error)
        assertTrue("clean count excludes the failed close", obs.detail.startsWith("19/20 clean"))
    }

    @Test
    fun `single open records a close failure as a classified error`() = runTest {
        val target = FakeOpenable(FakeOpenable.info(), closeOutcome = { closeFail("close failed") })
        val obs = runner.singleOpen(target, 115_200, 250, dwellMs = 0)
        assertEquals(ObservationStatus.CLASSIFIED_ERROR, obs.status)
        assertEquals(TransportError.UNKNOWN_IO_ERROR, obs.error)
        assertTrue("close error surfaced in detail", obs.detail.contains("close_error:"))
    }

    @Test
    fun `read-timeout accuracy surfaces a close failure without hiding the read evidence`() = runTest {
        val target = FakeOpenable(
            FakeOpenable.info(),
            readScript = { mutableListOf(ReadOutcome.TimedOut(250.0), ReadOutcome.TimedOut(262.0)) },
            closeOutcome = { closeFail("close failed") },
        )
        val obs = runner.readTimeoutAccuracy(target, timeoutMs = 250, samples = 2)
        assertEquals(ObservationStatus.OBSERVED_WITH_ERRORS, obs.status)
        assertEquals(TransportError.UNKNOWN_IO_ERROR, obs.error)
        // Read evidence preserved...
        assertNotNull(obs.timeoutStats)
        assertTrue(obs.detail.contains("timeout_samples=2"))
        // ...and the close failure is added, not swallowed.
        assertTrue(obs.detail.contains("close_error:"))
    }

    @Test
    fun `unplug keeps the disconnect evidence and adds a close failure without hiding it`() = runTest {
        val target = FakeOpenable(
            FakeOpenable.info(),
            readScript = {
                mutableListOf(
                    ReadOutcome.TimedOut(1000.0),
                    ReadOutcome.Failed(ClassifiedError(TransportError.DEVICE_DISCONNECTED, "gone"), elapsedMs = 42.0),
                )
            },
            closeOutcome = { closeFail("close failed") },
        )
        val obs = runner.unplugDetection(target, timeoutMs = 1000, maxSlices = 10)
        // Disconnect stays the headline classification (evidence not hidden by the close error).
        assertEquals(ObservationStatus.CLASSIFIED_ERROR, obs.status)
        assertEquals(TransportError.DEVICE_DISCONNECTED, obs.error)
        assertTrue("disconnect evidence kept", obs.detail.contains("DEVICE_DISCONNECTED"))
        assertTrue("slice timing kept", obs.detail.contains("slice_elapsed=42.0"))
        assertTrue("close failure added", obs.detail.contains("close_error:"))
    }
}
