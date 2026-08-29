package com.autonomousdroneexpert.m1c.platform

import android.hardware.usb.UsbConstants
import android.hardware.usb.UsbDevice
import android.hardware.usb.UsbDeviceConnection
import android.hardware.usb.UsbEndpoint
import android.hardware.usb.UsbInterface
import android.hardware.usb.UsbManager
import com.autonomousdroneexpert.m1c.domain.Acquired
import com.autonomousdroneexpert.m1c.domain.ClassifiedError
import com.autonomousdroneexpert.m1c.domain.CloseOutcome
import com.autonomousdroneexpert.m1c.domain.OpenSequence
import com.autonomousdroneexpert.m1c.domain.Openable
import com.autonomousdroneexpert.m1c.domain.OpenResult
import com.autonomousdroneexpert.m1c.domain.ReadOnlySession
import com.autonomousdroneexpert.m1c.domain.ReadOutcome
import com.autonomousdroneexpert.m1c.domain.TransportClassifiers
import com.autonomousdroneexpert.m1c.domain.TransportError
import com.autonomousdroneexpert.m1c.domain.UsbDeviceInfo
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext

/**
 * Read-only USB transport over Android's own UsbManager. No third-party library.
 *
 * There is NO write path here: the only USB transfer performed is a bulk read on the
 * IN endpoint (`bulkTransfer` with an endpoint whose direction is USB_DIR_IN). The class
 * never references USB_DIR_OUT, never writes, and never issues control transfers.
 *
 * Opening claims an interface and locates a bulk IN endpoint. This is not assumed to be
 * side-effect-free: claiming/configuring may cause driver-level activity. The harness
 * sends no payload bytes.
 */
class AndroidUsbTransport(
    private val manager: UsbManager,
    private val device: UsbDevice,
    override val info: UsbDeviceInfo,
) : Openable {

    override suspend fun open(baud: Int, readTimeoutMs: Int): OpenResult = withContext(Dispatchers.IO) {
        if (!manager.hasPermission(device)) {
            return@withContext OpenResult.Failed(
                ClassifiedError(TransportError.PERMISSION_DENIED, "no USB permission for ${device.deviceName}")
            )
        }
        // openDevice / claimInterface can throw (driver/permission races). Any unexpected
        // throwable is classified safely as UNKNOWN_IO_ERROR; cancellation is re-thrown.
        try {
            val (iface, inEndpoint) = findBulkInEndpoint(device)
                ?: return@withContext OpenResult.Failed(
                    ClassifiedError(TransportError.DRIVER_UNSUPPORTED, "no bulk IN endpoint on ${device.deviceName}")
                )
            val connection: UsbDeviceConnection = manager.openDevice(device)
                ?: return@withContext OpenResult.Failed(
                    ClassifiedError(TransportError.OPEN_FAILED, "openDevice returned null (busy or gone)")
                )
            // Resource-ownership guard: the connection is already open. If the claim is
            // rejected, or the session constructor fails/throws, OpenSequence releases (only
            // if claimed) and closes the connection -- no leaked handle. On success ownership
            // transfers to the session (which then owns release + close).
            //
            // baud is accepted for parity with the contract; we deliberately do NOT send a
            // SET_LINE_CODING control transfer (that would be an OUT control transfer).
            when (
                val acquired = OpenSequence.acquire(
                    claim = { connection.claimInterface(iface, true) },
                    makeSession = {
                        AndroidReadOnlySession(manager, device, connection, iface, inEndpoint, readTimeoutMs)
                    },
                    release = { connection.releaseInterface(iface) },
                    close = { connection.close() },
                )
            ) {
                is Acquired.Session -> OpenResult.Opened(acquired.session)
                Acquired.ClaimRejected -> OpenResult.Failed(
                    ClassifiedError(TransportError.PORT_BUSY, "claimInterface failed (interface busy)")
                )
            }
        } catch (e: CancellationException) {
            throw e
        } catch (t: Throwable) {
            OpenResult.Failed(TransportClassifiers.classifyThrowable(t))
        }
    }

    private fun findBulkInEndpoint(device: UsbDevice): Pair<UsbInterface, UsbEndpoint>? {
        for (i in 0 until device.interfaceCount) {
            val iface = device.getInterface(i)
            for (e in 0 until iface.endpointCount) {
                val ep = iface.getEndpoint(e)
                if (ep.type == UsbConstants.USB_ENDPOINT_XFER_BULK &&
                    ep.direction == UsbConstants.USB_DIR_IN
                ) {
                    return iface to ep
                }
            }
        }
        return null
    }
}

private class AndroidReadOnlySession(
    private val manager: UsbManager,
    private val device: UsbDevice,
    private val connection: UsbDeviceConnection,
    private val iface: UsbInterface,
    private val inEndpoint: UsbEndpoint,
    private val readTimeoutMs: Int,
) : ReadOnlySession {

    private val buffer = ByteArray(maxOf(64, inEndpoint.maxPacketSize))

    override suspend fun read(): ReadOutcome = withContext(Dispatchers.IO) {
        val start = System.nanoTime()
        try {
            // IN-direction bulk read only. The returned bytes are never inspected or logged;
            // only the count matters. A non-positive result is ambiguous on Android, so the
            // honest classification (data / inferred-timeout / io-error / disconnect) is
            // decided by the pure classifier, never asserted as a confirmed timeout here.
            val n = connection.bulkTransfer(inEndpoint, buffer, buffer.size, readTimeoutMs)
            val elapsedMs = (System.nanoTime() - start) / 1_000_000.0
            TransportClassifiers.classifyBulkRead(
                result = n,
                elapsedMs = elapsedMs,
                configuredTimeoutMs = readTimeoutMs,
                deviceStillPresent = deviceStillPresent(),
            )
        } catch (e: CancellationException) {
            throw e
        } catch (t: Throwable) {
            val elapsedMs = (System.nanoTime() - start) / 1_000_000.0
            ReadOutcome.Failed(TransportClassifiers.classifyThrowable(t), elapsedMs = elapsedMs)
        }
    }

    private fun deviceStillPresent(): Boolean =
        manager.deviceList.values.any { it.deviceId == device.deviceId }

    override fun close(): CloseOutcome {
        // Record, never swallow: releaseInterface returning false or throwing, and any
        // connection.close() throwable, are captured. close() is ALWAYS attempted even when
        // release fails. The first safe error is kept and reported to the caller.
        var firstError: ClassifiedError? = null
        try {
            if (!connection.releaseInterface(iface)) {
                firstError = ClassifiedError(
                    TransportError.UNKNOWN_IO_ERROR, "releaseInterface returned false",
                )
            }
        } catch (t: Throwable) {
            if (firstError == null) firstError = TransportClassifiers.classifyThrowable(t)
        }
        try {
            connection.close()
        } catch (t: Throwable) {
            if (firstError == null) firstError = TransportClassifiers.classifyThrowable(t)
        }
        return firstError?.let { CloseOutcome.Failed(it) } ?: CloseOutcome.Clean
    }
}
