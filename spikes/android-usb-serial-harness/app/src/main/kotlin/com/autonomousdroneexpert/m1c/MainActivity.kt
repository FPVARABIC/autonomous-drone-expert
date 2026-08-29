package com.autonomousdroneexpert.m1c

import android.content.Intent
import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.material3.Surface
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalLayoutDirection
import androidx.compose.ui.unit.LayoutDirection
import androidx.lifecycle.viewmodel.compose.viewModel
import com.autonomousdroneexpert.m1c.domain.SafetyItem
import com.autonomousdroneexpert.m1c.ui.DevicesScreen
import com.autonomousdroneexpert.m1c.ui.HarnessViewModel
import com.autonomousdroneexpert.m1c.ui.IdentificationScreen
import com.autonomousdroneexpert.m1c.ui.M1CTheme
import com.autonomousdroneexpert.m1c.ui.ResultsScreen
import com.autonomousdroneexpert.m1c.ui.SafetyGateScreen
import com.autonomousdroneexpert.m1c.ui.Screen
import com.autonomousdroneexpert.m1c.ui.TestsScreen

class MainActivity : ComponentActivity() {
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        setContent {
            M1CTheme {
                CompositionLocalProvider(LocalLayoutDirection provides LayoutDirection.Rtl) {
                    Surface(modifier = Modifier.fillMaxSize()) {
                        AppRoot(onShare = ::shareText)
                    }
                }
            }
        }
    }

    private fun shareText(text: String) {
        val intent = Intent(Intent.ACTION_SEND).apply {
            type = "text/plain"
            putExtra(Intent.EXTRA_TEXT, text)
        }
        startActivity(Intent.createChooser(intent, "مشاركة تقرير M1C"))
    }
}

@androidx.compose.runtime.Composable
private fun AppRoot(onShare: (String) -> Unit) {
    val vm: HarnessViewModel = viewModel()
    val state by vm.state.collectAsState()
    var screen by remember { mutableStateOf(Screen.Identification) }

    Box(Modifier.fillMaxSize()) {
        when (screen) {
            Screen.Identification -> IdentificationScreen(onContinue = { screen = Screen.SafetyGate })

            Screen.SafetyGate -> SafetyGateScreen(
                attestation = state.attestation,
                onToggle = { item: SafetyItem, on: Boolean -> vm.toggleSafety(item, on) },
                onProceed = { vm.refreshDevices(); screen = Screen.Devices },
            )

            Screen.Devices -> DevicesScreen(
                devices = state.devices,
                selectedId = state.selectedDeviceId,
                message = state.message,
                onRefresh = vm::refreshDevices,
                onSelect = vm::select,
                onRequestPermission = vm::requestPermission,
                onContinue = { screen = Screen.Tests },
            )

            Screen.Tests -> TestsScreen(
                running = state.running,
                message = state.message,
                onSingleOpen = vm::runSingleOpen,
                onOpenClose = vm::runOpenClose,
                onReadTimeout = vm::runReadTimeout,
                onUnplug = vm::runUnplug,
                onStop = vm::stop,
                onResults = { screen = Screen.Results },
            )

            Screen.Results -> ResultsScreen(
                observations = state.observations,
                exportPaths = state.exportPaths,
                message = state.message,
                onExport = vm::exportReport,
                onShare = { onShare(vm.shareText()) },
            )
        }
    }
}
