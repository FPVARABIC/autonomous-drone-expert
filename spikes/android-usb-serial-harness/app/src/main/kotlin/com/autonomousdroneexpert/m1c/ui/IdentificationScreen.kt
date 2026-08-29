package com.autonomousdroneexpert.m1c.ui

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxSize
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
import com.autonomousdroneexpert.m1c.BuildConfig

@Composable
fun IdentificationScreen(onContinue: () -> Unit) {
    Column(
        modifier = Modifier.fillMaxSize().padding(20.dp).verticalScroll(rememberScrollState()),
        verticalArrangement = Arrangement.spacedBy(14.dp),
    ) {
        Text("خبير الدرونات — مختبر العتاد", style = MaterialTheme.typography.headlineSmall, fontWeight = FontWeight.Bold)
        Text("M1C — Android USB Serial Hardware Harness", style = MaterialTheme.typography.bodyMedium)

        Card {
            Column(Modifier.padding(16.dp), verticalArrangement = Arrangement.spacedBy(6.dp)) {
                Text(BuildConfig.SPIKE_LABEL, fontWeight = FontWeight.Bold, color = MaterialTheme.colorScheme.error)
                Text("لا تستخدم هذه الأداة لإعداد الطيران. أداة اختبار عتاد قراءةً فقط.")
                Text("DO NOT USE FOR FLIGHT CONFIGURATION", style = MaterialTheme.typography.bodySmall)
            }
        }

        Card {
            Column(Modifier.padding(16.dp), verticalArrangement = Arrangement.spacedBy(4.dp)) {
                Text("معلومات الإصدار", fontWeight = FontWeight.Bold)
                Text("Build type: debug (review only)")
                Text("Source commit: ${BuildConfig.SOURCE_SHA}")
                Text("Application ID: ${BuildConfig.APPLICATION_ID}")
                Text("Version: ${BuildConfig.VERSION_NAME}")
                Text("الحالة: REQUIRES HARDWARE TEST")
            }
        }

        Card {
            Column(Modifier.padding(16.dp), verticalArrangement = Arrangement.spacedBy(4.dp)) {
                Text("ماذا تفعل هذه الأداة", fontWeight = FontWeight.Bold)
                Text("• تُعدّد أجهزة USB وتقرأ بياناتها الوصفية.")
                Text("• تفتح المنفذ (baud/timeout) وتقرأ وتغلق.")
                Text("• لا ترسل أي payload bytes، ولا MSP، ولا CLI.")
                Text("• فتح المنفذ ليس مضمونًا أن يكون بلا آثار جانبية.")
            }
        }

        Button(onClick = onContinue) { Text("متابعة إلى بوابة السلامة") }
    }
}
