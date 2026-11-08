package app.syncroapp.umbral

import android.app.Notification
import android.app.NotificationManager
import android.app.RemoteInput
import android.content.Context
import android.content.Intent
import android.os.Bundle
import android.service.notification.NotificationListenerService
import android.service.notification.StatusBarNotification
import org.json.JSONArray
import org.json.JSONObject

/** Notificaciones para el PC: serialización y acciones (responder, pulsar un botón, descartar). */
object NotifOps {
    private const val MAX_TEXT = 2000

    fun handle(ctx: Context, op: String, a: JSONObject): JSONObject = when (op) {
        "notifs" -> list(ctx)
        "notif_reply" -> act(ctx, a.getString("key"), a.getInt("index"), a.getString("text"))
        "notif_action" -> act(ctx, a.getString("key"), a.getInt("index"), null)
        "notif_dismiss" -> dismiss(a.getString("key"))
        "notif_clear_all" -> clearAll()
        else -> throw IllegalArgumentException("Operación desconocida: $op")
    }

    /** Null si no debe mostrarse (en curso, resumen de grupo o sin contenido). */
    fun toJson(ctx: Context, sbn: StatusBarNotification, ranking: NotificationListenerService.RankingMap?): JSONObject? {
        val n = sbn.notification
        // Las llamadas se muestran siempre (aunque Android las marque "en curso"): el PC debe poder contestarlas o rechazarlas.
        val isCall = n.category == Notification.CATEGORY_CALL
        val ongoing = Notification.FLAG_ONGOING_EVENT or Notification.FLAG_FOREGROUND_SERVICE
        if (!isCall && (n.flags and ongoing != 0 || n.flags and Notification.FLAG_GROUP_SUMMARY != 0)) return null

        val e = n.extras
        val title = e.getCharSequence(Notification.EXTRA_TITLE)?.toString().orEmpty()
        val text = (e.getCharSequence(Notification.EXTRA_BIG_TEXT) ?: e.getCharSequence(Notification.EXTRA_TEXT))
            ?.toString().orEmpty().take(MAX_TEXT)
        if (title.isEmpty() && text.isEmpty()) return null

        var quiet = false
        if (ranking != null) {
            val r = NotificationListenerService.Ranking()
            if (ranking.getRanking(sbn.key, r)) quiet = r.importance < NotificationManager.IMPORTANCE_DEFAULT
        }

        val actions = JSONArray()
        n.actions?.forEachIndexed { i, act ->
            val ri = act.remoteInputs
            actions.put(
                JSONObject().put("index", i).put("title", act.title?.toString().orEmpty())
                    .put("reply", !ri.isNullOrEmpty())
                    .put("hint", ri?.firstOrNull()?.label?.toString().orEmpty())
            )
        }

        val pm = ctx.packageManager
        val app = try { pm.getApplicationLabel(pm.getApplicationInfo(sbn.packageName, 0)).toString() } catch (_: Exception) { sbn.packageName }
        return JSONObject()
            .put("key", sbn.key).put("pkg", sbn.packageName).put("app", app)
            .put("title", title).put("text", text)
            .put("sub", e.getCharSequence(Notification.EXTRA_SUB_TEXT)?.toString().orEmpty())
            .put("when", sbn.postTime).put("actions", actions)
            .put("quiet", quiet && !isCall).put("clearable", sbn.isClearable)
            .put("call", isCall)
            // Android 12 o superior indica si la llamada es entrante (1) o ya está en curso (2).
            .put(
                "incoming",
                isCall && (
                    (android.os.Build.VERSION.SDK_INT >= 31 &&
                        e.getInt(Notification.EXTRA_CALL_TYPE, -1) == Notification.CallStyle.CALL_TYPE_INCOMING) ||
                        // Apps que no usan CallStyle: si tiene botón "Contestar", todavía está sonando.
                        CallOps.hasAnswerAction(n)
                    )
            )
    }

    private fun list(ctx: Context): JSONObject {
        val svc = NotifService.instance ?: return JSONObject().put("granted", false).put("items", JSONArray())
        val items = JSONArray()
        val ranking = svc.currentRanking
        for (sbn in svc.activeNotifications.orEmpty()) toJson(ctx, sbn, ranking)?.let { items.put(it) }
        return JSONObject().put("granted", true).put("items", items)
    }

    private fun find(key: String): Pair<NotifService, StatusBarNotification> {
        val svc = NotifService.instance ?: throw IllegalStateException("Falta el acceso a las notificaciones en el móvil")
        val sbn = svc.activeNotifications.orEmpty().firstOrNull { it.key == key }
            ?: throw IllegalStateException("La notificación ya no existe")
        return svc to sbn
    }

    /** Pulsa una acción de la notificación. Con `text` rellena el campo de respuesta (RemoteInput). */
    private fun act(ctx: Context, key: String, index: Int, text: String?): JSONObject {
        val (_, sbn) = find(key)
        val action = sbn.notification.actions?.getOrNull(index) ?: throw IllegalStateException("Acción no encontrada")
        val intent = Intent()
        if (text != null) {
            val inputs = action.remoteInputs ?: throw IllegalStateException("Esta acción no admite respuesta")
            val results = Bundle()
            for (r in inputs) results.putCharSequence(r.resultKey, text)
            RemoteInput.addResultsToIntent(inputs, intent, results)
        }
        action.actionIntent.send(ctx, 0, intent)
        return JSONObject()
    }

    /** Descarta todas las notificaciones que se pueden descartar (no toca las de música, llamadas en curso, etc.). */
    private fun clearAll(): JSONObject {
        val svc = NotifService.instance ?: throw IllegalStateException("Falta el acceso a las notificaciones en el móvil")
        val keys = svc.activeNotifications.orEmpty().filter { it.isClearable }.map { it.key }
        if (keys.isNotEmpty()) svc.cancelNotifications(keys.toTypedArray())
        return JSONObject().put("n", keys.size)
    }

    private fun dismiss(key: String): JSONObject {
        val (svc, sbn) = find(key)
        if (!sbn.isClearable) throw IllegalStateException("Esta notificación no se puede descartar")
        svc.cancelNotification(key)
        return JSONObject()
    }
}
