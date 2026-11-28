package app.syncroapp.umbral

import android.content.ComponentName
import android.content.Context
import android.content.pm.PackageManager
import android.service.notification.NotificationListenerService
import android.service.notification.StatusBarNotification
import androidx.core.app.NotificationManagerCompat
import org.json.JSONObject

/** Lee las notificaciones del móvil y las reenvía al PC mientras hay conexión. */
class NotifService : NotificationListenerService() {
    override fun onListenerConnected() {
        instance = this
        attempts = 0
        // Si el sistema había cerrado la app, vuelve a levantar la conexión con el PC.
        if (Store(this).hasPc) {
            try {
                LinkService.start(this)
            } catch (_: Exception) {
                // Android puede negar el arranque en segundo plano; se reintenta al abrir la app.
            }
        }
    }

    override fun onListenerDisconnected() {
        instance = null
        requestRebind(ComponentName(this, NotifService::class.java))
    }

    override fun onNotificationPosted(sbn: StatusBarNotification, rankingMap: RankingMap) {
        val json = NotifOps.toJson(this, sbn, rankingMap) ?: return
        Link.send(json.put("t", "notif"))
    }

    override fun onNotificationRemoved(sbn: StatusBarNotification, rankingMap: RankingMap, reason: Int) {
        val wasCall = sbn.notification.category == android.app.Notification.CATEGORY_CALL ||
            CallOps.isCellularPkg(this, sbn.packageName)
        if (!wasCall) {
            Link.send(JSONObject().put("t", "notif_gone").put("key", sbn.key))
            return
        }
        callReason = reason
        android.util.Log.d("SyncroUmbral", "llamada: notificación retirada (motivo $reason) ${sbn.packageName}")
        // La app de teléfono retira y vuelve a publicar la notificación de la llamada al cambiar de estado (timbre, pantalla
        // completa, en curso). Se espera un momento: si sigue habiendo una notificación de llamada, el PC la recibe de nuevo
        // y la llamada no parpadea ni desaparece; si no, se avisa de que terminó.
        handler.postDelayed({
            Link.send(JSONObject().put("t", "notif_gone").put("key", sbn.key).put("reason", callReason))
            val ranking = currentRanking
            for (a in activeNotifications.orEmpty()) {
                NotifOps.toJson(this, a, ranking)?.takeIf { it.optBoolean("call") }?.let { Link.send(it.put("t", "notif")) }
            }
        }, 1500)
    }

    @Volatile private var callReason = -1
    private val handler = android.os.Handler(android.os.Looper.getMainLooper())

    companion object {
        @Volatile var instance: NotifService? = null
        @Volatile private var attempts = 0

        /**
         * Algunos fabricantes (Xiaomi/HyperOS entre ellos) cierran el lector de notificaciones y no lo vuelven a conectar
         * aunque el permiso siga concedido: el móvil parece conectado pero no llega ninguna notificación. Si el permiso está y
         * el lector no, se pide a Android que lo reconecte; si insiste en no hacerlo, se desactiva y activa el componente,
         * que le obliga a volver a enlazarlo (el permiso se conserva).
         */
        fun ensureBound(ctx: Context) {
            if (instance != null) {
                attempts = 0
                return
            }
            if (!NotificationManagerCompat.getEnabledListenerPackages(ctx).contains(ctx.packageName)) return
            val me = ComponentName(ctx, NotifService::class.java)
            attempts++
            if (attempts % 3 == 0) {
                val pm = ctx.packageManager
                pm.setComponentEnabledSetting(me, PackageManager.COMPONENT_ENABLED_STATE_DISABLED, PackageManager.DONT_KILL_APP)
                pm.setComponentEnabledSetting(me, PackageManager.COMPONENT_ENABLED_STATE_ENABLED, PackageManager.DONT_KILL_APP)
            }
            requestRebind(me)
        }
    }
}
