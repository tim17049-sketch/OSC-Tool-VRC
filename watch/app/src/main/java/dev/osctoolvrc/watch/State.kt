package dev.osctoolvrc.watch

import android.content.Context
import kotlinx.coroutines.flow.MutableStateFlow

/** Where the connection to the PC stands. */
enum class Link { Idle, NoWifi, Searching, Connecting, Connected }

/** Why the service ended by itself. */
enum class Ended { PcClosed, Unreachable }

data class UiState(
    val running: Boolean = false,
    /** Current heart rate; null while there is no reading. */
    val bpm: Int? = null,
    /** The sensor is on but has no skin contact. */
    val noContact: Boolean = false,
    val link: Link = Link.Idle,
    /** Name or address of the PC we are talking to. */
    val peer: String? = null,
    /** Set when the service stopped on its own, to tell the user why. */
    val ended: Ended? = null,
)

/** Shared between the service (writes) and the screen (reads). */
object Repo {
    val state = MutableStateFlow(UiState())
}

/** Address of the PC: host plus port of the desktop app's receiver. */
data class Target(val host: String, val port: Int) {
    /** For a ws:// URL; IPv6 literals need brackets. */
    val authority: String
        get() = if (':' in host) "[$host]:$port" else "$host:$port"
}

object Settings {
    const val DEFAULT_PORT = 9555
    private const val PREFS = "settings"
    private const val KEY_HOST = "host"

    /** What the user typed, or "" for automatic discovery. */
    fun host(context: Context): String =
        context.getSharedPreferences(PREFS, Context.MODE_PRIVATE).getString(KEY_HOST, "") ?: ""

    fun setHost(context: Context, host: String) {
        context.getSharedPreferences(PREFS, Context.MODE_PRIVATE)
            .edit().putString(KEY_HOST, host.trim()).apply()
    }

    /** Manually entered address, e.g. "192.168.1.20" or "192.168.1.20:9555". */
    fun target(context: Context): Target? = parse(host(context))

    fun parse(input: String): Target? {
        val text = input.trim().removePrefix("ws://").trimEnd('/')
        if (text.isEmpty() || text.any { it.isWhitespace() }) return null
        val colon = text.lastIndexOf(':')
        // One colon: host:port. More than one would be a bare IPv6 address.
        if (colon > 0 && text.indexOf(':') == colon) {
            val port = text.substring(colon + 1).toIntOrNull() ?: return null
            if (port !in 1..65535) return null
            return Target(text.substring(0, colon), port)
        }
        return Target(text, DEFAULT_PORT)
    }
}
