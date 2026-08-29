package com.autonomousdroneexpert.m1c.ui

import android.app.Application
import android.content.Context
import android.hardware.usb.UsbDevice
import android.hardware.usb.UsbManager
import android.os.Build
import android.os.SystemClock
import androidx.lifecycle.AndroidViewModel
import androidx.lifecycle.viewModelScope
import com.autonomousdroneexpert.m1c.BuildConfig
import com.autonomousdroneexpert.m1c.domain.HardwareObservation
import com.autonomousdroneexpert.m1c.domain.HardwareTestReport
import com.autonomousdroneexpert.m1c.domain.Openable
import com.autonomousdroneexpert.m1c.domain.ReadOnlyHardwareTestRunner
import com.autonomousdroneexpert.m1c.domain.ReportEnvironment
import com.autonomousdroneexpert.m1c.domain.SafetyAttestation
import com.autonomousdroneexpert.m1c.domain.SafetyItem
import com.autonomousdroneexpert.m1c.domain.UsbDeviceInfo
import com.autonomousdroneexpert.m1c.platform.AndroidUsbDiscovery
import com.autonomousdroneexpert.m1c.platform.AndroidUsbTransport
import com.autonomousdroneexpert.m1c.platform.UsbPermissionController
import java.io.File
import kotlinx.coroutines.Job
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.launch

data class HarnessState(
    val attestation: SafetyAttestation = SafetyAttestation(),
    val devices: List<UsbDeviceInfo> = emptyList(),
    val selectedDeviceId: Int? = null,
    val observations: List<HardwareObservation> = emptyList(),
    val running: String? = null,
    val message: String? = null,
    val exportPaths: List<String> = emptyList(),
)

class HarnessViewModel(app: Application) : AndroidViewModel(app) {
    private val manager = app.getSystemService(Context.USB_SERVICE) as UsbManager
    private val discovery = AndroidUsbDiscovery(manager)
    private val permission = UsbPermissionController(app, manager)
    private val runner = ReadOnlyHardwareTestRunner(clock = { SystemClock.elapsedRealtime() })

    private val _state = MutableStateFlow(HarnessState())
    val state: StateFlow<HarnessState> = _state.asStateFlow()

    private var testJob: Job? = null

    fun toggleSafety(item: SafetyItem, on: Boolean) {
        _state.value = _state.value.copy(
            attestation = _state.value.attestation.toggle(item, on, System.currentTimeMillis())
        )
    }

    fun refreshDevices() {
        _state.value = _state.value.copy(devices = discovery.listDevices(), message = null)
    }

    fun select(deviceId: Int) {
        _state.value = _state.value.copy(selectedDeviceId = deviceId)
    }

    fun requestPermission() {
        val dev = rawDevice() ?: return
        permission.request(dev) { granted ->
            _state.value = _state.value.copy(message = if (granted) "USB permission granted" else "USB permission denied")
            refreshDevices()
        }
    }

    fun runSingleOpen() = launchTest("single-open") {
        runner.singleOpen(it, 115_200, 250, onPortOpen = {
            _state.value = _state.value.copy(
                message = "المنفذ مفتوح الآن — راقب مؤشرات LED وأي إعادة تعداد على اللوحة",
            )
        })
    }
    fun runOpenClose() = launchTest("open-close x20") { runner.openCloseCycles(it, 20, 115_200, 250) }
    fun runReadTimeout() = launchTest("read-timeout") { runner.readTimeoutAccuracy(it, 250, 100) }
    fun runUnplug() = launchTest("unplug-detection") { runner.unplugDetection(it, 1000, 120) }

    fun stop() {
        testJob?.cancel()
    }

