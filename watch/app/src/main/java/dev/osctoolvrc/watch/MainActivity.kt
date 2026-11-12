package dev.osctoolvrc.watch

import android.Manifest
import android.content.pm.PackageManager
import android.hardware.Sensor
import android.hardware.SensorManager
import android.os.Build
import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.compose.BackHandler
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.compose.setContent
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.BasicTextField
import androidx.compose.foundation.text.KeyboardActions
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.runtime.Composable
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.SolidColor
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.core.content.ContextCompat
import androidx.wear.compose.foundation.lazy.ScalingLazyColumn
import androidx.wear.compose.foundation.lazy.rememberScalingLazyListState
import androidx.wear.compose.material.Chip
import androidx.wear.compose.material.ChipDefaults
import androidx.wear.compose.material.MaterialTheme
import androidx.wear.compose.material.PositionIndicator
import androidx.wear.compose.material.Scaffold
import androidx.wear.compose.material.Text
import androidx.wear.compose.material.TimeText

private val Pulse = Color(0xFFFF5C8A)
private val Signal = Color(0xFF6EE7C8)
private val Ink = Color(0xFF15122A)
private val Muted = Color(0xFFA69FC8)

class MainActivity : ComponentActivity() {
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        setContent {
            MaterialTheme(
                colors = MaterialTheme.colors.copy(
                    primary = Pulse,
                    onPrimary = Color(0xFF2A0B18),
                    surface = Color(0xFF272247),
                    onSurface = Color.White,
                    background = Color.Black,
                ),
            ) { App() }
        }
    }
}

@Composable
private fun App() {
    val context = LocalContext.current
    val state by Repo.state.collectAsState()
    var editing by remember { mutableStateOf(false) }
    var denied by remember { mutableStateOf(false) }
    var host by remember { mutableStateOf(Settings.host(context)) }
    val hasSensor = remember {
        context.getSystemService(SensorManager::class.java)
            .getDefaultSensor(Sensor.TYPE_HEART_RATE) != null
    }

    val permissions = rememberLauncherForActivityResult(
        ActivityResultContracts.RequestMultiplePermissions(),
    ) {
        val granted = ContextCompat.checkSelfPermission(context, Manifest.permission.BODY_SENSORS) ==
            PackageManager.PERMISSION_GRANTED
        denied = !granted
        if (granted) HrService.start(context)
    }

    if (editing) {
        BackHandler { editing = false }
        AddressScreen(
            initial = host,
            onSave = {
                Settings.setHost(context, it)
                host = Settings.host(context)
                editing = false
            },
        )
        return
    }

    MainScreen(
        state = state,
        host = host,
        message = when {
            !hasSensor -> stringResource(R.string.no_sensor)
            denied -> stringResource(R.string.permission_needed)
            else -> null
        },
        canStart = hasSensor,
        onToggle = {
            if (state.running) {
                HrService.stop(context)
            } else {
                val wanted = buildList {
                    add(Manifest.permission.BODY_SENSORS)
                    if (Build.VERSION.SDK_INT >= 33) add(Manifest.permission.POST_NOTIFICATIONS)
                }
                permissions.launch(wanted.toTypedArray())
            }
        },
        onEditAddress = { editing = true },
    )
}

