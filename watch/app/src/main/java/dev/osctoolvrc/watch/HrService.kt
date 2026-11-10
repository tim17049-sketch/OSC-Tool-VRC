package dev.osctoolvrc.watch

import android.app.Notification
import android.app.NotificationChannel
import android.app.NotificationManager
import android.app.PendingIntent
import android.app.Service
import android.content.Context
import android.content.Intent
import android.content.pm.ServiceInfo
import android.hardware.Sensor
import android.hardware.SensorEvent
import android.hardware.SensorEventListener
import android.hardware.SensorManager
import android.net.ConnectivityManager
import android.net.Network
import android.net.NetworkCapabilities
import android.net.NetworkRequest
import android.net.nsd.NsdManager
import android.net.nsd.NsdServiceInfo
import android.os.Build
import android.os.Handler
import android.os.IBinder
import android.os.Looper
import android.os.PowerManager
import android.os.SystemClock
import androidx.core.app.NotificationCompat
import androidx.core.app.ServiceCompat
import androidx.core.content.ContextCompat
import kotlinx.coroutines.flow.update
import okhttp3.OkHttpClient
import okhttp3.Request
import okhttp3.Response
import okhttp3.WebSocket
import okhttp3.WebSocketListener
import org.json.JSONObject
import java.net.Inet4Address
import java.util.concurrent.TimeUnit
import kotlin.math.roundToInt

/**
 * Reads the heart rate sensor and streams every value to the desktop app.
 *
 * Runs as a foreground service with a wake lock so it keeps going with the
 * screen off. The connection is a WebSocket over Wi-Fi straight to the PC;
 * no phone and no cloud in between, which keeps the delay to the sensor's own
 * update rate (about one value per second).
 *
 * To save battery the sensor only runs while the PC is connected, and the
 * service ends itself when the desktop app closes or the PC has been out of
 * reach for a few minutes.
 *
 * All state is touched on the main thread only.
 */
class HrService : Service(), SensorEventListener {

    private val main = Handler(Looper.getMainLooper())
    private lateinit var sensors: SensorManager
    private lateinit var connectivity: ConnectivityManager
    private lateinit var nsd: NsdManager
    private var wakeLock: PowerManager.WakeLock? = null

    private var running = false
    private var sensorOn = false
    private var wifi: Network? = null

    private var client: OkHttpClient? = null
    private var socket: WebSocket? = null
    private var socketOpen = false
    /** Bumped for every connection so callbacks of old ones are ignored. */
    private var generation = 0

    private var discovery: NsdManager.DiscoveryListener? = null
    private var found: Target? = null

    private var bpm: Int? = null
    private var noContact = false
    private var lastEventAt = 0L
    private var lastSentAt = 0L
    private var lastStatusAt = 0L
    private var sensorName: String? = null

    override fun onBind(intent: Intent?): IBinder? = null

    override fun onCreate() {
        super.onCreate()
        sensors = getSystemService(SensorManager::class.java)
        connectivity = getSystemService(ConnectivityManager::class.java)
        nsd = getSystemService(NsdManager::class.java)
    }

    override fun onStartCommand(intent: Intent?, flags: Int, startId: Int): Int {
        if (intent?.action == ACTION_STOP) stop() else start()
        return START_NOT_STICKY
    }

    override fun onDestroy() {
        // stop() already ran if we ended ourselves; keep its reason on screen.
        if (running) stop()
        super.onDestroy()
    }

    // ---- lifecycle ---------------------------------------------------------

