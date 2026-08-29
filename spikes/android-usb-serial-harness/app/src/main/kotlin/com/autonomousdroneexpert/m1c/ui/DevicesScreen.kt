package com.autonomousdroneexpert.m1c.ui

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.Button
import androidx.compose.material3.Card
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import com.autonomousdroneexpert.m1c.domain.UsbDeviceInfo

@Composable
fun DevicesScreen(
    devices: List<UsbDeviceInfo>,
    selectedId: Int?,
    message: String?,
    onRefresh: () -> Unit,
    onSelect: (Int) -> Unit,
    onRequestPermission: () -> Unit,
    onContinue: () -> Unit,
) {
    Column(
        Modifier.fillMaxSize().padding(20.dp).verticalScroll(rememberScrollState()),
        verticalArrangement = Arrangement.spacedBy(10.dp),
    ) {
        Text("أجهزة USB", style = MaterialTheme.typography.headlineSmall, fontWeight = FontWeight.Bold)
        Text("firmware identity: UNKNOWN — MSP PROHIBITED", style = MaterialTheme.typography.bodySmall)

        Column(verticalArrangement = Arrangement.spacedBy(6.dp)) {
            Button(onClick = onRefresh, modifier = Modifier.fillMaxWidth()) { Text("تحديث الأجهزة") }
            Button(onClick = onRequestPermission, modifier = Modifier.fillMaxWidth()) { Text("طلب إذن USB") }
        }

        if (message != null) Text(message, color = MaterialTheme.colorScheme.primary)

        if (devices.isEmpty()) {
            Text("لا توجد أجهزة USB. وصّل الجهاز واضغط تحديث.")
        } else {
            for (d in devices) {
                Card(Modifier.fillMaxWidth()) {
                    Column(Modifier.padding(12.dp), verticalArrangement = Arrangement.spacedBy(2.dp)) {
                        Text(d.deviceName, fontWeight = FontWeight.Bold)
                        Text("VID ${d.vidHex()}  PID ${d.pidHex()}")
                        Text("manufacturer: ${d.manufacturer ?: "-"}")
                        Text("product: ${d.product ?: "-"}")
                        Text("serial: ${d.serial ?: "-"} (USB descriptor only)")
                        Text("android id: ${d.androidDeviceId}")
                        Text("permission: ${if (d.permissionGranted) "granted" else "not granted"}")
                        Text("driver: ${d.driverMatch}")
                        OutlinedButton(onClick = { onSelect(d.androidDeviceId) }) {
                            Text(if (selectedId == d.androidDeviceId) "محدَّد ✓" else "اختيار")
                        }
                    }
                }
            }
        }

        Button(onClick = onContinue, enabled = selectedId != null, modifier = Modifier.fillMaxWidth()) {
            Text("متابعة إلى الاختبارات")
        }
    }
}
