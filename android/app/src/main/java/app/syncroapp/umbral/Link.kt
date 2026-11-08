package app.syncroapp.umbral

import okhttp3.WebSocket
import org.json.JSONObject

/** Conexión activa con el PC. Permite que otros componentes (p. ej. el lector de notificaciones) envíen mensajes. */
object Link {
    @Volatile var ws: WebSocket? = null

    fun send(msg: JSONObject): Boolean = ws?.send(msg.toString()) ?: false

    /** Estado del móvil que el PC muestra en su cabecera. */
    fun sendStatus(ctx: android.content.Context) {
        val i = ctx.registerReceiver(null, android.content.IntentFilter(android.content.Intent.ACTION_BATTERY_CHANGED)) ?: return
        val level = i.getIntExtra(android.os.BatteryManager.EXTRA_LEVEL, -1)
        val scale = i.getIntExtra(android.os.BatteryManager.EXTRA_SCALE, 100)
        if (level < 0 || scale <= 0) return
        val st = i.getIntExtra(android.os.BatteryManager.EXTRA_STATUS, -1)
        val charging = st == android.os.BatteryManager.BATTERY_STATUS_CHARGING || st == android.os.BatteryManager.BATTERY_STATUS_FULL
        val pm = ctx.getSystemService(android.content.Context.POWER_SERVICE) as android.os.PowerManager
        send(
            JSONObject().put("t", "status").put("battery", level * 100 / scale).put("charging", charging)
                // Avisos para el PC: con el ahorro de batería activo o con SyncroUmbral "optimizado", Android puede cortar la conexión.
                .put("saver", pm.isPowerSaveMode)
                .put("optimized", !pm.isIgnoringBatteryOptimizations(ctx.packageName))
        )
    }
}
