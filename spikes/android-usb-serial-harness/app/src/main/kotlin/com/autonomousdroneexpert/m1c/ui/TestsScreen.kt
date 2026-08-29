package com.autonomousdroneexpert.m1c.ui

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.Button
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp

@Composable
fun TestsScreen(
    running: String?,
    message: String?,
    onSingleOpen: () -> Unit,
    onOpenClose: () -> Unit,
    onReadTimeout: () -> Unit,
    onUnplug: () -> Unit,
    onStop: () -> Unit,
    onResults: () -> Unit,
) {
    Column(
        Modifier.fillMaxSize().padding(20.dp).verticalScroll(rememberScrollState()),
        verticalArrangement = Arrangement.spacedBy(8.dp),
    ) {
        Text("اختبارات العتاد (قراءة فقط)", style = MaterialTheme.typography.headlineSmall, fontWeight = FontWeight.Bold)
        Text("لا ترسل الأداة أي payload bytes. فتح المنفذ ليس مضمونًا أن يكون بلا آثار جانبية.", style = MaterialTheme.typography.bodySmall)

        val busy = running != null
        Button(onClick = onSingleOpen, enabled = !busy, modifier = Modifier.fillMaxWidth()) { Text("فتح واحد وملاحظة") }
        Button(onClick = onOpenClose, enabled = !busy, modifier = Modifier.fillMaxWidth()) { Text("فتح/إغلاق 20 دورة") }
        Button(onClick = onReadTimeout, enabled = !busy, modifier = Modifier.fillMaxWidth()) { Text("دقة مهلة القراءة") }
        Button(onClick = onUnplug, enabled = !busy, modifier = Modifier.fillMaxWidth()) { Text("اكتشاف نزع الكابل") }
        OutlinedButton(onClick = onStop, enabled = busy, modifier = Modifier.fillMaxWidth()) { Text("إيقاف الاختبار الحالي") }

        if (running != null) Text("جارٍ: $running", color = MaterialTheme.colorScheme.primary)
        if (message != null) Text(message)

        Button(onClick = onResults, modifier = Modifier.fillMaxWidth()) { Text("عرض النتائج والتصدير") }
    }
}
