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
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import com.autonomousdroneexpert.m1c.domain.HardwareObservation

@Composable
fun ResultsScreen(
    observations: List<HardwareObservation>,
    exportPaths: List<String>,
    message: String?,
    onExport: () -> Unit,
    onShare: () -> Unit,
) {
    Column(
        Modifier.fillMaxSize().padding(20.dp).verticalScroll(rememberScrollState()),
        verticalArrangement = Arrangement.spacedBy(8.dp),
    ) {
        Text("النتائج", style = MaterialTheme.typography.headlineSmall, fontWeight = FontWeight.Bold)
        Text("الحالة النهائية: REQUIRES HARDWARE TEST — SPIKE", style = MaterialTheme.typography.bodySmall)

        if (observations.isEmpty()) {
            Text("لا نتائج بعد. شغّل اختبارًا أولًا.")
        } else {
            for (o in observations) {
                Card(Modifier.fillMaxWidth()) {
                    Column(Modifier.padding(12.dp), verticalArrangement = Arrangement.spacedBy(2.dp)) {
                        Text("${o.stage} — ${o.status}", fontWeight = FontWeight.Bold)
                        Text(o.detail)
                        o.timeoutStats?.let {
                            Text("min=${it.minMs} median=${it.medianMs} p95=${it.p95Ms} max=${it.maxMs} n=${it.samples}")
                        }
                        Text("at ${o.atElapsedMillis} ms", style = MaterialTheme.typography.bodySmall)
                    }
                }
            }
        }

        Button(onClick = onExport, modifier = Modifier.fillMaxWidth()) { Text("حفظ التقرير (JSON + نص)") }
        Button(onClick = onShare, modifier = Modifier.fillMaxWidth()) { Text("مشاركة كنص") }

        if (message != null) Text(message, color = MaterialTheme.colorScheme.primary)
        for (p in exportPaths) Text(p, style = MaterialTheme.typography.bodySmall)
    }
}