    private fun start() {
        if (running) return
        val type = if (Build.VERSION.SDK_INT >= 34) ServiceInfo.FOREGROUND_SERVICE_TYPE_HEALTH else 0
        ServiceCompat.startForeground(this, NOTIFICATION_ID, notification(), type)

        val sensor = sensors.getDefaultSensor(Sensor.TYPE_HEART_RATE)
        if (sensor == null) {
            stop()
            return
        }
        running = true
        sensorName = sensor.name?.takeIf { it.isNotBlank() }
        wakeLock = getSystemService(PowerManager::class.java)
            .newWakeLock(PowerManager.PARTIAL_WAKE_LOCK, "osctool:hr")
            .apply { acquire() }

        // Ask for Wi-Fi explicitly: by default a watch routes traffic through
        // the phone via Bluetooth, which is slow and cannot reach the PC.
        val request = NetworkRequest.Builder()
            .addTransportType(NetworkCapabilities.TRANSPORT_WIFI)
            .build()
        connectivity.requestNetwork(request, networkCallback)

        Repo.state.value = UiState(running = true, link = Link.NoWifi)
        main.postDelayed(ticker, 1000)
        main.postDelayed(giveUp, AUTO_STOP_MS)
    }

    private fun stop(ended: Ended? = null) {
        if (running) {
            running = false
            sensorOff()
            runCatching { connectivity.unregisterNetworkCallback(networkCallback) }
            wakeLock?.let { if (it.isHeld) it.release() }
            wakeLock = null
        }
        main.removeCallbacksAndMessages(null)
        stopDiscovery()
        closeSocket()
        wifi = null
        found = null
        bpm = null
        noContact = false
        Repo.state.value = UiState(ended = ended)
        ServiceCompat.stopForeground(this, ServiceCompat.STOP_FOREGROUND_REMOVE)
        stopSelf()
    }

    /** Nobody to send to for too long: end instead of draining the battery. */
    private val giveUp = Runnable { if (running) stop(Ended.Unreachable) }

    // ---- sensor ------------------------------------------------------------

    private fun sensorOn() {
        if (sensorOn) return
        val sensor = sensors.getDefaultSensor(Sensor.TYPE_HEART_RATE) ?: return
        sensorOn = sensors.registerListener(this, sensor, SensorManager.SENSOR_DELAY_FASTEST)
    }

    private fun sensorOff() {
        if (!sensorOn) return
        sensors.unregisterListener(this)
        sensorOn = false
        bpm = null
        noContact = false
        Repo.state.update { it.copy(bpm = null, noContact = false) }
    }

    override fun onSensorChanged(event: SensorEvent) {
        val value = event.values.firstOrNull()?.roundToInt() ?: 0
        val contact = value > 0 && event.accuracy != SensorManager.SENSOR_STATUS_NO_CONTACT
        bpm = if (contact) value else null
        noContact = !contact
        lastEventAt = SystemClock.elapsedRealtime()
        Repo.state.update { it.copy(bpm = bpm, noContact = noContact) }
        send()
    }

    override fun onAccuracyChanged(sensor: Sensor?, accuracy: Int) {}

    /** Once a second: repeat the value so the PC knows we are alive, and
     *  notice when the sensor went quiet. */
    private val ticker = object : Runnable {
        override fun run() {
            if (!running) return
            val now = SystemClock.elapsedRealtime()
            if (sensorOn && bpm != null && now - lastEventAt > SENSOR_SILENCE_MS) {
                bpm = null
                noContact = true
                Repo.state.update { it.copy(bpm = null, noContact = true) }
            }
            if (now - lastSentAt >= 950 && (bpm != null || noContact)) send()
            if (socketOpen && now - lastStatusAt >= STATUS_MS) {
                socket?.send(DeviceInfo.status(this@HrService).toString())
                lastStatusAt = now
            }
            main.postDelayed(this, 1000)
        }
    }

    private fun send() {
        if (!socketOpen) return
        socket?.send("""{"type":"hr","bpm":${bpm ?: 0}}""")
        lastSentAt = SystemClock.elapsedRealtime()
    }

    // ---- network -----------------------------------------------------------

    private val networkCallback = object : ConnectivityManager.NetworkCallback() {
        override fun onAvailable(network: Network) {
            main.post {
                if (!running) return@post
                wifi = network
                ensureLink()
            }
        }

        override fun onLost(network: Network) {
            main.post {
                if (!running || wifi != network) return@post
                wifi = null
                stopDiscovery()
                if (socketOpen) linkDown()
                closeSocket()
                found = null
                Repo.state.update { it.copy(link = Link.NoWifi, peer = null) }
            }
        }
    }

