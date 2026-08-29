package com.autonomousdroneexpert.m1c.domain

/**
 * TEST-ONLY simulation. This is NOT a real transport and never touches hardware. It
 * scripts read/open outcomes so the runner can be tested deterministically on the JVM.
 * It lives under src/test and is clearly marked as a simulation.
 */
class FakeReadOnlySession(
    private val script: MutableList<ReadOutcome>,
    private val closeOutcome: CloseOutcome = CloseOutcome.Clean,
) : ReadOnlySession {
    var closed = false
        private set

    override suspend fun read(): ReadOutcome =
        if (script.isNotEmpty()) script.removeAt(0) else ReadOutcome.TimedOut(250.0)

    override fun close(): CloseOutcome {
        closed = true
        return closeOutcome
    }
}

class FakeOpenable(
    override val info: UsbDeviceInfo,
    private val openError: ClassifiedError? = null,
    private val readScript: () -> MutableList<ReadOutcome> = { mutableListOf() },
    // Per-open close outcome, indexed by the 1-based open count, so tests can make a
    // specific cycle's close fail (e.g. the 20th of 20).
    private val closeOutcome: (openIndex: Int) -> CloseOutcome = { CloseOutcome.Clean },
) : Openable {
    var opens = 0
        private set

    override suspend fun open(baud: Int, readTimeoutMs: Int): OpenResult {
        opens++
        return openError?.let { OpenResult.Failed(it) }
            ?: OpenResult.Opened(FakeReadOnlySession(readScript(), closeOutcome(opens)))
    }

    companion object {
        fun info(deviceId: Int = 1) = UsbDeviceInfo(
            deviceName = "/dev/fake$deviceId",
            vid = 0x0483,
            pid = 0x5740,
            manufacturer = "STMicroelectronics",
            product = "Virtual COM Port",
            serial = null,
            androidDeviceId = deviceId,
            permissionGranted = true,
            driverMatch = "interfaces=1 firstClass=2",
        )
    }
}
