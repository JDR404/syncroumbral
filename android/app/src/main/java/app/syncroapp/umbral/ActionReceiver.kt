package app.syncroapp.umbral

import android.content.BroadcastReceiver
import android.content.Context
import android.content.Intent
import android.widget.Toast
import org.json.JSONObject

/** Botones "Bloquear" y "Desbloquear" de la notificación permanente: actúan sobre SyncroUmbral del PC sin abrir la app. */
class ActionReceiver : BroadcastReceiver() {
    override fun onReceive(context: Context, intent: Intent) {
        val (type, ok, fail) = when (intent.action) {
            ACTION_LOCK -> Triple("lock_pc", "Bloqueo enviado al PC", "Sin conexión con el PC")
            ACTION_UNLOCK -> Triple("unlock_pc", "Desbloqueo enviado al PC", "Sin conexión con el PC")
            else -> return
        }
        val sent = Link.send(JSONObject().put("t", type))
        Toast.makeText(context, if (sent) ok else fail, Toast.LENGTH_SHORT).show()
    }

    companion object {
        const val ACTION_LOCK = "app.syncroapp.umbral.ACTION_LOCK_PC"
        const val ACTION_UNLOCK = "app.syncroapp.umbral.ACTION_UNLOCK_PC"
    }
}
