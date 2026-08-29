package com.autonomousdroneexpert.m1c.ui

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.Button
import androidx.compose.material3.Checkbox
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import com.autonomousdroneexpert.m1c.domain.SafetyAttestation
import com.autonomousdroneexpert.m1c.domain.SafetyItem

private fun label(item: SafetyItem): String = when (item) {
    SafetyItem.LIPO_DISCONNECTED -> "بطارية LiPo مفصولة ومُبعدة عن الدرون."
    SafetyItem.PROPELLERS_REMOVED -> "جميع المراوح منزوعة."
    SafetyItem.USB_ONLY -> "USB فقط — لا مصدر طاقة أو اتصال آخر."
    SafetyItem.AIRCRAFT_SECURED -> "الدرون ثابت على سطح آمن."
    SafetyItem.CONFIGURATORS_CLOSED -> "Betaflight وSpeedyBee مغلقان."
    SafetyItem.NO_FLIGHT_OR_MOTOR_TEST -> "لا اختبار طيران أو محركات."
    SafetyItem.NO_PAYLOAD_WRITES -> "الأداة لا ترسل payload bytes."
    SafetyItem.OPEN_NOT_SIDE_EFFECT_FREE -> "فتح المنفذ ليس مضمونًا أن يكون بلا آثار جانبية."
}

@Composable
fun SafetyGateScreen(
    attestation: SafetyAttestation,
    onToggle: (SafetyItem, Boolean) -> Unit,
    onProceed: () -> Unit,
) {
    Column(
        modifier = Modifier.fillMaxSize().padding(20.dp).verticalScroll(rememberScrollState()),
        verticalArrangement = Arrangement.spacedBy(10.dp),
    ) {
        Text("بوابة السلامة", style = MaterialTheme.typography.headlineSmall, fontWeight = FontWeight.Bold)
        Text("يجب الموافقة على كل بند قبل تمكين أي فتح للجهاز.", style = MaterialTheme.typography.bodyMedium)

        for (item in SafetyItem.entries) {
            Row(verticalAlignment = androidx.compose.ui.Alignment.CenterVertically) {
                Checkbox(
                    checked = attestation.accepted.contains(item),
                    onCheckedChange = { onToggle(item, it) },
                )
                Text(label(item), Modifier.padding(start = 4.dp))
            }
        }

        Button(onClick = onProceed, enabled = attestation.allAccepted) {
            Text(if (attestation.allAccepted) "تأكيد ومتابعة" else "أكمل جميع البنود للمتابعة")
        }
    }
}
