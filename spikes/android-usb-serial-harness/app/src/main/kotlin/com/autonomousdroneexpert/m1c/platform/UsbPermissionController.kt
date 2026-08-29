package com.autonomousdroneexpert.m1c.platform

import android.app.PendingIntent
import android.content.BroadcastReceiver
import android.content.Context
import android.content.Intent
import android.content.IntentFilter
import android.hardware.usb.UsbDevice
import android.hardware.usb.UsbManager
import android.os.Build

/**
 * Wraps the Android USB runtime-permission flow. Requesting permission never opens the
 * device: opening is a separate, explicit user action (per the protocol).
 */
class UsbPermissionController(
    private val context: Context,
    private val manager: UsbManager,
) {
    private val action = context.packageName + ".USB_PERMISSION"
    private var receiver: BroadcastReceiver? = null

    fun hasPermission(device: UsbDevice): Boolean = manager.hasPermission(device)

    fun request(device: UsbDevice, onResult: (granted: Boolean) -> Unit) {
        unregister()
        val r = object : BroadcastReceiver() {
            override fun onReceive(c: Context?, intent: Intent?) {
                if (intent?.action == action) {
                    val granted = intent.getBooleanExtra(UsbManager.EXTRA_PERMISSION_GRANTED, false)
                    unregister()
                    onResult(granted)
                }
            }
        }
        receiver = r
        val flags =
            if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.S) PendingIntent.FLAG_IMMUTABLE else 0
        val pi = PendingIntent.getBroadcast(context, 0, Intent(action).setPackage(context.packageName), flags)
        val filter = IntentFilter(action)
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU) {
            context.registerReceiver(r, filter, Context.RECEIVER_NOT_EXPORTED)
        } else {
            @Suppress("UnspecifiedRegisterReceiverFlag")
            context.registerReceiver(r, filter)
        }
        manager.requestPermission(device, pi)
    }

    fun unregister() {
        receiver?.let {
            try {
                context.unregisterReceiver(it)
            } catch (_: IllegalArgumentException) {
                // not registered; ignore
            }
        }
        receiver = null
    }
}