    private fun launchTest(name: String, block: suspend (Openable) -> HardwareObservation) {
        if (!_state.value.attestation.allAccepted) {
            _state.value = _state.value.copy(message = "أكمل بوابة السلامة أولًا")
            return
        }
        val openable = buildOpenable()
        if (openable == null) {
            _state.value = _state.value.copy(message = "اختر جهازًا مُصرّحًا أولًا")
            return
        }
        testJob?.cancel()
        _state.value = _state.value.copy(running = name, message = null)
        testJob = viewModelScope.launch {
            try {
                val obs = block(openable)
                _state.value = _state.value.copy(
                    observations = _state.value.observations + obs,
                    running = null,
                )
            } catch (e: kotlinx.coroutines.CancellationException) {
                // Do not hide cancellation: clear the running flag, note it, then re-throw so
                // the coroutine is properly cancelled. Coroutine cancellation is NOT proof the
                // driver read was cancelled.
                _state.value = _state.value.copy(
                    running = null,
                    message = "أُلغي الاختبار (إلغاء coroutine؛ ليس دليلًا على إلغاء I/O في الـdriver)",
                )
                throw e
            } catch (t: Throwable) {
                // Any unexpected error must free the UI, not leave it stuck on "running".
                _state.value = _state.value.copy(
                    running = null,
                    message = "خطأ غير متوقع (${t.javaClass.simpleName}): ${t.message ?: "بدون رسالة"}",
                )
            }
        }
    }

    private fun rawDevice(): UsbDevice? {
        val id = _state.value.selectedDeviceId ?: return null
        return manager.deviceList.values.firstOrNull { it.deviceId == id }
    }

    private fun buildOpenable(): Openable? {
        val dev = rawDevice() ?: return null
        val info = _state.value.devices.firstOrNull { it.androidDeviceId == dev.deviceId } ?: return null
        if (!manager.hasPermission(dev)) return null
        return AndroidUsbTransport(manager, dev, info)
    }

    fun exportReport() {
        val info = _state.value.devices.firstOrNull { it.androidDeviceId == _state.value.selectedDeviceId }
        val report = HardwareTestReport(
            environment = ReportEnvironment(
                appVersion = BuildConfig.VERSION_NAME,
                sourceSha = BuildConfig.SOURCE_SHA,
                androidVersion = "Android ${Build.VERSION.RELEASE} (SDK ${Build.VERSION.SDK_INT})",
                phoneModel = "${Build.MANUFACTURER} ${Build.MODEL}",
                applicationId = BuildConfig.APPLICATION_ID,
            ),
            device = info,
            safetyAttestedAtEpochMillis = _state.value.attestation.attestedAtEpochMillis,
            testParameters = mapOf(
                "baud" to "115200",
                "readTimeoutMs" to "250",
                "openCloseCycles" to "20",
                "readTimeoutSamples" to "100",
            ),
            observations = _state.value.observations,
        )
        val dir = File(getApplication<Application>().cacheDir, "reports").apply { mkdirs() }
        val json = File(dir, "m1c-report.json").apply { writeText(report.toJson()) }
        val txt = File(dir, "m1c-report.txt").apply { writeText(report.toPlainText()) }
        _state.value = _state.value.copy(
            exportPaths = listOf(json.absolutePath, txt.absolutePath),
            message = "حُفظ التقرير في ذاكرة التطبيق المؤقتة",
        )
    }

    /** Plain-text report for the Android share sheet (no file provider, no permission). */
    fun shareText(): String {
        val info = _state.value.devices.firstOrNull { it.androidDeviceId == _state.value.selectedDeviceId }
        return HardwareTestReport(
            environment = ReportEnvironment(
                appVersion = BuildConfig.VERSION_NAME,
                sourceSha = BuildConfig.SOURCE_SHA,
                androidVersion = "Android ${Build.VERSION.RELEASE} (SDK ${Build.VERSION.SDK_INT})",
                phoneModel = "${Build.MANUFACTURER} ${Build.MODEL}",
                applicationId = BuildConfig.APPLICATION_ID,
            ),
            device = info,
            safetyAttestedAtEpochMillis = _state.value.attestation.attestedAtEpochMillis,
            testParameters = mapOf("baud" to "115200", "readTimeoutMs" to "250"),
            observations = _state.value.observations,
        ).toPlainText()
    }

    override fun onCleared() {
        permission.unregister()
        super.onCleared()
    }
}