@Composable
private fun MainScreen(
    state: UiState,
    host: String,
    message: String?,
    canStart: Boolean,
    onToggle: () -> Unit,
    onEditAddress: () -> Unit,
) {
    val listState = rememberScalingLazyListState()
    Scaffold(
        timeText = { TimeText() },
        positionIndicator = { PositionIndicator(scalingLazyListState = listState) },
    ) {
        ScalingLazyColumn(
            modifier = Modifier.fillMaxSize(),
            state = listState,
            horizontalAlignment = Alignment.CenterHorizontally,
        ) {
            item {
                Row(verticalAlignment = Alignment.Bottom) {
                    Text(
                        text = state.bpm?.toString() ?: "--",
                        color = if (state.bpm != null) Pulse else Muted,
                        fontSize = 54.sp,
                        fontWeight = FontWeight.Bold,
                    )
                    Text(
                        text = stringResource(R.string.bpm),
                        color = Muted,
                        fontSize = 14.sp,
                        modifier = Modifier.padding(start = 6.dp, bottom = 10.dp),
                    )
                }
            }
            item {
                Text(
                    text = message ?: status(state),
                    color = if (state.link == Link.Connected && state.bpm != null) Signal else Muted,
                    fontSize = 13.sp,
                    textAlign = TextAlign.Center,
                    modifier = Modifier
                        .fillMaxWidth()
                        .padding(horizontal = 12.dp, vertical = 4.dp),
                )
            }
            item {
                Chip(
                    onClick = onToggle,
                    enabled = canStart,
                    label = {
                        Text(
                            text = stringResource(if (state.running) R.string.stop else R.string.start),
                            textAlign = TextAlign.Center,
                            modifier = Modifier.fillMaxWidth(),
                        )
                    },
                    colors = if (state.running) ChipDefaults.secondaryChipColors() else ChipDefaults.primaryChipColors(),
                    modifier = Modifier.fillMaxWidth(),
                )
            }
            item {
                Chip(
                    onClick = onEditAddress,
                    // Changing the address mid-stream would need a reconnect.
                    enabled = !state.running,
                    label = {
                        Text(
                            text = if (host.isBlank()) stringResource(R.string.pc_auto)
                            else stringResource(R.string.pc_manual, host),
                            maxLines = 2,
                        )
                    },
                    colors = ChipDefaults.secondaryChipColors(),
                    modifier = Modifier.fillMaxWidth(),
                )
            }
        }
    }
}

@Composable
private fun status(state: UiState): String = when {
    !state.running -> when (state.ended) {
        Ended.PcClosed -> stringResource(R.string.status_ended_pc_closed)
        Ended.Unreachable -> stringResource(R.string.status_ended_unreachable)
        null -> stringResource(R.string.status_idle)
    }
    state.noContact -> stringResource(R.string.status_no_contact)
    else -> when (state.link) {
        Link.Idle, Link.NoWifi -> stringResource(R.string.status_no_wifi)
        Link.Searching -> stringResource(R.string.status_searching)
        Link.Connecting -> stringResource(R.string.status_connecting, state.peer ?: "PC")
        Link.Connected -> stringResource(R.string.status_connected, state.peer ?: "PC")
    }
}

@Composable
private fun AddressScreen(initial: String, onSave: (String) -> Unit) {
    var text by remember { mutableStateOf(initial) }
    // Empty means "find automatically"; anything else has to parse.
    val valid = text.isBlank() || Settings.parse(text) != null

    Column(
        modifier = Modifier
            .fillMaxSize()
            .background(Color.Black)
            .padding(horizontal = 22.dp),
        verticalArrangement = Arrangement.spacedBy(6.dp, Alignment.CenterVertically),
        horizontalAlignment = Alignment.CenterHorizontally,
    ) {
        Text(stringResource(R.string.pc_title), fontSize = 14.sp, fontWeight = FontWeight.Bold)
        BasicTextField(
            value = text,
            onValueChange = { text = it },
            singleLine = true,
            textStyle = TextStyle(color = Color.White, fontSize = 16.sp, textAlign = TextAlign.Center),
            cursorBrush = SolidColor(Pulse),
            keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Uri, imeAction = ImeAction.Done),
            keyboardActions = KeyboardActions(onDone = { if (valid) onSave(text) }),
            decorationBox = { field ->
                Column(
                    modifier = Modifier
                        .fillMaxWidth()
                        .background(Ink, RoundedCornerShape(12.dp))
                        .padding(horizontal = 10.dp, vertical = 10.dp),
                    horizontalAlignment = Alignment.CenterHorizontally,
                ) {
                    if (text.isEmpty()) {
                        Text(stringResource(R.string.pc_hint), color = Muted, fontSize = 14.sp)
                    }
                    field()
                }
            },
        )
        Text(
            stringResource(R.string.pc_help),
            color = Muted,
            fontSize = 11.sp,
            textAlign = TextAlign.Center,
        )
        Chip(
            onClick = { onSave(text) },
            enabled = valid,
            label = {
                Text(
                    stringResource(R.string.save),
                    textAlign = TextAlign.Center,
                    modifier = Modifier.fillMaxWidth(),
                )
            },
            colors = ChipDefaults.primaryChipColors(),
            modifier = Modifier.fillMaxWidth(),
        )
    }
}