    /** Move one step closer to an open connection, whatever is missing. */
    private fun ensureLink() {
        if (!running || socket != null) return
        val network = wifi ?: run {
            Repo.state.update { it.copy(link = Link.NoWifi, peer = null) }
            return
        }
        val target = Settings.target(this) ?: found
        if (target != null) connect(network, target) else discover(network)
    }

    private val retry = Runnable { ensureLink() }

    private fun retryLater() {
        main.removeCallbacks(retry)
        main.postDelayed(retry, RETRY_MS)
    }

    private fun discover(network: Network) {
        Repo.state.update { it.copy(link = Link.Searching, peer = null) }
        if (discovery != null) return
        val listener = object : NsdManager.DiscoveryListener {
            override fun onServiceFound(info: NsdServiceInfo) = resolve(info)
            override fun onServiceLost(info: NsdServiceInfo) {}
            override fun onDiscoveryStarted(type: String) {}
            override fun onDiscoveryStopped(type: String) {}
            override fun onStopDiscoveryFailed(type: String, code: Int) {}
            override fun onStartDiscoveryFailed(type: String, code: Int) {
                main.post {
                    if (discovery === this) discovery = null
                    retryLater()
                }
            }
        }
        discovery = listener
        try {
            if (Build.VERSION.SDK_INT >= 33) {
                nsd.discoverServices(
                    SERVICE_TYPE, NsdManager.PROTOCOL_DNS_SD, network,
                    ContextCompat.getMainExecutor(this), listener,
                )
            } else {
                nsd.discoverServices(SERVICE_TYPE, NsdManager.PROTOCOL_DNS_SD, listener)
            }
        } catch (e: RuntimeException) {
            discovery = null
            retryLater()
        }
    }

    @Suppress("DEPRECATION") // resolveService/host: the replacements need API 34
    private fun resolve(info: NsdServiceInfo) {
        val listener = object : NsdManager.ResolveListener {
            override fun onResolveFailed(info: NsdServiceInfo, code: Int) {}
            override fun onServiceResolved(resolved: NsdServiceInfo) {
                val address = if (Build.VERSION.SDK_INT >= 34) {
                    resolved.hostAddresses.firstOrNull { it is Inet4Address }
                        ?: resolved.hostAddresses.firstOrNull()
                } else {
                    resolved.host
                }
                val host = address?.hostAddress?.substringBefore('%') ?: return
                main.post {
                    if (!running || socket != null) return@post
                    found = Target(host, resolved.port)
                    stopDiscovery()
                    ensureLink()
                }
            }
        }
        runCatching { nsd.resolveService(info, listener) }
    }

    private fun stopDiscovery() {
        discovery?.let { runCatching { nsd.stopServiceDiscovery(it) } }
        discovery = null
    }

