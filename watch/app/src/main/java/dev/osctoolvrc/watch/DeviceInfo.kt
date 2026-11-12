package dev.osctoolvrc.watch

import android.content.Context
import android.content.pm.PackageManager
import android.os.BatteryManager
import android.os.Build
import org.json.JSONObject

/**
 * What the watch tells the PC about itself when it connects, so the desktop
 * app can show the right name for any watch, older or newer. The PC treats
 * every field as optional and ignores fields it does not know.
 */
object DeviceInfo {

    /** Name as shown in the watch's settings, e.g. "Galaxy Watch7 (A1B2)". */
    fun name(context: Context): String? =
        runCatching {
            android.provider.Settings.Global.getString(
                context.contentResolver, android.provider.Settings.Global.DEVICE_NAME,
            )
        }.getOrNull()?.takeIf { it.isNotBlank() }

    /** "Wear OS 5" on a watch, "Android 14" elsewhere. */
    fun os(context: Context): String {
        val watch = context.packageManager.hasSystemFeature(PackageManager.FEATURE_WATCH)
        if (!watch) return "Android ${Build.VERSION.RELEASE}"
        val wear = when (Build.VERSION.SDK_INT) {
            30 -> "3"
            33 -> "4"
            34 -> "5"
            35 -> "5.1"
            36 -> "6"
            else -> null
        }
        return if (wear != null) "Wear OS $wear" else "Wear OS (Android ${Build.VERSION.RELEASE})"
    }

    data class Battery(val level: Int?, val charging: Boolean?)

    fun battery(context: Context): Battery {
        val manager = context.getSystemService(BatteryManager::class.java) ?: return Battery(null, null)
        val level = manager.getIntProperty(BatteryManager.BATTERY_PROPERTY_CAPACITY).takeIf { it in 0..100 }
        return Battery(level, manager.isCharging)
    }

    fun hello(context: Context, sensor: String?): JSONObject {
        val battery = battery(context)
        return JSONObject()
            .put("type", "hello")
            .put("device", name(context) ?: Build.MODEL)
            .put("manufacturer", Build.MANUFACTURER)
            .put("model", Build.MODEL)
            .put("os", os(context))
            .put("app", BuildConfig.VERSION_NAME)
            .apply {
                if (sensor != null) put("sensor", sensor)
                if (battery.level != null) put("battery", battery.level)
                if (battery.charging != null) put("charging", battery.charging)
            }
    }

    fun status(context: Context): JSONObject {
        val battery = battery(context)
        return JSONObject()
            .put("type", "status")
            .apply {
                if (battery.level != null) put("battery", battery.level)
                if (battery.charging != null) put("charging", battery.charging)
            }
    }
}
