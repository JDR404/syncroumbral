package app.syncroapp.umbral

import android.app.Activity
import android.content.ClipboardManager
import android.os.Handler
import android.os.Looper
import android.widget.Toast
import org.json.JSONObject

/**
 * Botón "Copiar al PC" de la notificación permanente. Android 10 o superior solo deja leer el portapapeles a la app que está en
 * primer plano y con el foco, y el foco llega unos milisegundos después de abrirse la ventana: por eso se reintenta unas
 * cuantas veces (antes fallaba "a veces"). El PC deja el texto directamente en su portapapeles (`clip`).
 */
class ClipSendActivity : Activity() {
    private val main = Handler(Looper.getMainLooper())
    private var finished = false
    private var tries = 0

    override fun onWindowFocusChanged(hasFocus: Boolean) {
        super.onWindowFocusChanged(hasFocus)
        if (hasFocus && !finished && tries == 0) attempt()
    }

    private fun attempt() {
        if (finished) return
        tries++
        val text = try {
            getSystemService(ClipboardManager::class.java).primaryClip
                ?.takeIf { it.itemCount > 0 }?.getItemAt(0)?.coerceToText(this)?.toString().orEmpty()
        } catch (_: Exception) { "" }
        if (text.isBlank() && tries < 6) {
            main.postDelayed({ attempt() }, 150)
            return
        }
        finished = true
        val msg = when {
            text.isBlank() -> "No se pudo leer el portapapeles (está vacío o Android lo bloqueó). Cópialo de nuevo e inténtalo otra vez"
            Link.send(JSONObject().put("t", "share_text").put("text", text.take(100_000)).put("clip", true)) -> "Portapapeles enviado al PC"
            else -> "Sin conexión con el PC"
        }
        Toast.makeText(this, msg, Toast.LENGTH_SHORT).show()
        finish()
    }

    override fun onDestroy() {
        main.removeCallbacksAndMessages(null)
        super.onDestroy()
    }
}