    private fun connect(network: Network, target: Target) {
        Repo.state.update { it.copy(link = Link.Connecting, peer = target.host) }
        val mine = ++generation
        val http = OkHttpClient.Builder()
            .socketFactory(network.socketFactory)
            .connectTimeout(4, TimeUnit.SECONDS)
            .pingInterval(5, TimeUnit.SECONDS)
            .build()
        client = http
        val request = try {
            Request.Builder().url("ws://${target.authority}/").build()
        } catch (e: IllegalArgumentException) {
            // Unusable manual address; keep trying in case it gets corrected.
            linkLost()
            return
        }
        socket = http.newWebSocket(request, object : WebSocketListener() {
            override fun onOpen(webSocket: WebSocket, response: Response) {
                main.post {
                    if (mine != generation) return@post
                    socketOpen = true
                    main.removeCallbacks(giveUp)
                    lastEventAt = SystemClock.elapsedRealtime()
                    sensorOn()
                    webSocket.send(DeviceInfo.hello(this@HrService, sensorName).toString())
                    lastStatusAt = SystemClock.elapsedRealtime()
                    Repo.state.update { it.copy(link = Link.Connected) }
                    if (bpm != null || noContact) send()
                }
            }

            override fun onMessage(webSocket: WebSocket, text: String) {
                val name = runCatching {
                    JSONObject(text).takeIf { it.optString("type") == "welcome" }?.optString("name")
                }.getOrNull()
                if (name.isNullOrBlank()) return
                main.post {
                    if (mine == generation) Repo.state.update { it.copy(peer = name) }
                }
            }

            override fun onClosing(webSocket: WebSocket, code: Int, reason: String) {
                // The desktop app says goodbye when it quits: no point waiting.
                if (code != CLOSE_APP_QUIT) return
                main.post { if (mine == generation && running) stop(Ended.PcClosed) }
            }

            override fun onClosed(webSocket: WebSocket, code: Int, reason: String) {
                main.post { if (mine == generation) linkLost() }
            }

            override fun onFailure(webSocket: WebSocket, t: Throwable, response: Response?) {
                main.post { if (mine == generation) linkLost() }
            }
        })
    }

    private fun linkLost() {
        val wasOpen = socketOpen
        closeSocket()
        // The PC may have a new address by now; look it up again.
        found = null
        if (!running) return
        if (wasOpen) linkDown()
        Repo.state.update {
            it.copy(link = if (wifi == null) Link.NoWifi else Link.Searching, peer = null)
        }
        retryLater()
    }

    /** Connection gone: pause the sensor and start the countdown to giving up. */
    private fun linkDown() {
        sensorOff()
        main.removeCallbacks(giveUp)
        main.postDelayed(giveUp, AUTO_STOP_MS)
    }

    private fun closeSocket() {
        generation++
        socketOpen = false
        socket?.close(1000, null)
        socket = null
        client?.dispatcher?.executorService?.shutdown()
        client = null
    }

    // ---- notification ------------------------------------------------------

    private fun notification(): Notification {
        val manager = getSystemService(NotificationManager::class.java)
        manager.createNotificationChannel(
            NotificationChannel(
                CHANNEL_ID,
                getString(R.string.notification_channel),
                NotificationManager.IMPORTANCE_LOW,
            )
        )
        val open = PendingIntent.getActivity(
            this, 0, Intent(this, MainActivity::class.java), PendingIntent.FLAG_IMMUTABLE,
        )
        val stop = PendingIntent.getService(
            this, 1, Intent(this, HrService::class.java).setAction(ACTION_STOP),
            PendingIntent.FLAG_IMMUTABLE,
        )
        return NotificationCompat.Builder(this, CHANNEL_ID)
            .setSmallIcon(R.drawable.ic_pulse)
            .setContentTitle(getString(R.string.app_name))
            .setContentText(getString(R.string.notification_text))
            .setOngoing(true)
            .setContentIntent(open)
            .addAction(0, getString(R.string.stop), stop)
            .build()
    }

    companion object {
        /** Must match SERVICE_TYPE in the desktop app (core/src/hr.rs). */
        private const val SERVICE_TYPE = "_osctoolhr._tcp"
        private const val ACTION_STOP = "dev.osctoolvrc.watch.STOP"
        private const val CHANNEL_ID = "hr"
        private const val NOTIFICATION_ID = 1
        private const val RETRY_MS = 3000L
        private const val SENSOR_SILENCE_MS = 30_000L
        /** How often the battery level goes to the PC. */
        private const val STATUS_MS = 60_000L
        /** How long to keep looking for the PC before the service ends. */
        private const val AUTO_STOP_MS = 3 * 60_000L
        /** Close code the desktop app sends when it quits (core/src/hr.rs). */
        private const val CLOSE_APP_QUIT = 4000

        fun start(context: Context) {
            ContextCompat.startForegroundService(context, Intent(context, HrService::class.java))
        }

        fun stop(context: Context) {
            context.startService(Intent(context, HrService::class.java).setAction(ACTION_STOP))
        }
    }
}
