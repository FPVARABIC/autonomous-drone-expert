package com.autonomousdroneexpert.m1c.platform

import android.hardware.usb.UsbManager
import com.autonomousdroneexpert.m1c.domain.UsbDeviceInfo
import com.autonomousdroneexpert.m1c.domain.UsbDiscovery

/** Lists real attached USB devices via Android's UsbManager. Metadata only. */
class AndroidUsbDiscovery(private val manager: UsbManager) : UsbDiscovery {
    override fun listDevices(): List<UsbDeviceInfo> =
        manager.deviceList.values.map { d ->
            val serial: String? = try {
                // May require permission; absence is normal and never fabricated.
                d.serialNumber
            } catch (_: SecurityException) {
                null
            }
            UsbDeviceInfo(
                deviceName = d.deviceName,
                vid = d.vendorId,
                pid = d.productId,
                manufacturer = d.manufacturerName,
                product = d.productName,
                serial = serial,
                androidDeviceId = d.deviceId,
                permissionGranted = manager.hasPermission(d),
                driverMatch = describeDriverMatch(d.interfaceCount, firstInterfaceClass(d)),
            )
        }.sortedBy { it.deviceName }

    private fun firstInterfaceClass(d: android.hardware.usb.UsbDevice): Int =
        if (d.interfaceCount > 0) d.getInterface(0).interfaceClass else -1

    private fun describeDriverMatch(interfaceCount: Int, firstClass: Int): String =
        "interfaces=$interfaceCount firstClass=$firstClass"
}
