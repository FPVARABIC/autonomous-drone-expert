package com.autonomousdroneexpert.m1c.domain

/**
 * USB device metadata as the harness can see it. Every optional field is genuinely
 * optional; missing metadata is normal and must never be fabricated. No payload content
 * is ever represented here.
 */
data class UsbDeviceInfo(
    val deviceName: String,
    val vid: Int?,
    val pid: Int?,
    val manufacturer: String?,
    val product: String?,
    val serial: String?,
    val androidDeviceId: Int,
    val permissionGranted: Boolean,
    val driverMatch: String,
) {
    fun vidHex(): String = vid?.let { "%04x".format(it) } ?: "-"
    fun pidHex(): String = pid?.let { "%04x".format(it) } ?: "-"
}

/** Enumeration source. The Android implementation lists real devices only. */
interface UsbDiscovery {
    fun listDevices(): List<UsbDeviceInfo>
}
