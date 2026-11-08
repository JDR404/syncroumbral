package app.syncroapp.umbral

import android.content.Context
import android.os.PowerManager
import org.json.JSONObject

/**
 * Mantiene el teléfono despierto mientras el PC tiene apps abiertas en pantallas virtuales.
 * Con el teléfono bloqueado Android duerme la pantalla a los 10 s, y al dormirse apaga también la pantalla virtual
 * (la app se ve negra). El PC apaga solo el panel físico; este bloqueo evita que el sistema entre en reposo.
 */
object PowerOps {
    private const val MAX_HOLD_MS = 6L * 60 * 60 * 1000 // seguridad: nunca más de 6 horas
    private var lock: PowerManager.WakeLock? = null

    fun handle(ctx: Context, op: String, a: JSONObject): JSONObject {
        if (op != "keep_awake") throw IllegalArgumentException("Operación desconocida: $op")
        if (a.getBoolean("on")) acquire(ctx) else release()
        return JSONObject().put("on", lock?.isHeld == true)
    }

    @Synchronized
    @Suppress("DEPRECATION")
    private fun acquire(ctx: Context) {
        if (lock?.isHeld == true) return
        val pm = ctx.getSystemService(Context.POWER_SERVICE) as PowerManager
        lock = pm.newWakeLock(
            PowerManager.SCREEN_BRIGHT_WAKE_LOCK or PowerManager.ACQUIRE_CAUSES_WAKEUP,
            "SyncroUmbral:apps-en-el-pc"
        ).also { it.setReferenceCounted(false); it.acquire(MAX_HOLD_MS) }
    }

    /** Se llama al pedirlo el PC y siempre que se pierde la conexión con él. */
    @Synchronized
    fun release() {
        lock?.let { if (it.isHeld) it.release() }
        lock = null
    }
}
