package app.syncroapp.umbral

import android.app.Notification
import android.app.NotificationChannel
import android.app.NotificationManager
import android.app.PendingIntent
import android.app.RemoteInput
import android.content.BroadcastReceiver
import android.content.Context
import android.content.Intent

/**
 * Notificación de prueba con campo de respuesta, para comprobar de punta a punta que el PC recibe
 * notificaciones y que puede responderlas. Al responder, publica "Respuesta recibida: ...".
 */
object TestNotification {
    private const val CHANNEL = "umbral_test"
    private const val ID = 4001
    const val KEY_REPLY = "reply"

    fun post(ctx: Context, body: String = "Mensaje de prueba. Responde desde el PC.") {
        val nm = ctx.getSystemService(NotificationManager::class.java)
        nm.createNotificationChannel(NotificationChannel(CHANNEL, "Prueba de notificaciones", NotificationManager.IMPORTANCE_HIGH))

        val replyIntent = PendingIntent.getBroadcast(
            ctx, 0, Intent(ctx, ReplyReceiver::class.java).setPackage(ctx.packageName),
            PendingIntent.FLAG_UPDATE_CURRENT or PendingIntent.FLAG_MUTABLE
        )
        val input = RemoteInput.Builder(KEY_REPLY).setLabel("Responder").build()
        val action = Notification.Action.Builder(android.R.drawable.ic_menu_send, "Responder", replyIntent)
            .addRemoteInput(input).build()

        nm.notify(
            ID,
            Notification.Builder(ctx, CHANNEL)
                .setSmallIcon(android.R.drawable.ic_dialog_email)
                .setContentTitle("SyncroUmbral (prueba)")
                .setContentText(body)
                .addAction(action)
                .setAutoCancel(true)
                .build()
        )
    }

    private const val CALL_ID = 4002

    /**
     * Llamada entrante simulada, como la de WhatsApp: categoría llamada, "Contestar" abre una pantalla (una actividad) y
     * "Rechazar" es un aviso interno. Sirve para comprobar que el PC puede contestar con SyncroUmbral en segundo plano.
     */
    fun postCall(ctx: Context) {
        val nm = ctx.getSystemService(NotificationManager::class.java)
        nm.createNotificationChannel(NotificationChannel(CHANNEL, "Prueba de notificaciones", NotificationManager.IMPORTANCE_HIGH))
        val answer = PendingIntent.getActivity(
            ctx, 1, Intent(ctx, MainActivity::class.java).setAction("app.syncroapp.umbral.TEST_CALL_ANSWERED")
                .addFlags(Intent.FLAG_ACTIVITY_NEW_TASK),
            PendingIntent.FLAG_UPDATE_CURRENT or PendingIntent.FLAG_IMMUTABLE
        )
        val decline = PendingIntent.getBroadcast(
            ctx, 2, Intent(ctx, CallDeclineReceiver::class.java).setPackage(ctx.packageName),
            PendingIntent.FLAG_UPDATE_CURRENT or PendingIntent.FLAG_IMMUTABLE
        )
        nm.notify(
            CALL_ID,
            Notification.Builder(ctx, CHANNEL)
                .setSmallIcon(android.R.drawable.ic_menu_call)
                .setContentTitle("Llamada de prueba")
                .setContentText("Llamada entrante de WhatsApp (simulada)")
                .setCategory(Notification.CATEGORY_CALL)
                .setOngoing(true)
                .addAction(Notification.Action.Builder(android.R.drawable.ic_menu_call, "Contestar", answer).build())
                .addAction(Notification.Action.Builder(android.R.drawable.ic_menu_close_clear_cancel, "Rechazar", decline).build())
                .build()
        )
    }

    class CallDeclineReceiver : BroadcastReceiver() {
        override fun onReceive(context: Context, intent: Intent) {
            context.getSystemService(NotificationManager::class.java).cancel(CALL_ID)
        }
    }

    class ReplyReceiver : BroadcastReceiver() {
        override fun onReceive(context: Context, intent: Intent) {
            val text = RemoteInput.getResultsFromIntent(intent)?.getCharSequence(KEY_REPLY)?.toString() ?: return
            post(context, "Respuesta recibida: $text")
        }
    }
}
