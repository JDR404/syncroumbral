package app.syncroapp.umbral

import android.service.notification.NotificationListenerService
import android.service.notification.StatusBarNotification
import org.json.JSONObject

/** Lee las notificaciones del móvil y las reenvía al PC mientras hay conexión. */
class NotifService : NotificationListenerService() {
    override fun onListenerConnected() {
        instance = this
    }

    override fun onListenerDisconnected() {
        instance = null
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
    }
}
