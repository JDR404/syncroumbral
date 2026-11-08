package app.syncroapp.umbral

import android.content.ClipData
import android.content.ClipboardManager
import android.content.Context
import android.os.Handler
import android.os.Looper
import org.json.JSONObject
import java.util.concurrent.CountDownLatch
import java.util.concurrent.TimeUnit

/** Portapapeles del móvil. Android solo permite a una app en segundo plano escribirlo, no leerlo. */
object ClipOps {
    fun handle(ctx: Context, op: String, a: JSONObject): JSONObject {
        if (op != "clip_set") throw IllegalArgumentException("Operación desconocida: $op")
        val text = a.getString("text").take(100_000)
        var error: Exception? = null
        val done = CountDownLatch(1)
        // El portapapeles se toca desde el hilo principal.
        Handler(Looper.getMainLooper()).post {
            try {
                ctx.getSystemService(ClipboardManager::class.java)
                    .setPrimaryClip(ClipData.newPlainText("SyncroUmbral", text))
            } catch (e: Exception) {
                error = e
            }
            done.countDown()
        }
        if (!done.await(3, TimeUnit.SECONDS)) throw IllegalStateException("El móvil no respondió")
        error?.let { throw IllegalStateException(it.message ?: "No se pudo copiar") }
        return JSONObject()
    }
}
