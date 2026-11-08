package app.syncroapp.umbral

import android.content.Context
import android.net.wifi.WifiManager
import android.os.Build
import android.os.PowerManager

/**
 * Mantiene viva la conexión con el PC cuando se apaga la pantalla del móvil. Sin esto Android duerme la CPU y el Wi-Fi entre
 * latidos, el PC deja de oír al móvil y se bloquea y desbloquea una y otra vez. Solo se mantiene mientras hay sesión con el PC
 * y se suelta al perderla. Cuesta algo de batería; se puede desactivar desde la pantalla principal de la app.
 */
object KeepAlive {
    private const val MAX_HOLD_MS = 12L * 60 * 60 * 1000 // seguridad: nunca más de 12 horas seguidas
    private var cpu: PowerManager.WakeLock? = null
    private var wifi: WifiManager.WifiLock? = null

    @Synchronized
    @Suppress("DEPRECATION")
    fun acquire(ctx: Context) {
        if (cpu?.isHeld != true) {
            val pm = ctx.applicationContext.getSystemService(Context.POWER_SERVICE) as PowerManager
            cpu = pm.newWakeLock(PowerManager.PARTIAL_WAKE_LOCK, "SyncroUmbral:conexion")
                .also { it.setReferenceCounted(false); it.acquire(MAX_HOLD_MS) }
        }
        if (wifi?.isHeld != true) {
            val wm = ctx.applicationContext.getSystemService(Context.WIFI_SERVICE) as WifiManager
            val mode = if (Build.VERSION.SDK_INT >= 29) WifiManager.WIFI_MODE_FULL_LOW_LATENCY else WifiManager.WIFI_MODE_FULL_HIGH_PERF
            wifi = wm.createWifiLock(mode, "SyncroUmbral:wifi").also { it.setReferenceCounted(false); it.acquire() }
        }
    }

    @Synchronized
    fun release() {
        cpu?.let { if (it.isHeld) it.release() }
        wifi?.let { if (it.isHeld) it.release() }
        cpu = null
        wifi = null
    }
}
