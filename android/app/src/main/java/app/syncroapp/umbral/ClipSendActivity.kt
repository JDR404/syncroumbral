package app.syncroapp.umbral

import android.app.Activity
import android.content.ClipboardManager
import android.widget.Toast
import org.json.JSONObject

/**
 * Botón "Portapapeles al PC" de la notificación. Android 10 o superior solo deja leer el portapapeles a la app que está en
 * primer plano, así que esta actividad transparente se abre, lee el portapapeles en cuanto tiene el foco, lo envía y se cierra.
 */
class ClipSendActivity : Activity() {
    private var done = false

    override fun onWindowFocusChanged(hasFocus: Boolean) {
        super.onWindowFocusChanged(hasFocus)
        if (!hasFocus || done) return
        done = true
        val text = getSystemService(ClipboardManager::class.java).primaryClip
            ?.takeIf { it.itemCount > 0 }?.getItemAt(0)?.coerceToText(this)?.toString().orEmpty()
        val msg = when {
            text.isBlank() -> "El portapapeles está vacío"
            Link.send(JSONObject().put("t", "share_text").put("text", text.take(100_000))) -> "Portapapeles enviado al PC"
            else -> "Sin conexión con el PC"
        }
        Toast.makeText(this, msg, Toast.LENGTH_SHORT).show()
        finish()
    }
}
