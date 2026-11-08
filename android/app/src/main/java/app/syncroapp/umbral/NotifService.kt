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

    override fun onNotificationRemoved(sbn: StatusBarNotification) {
        Link.send(JSONObject().put("t", "notif_gone").put("key", sbn.key))
    }

    companion object {
        @Volatile var instance: NotifService? = null
    }
}
