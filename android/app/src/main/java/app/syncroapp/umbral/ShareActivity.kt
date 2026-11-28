package app.syncroapp.umbral

import android.app.Activity
import android.content.Intent
import android.net.Uri
import android.os.Build
import android.os.Bundle
import android.os.Handler
import android.os.Looper
import android.provider.OpenableColumns
import android.util.Base64
import android.widget.Toast
import org.json.JSONObject

/**
 * Destino del menú "Compartir" de Android: envía texto o archivos al PC. Se mantiene abierta (sin interfaz)
 * hasta terminar, porque Android solo concede lectura de los archivos mientras esta pantalla vive.
 */
class ShareActivity : Activity() {
    private val ui = Handler(Looper.getMainLooper())

    /** Máximo por archivo (el PC aplica el mismo límite). */
    private val maxFile = 2L * 1024 * 1024 * 1024

    /**
     * Esta pantalla la puede abrir cualquier app (es el destino del menú Compartir): antes de enviar nada al PC se
     * le muestra al usuario qué se va a mandar y a dónde, y solo se envía si lo acepta.
     */
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        val intent = intent
        if (Link.ws == null) {
            Toast.makeText(applicationContext, "Sin conexión con el PC", Toast.LENGTH_LONG).show()
            finish()
            return
        }
        Thread {
            val summary = try { describe(intent) } catch (e: Exception) { null }
            ui.post {
                if (summary == null) {
                    Toast.makeText(applicationContext, "No hay nada que enviar", Toast.LENGTH_LONG).show()
                    finish()
                    return@post
                }
                ShareSheet.show(this, "Enviar a ${Store(this).pcName}", summary, { send(intent) }, { finish() })
            }
        }.start()
    }

    private fun send(intent: Intent) {
        Thread {
            val msg = try { share(intent) } catch (e: Exception) { "No se pudo enviar: ${e.message}" }
            ui.post {
                Toast.makeText(applicationContext, msg, Toast.LENGTH_LONG).show()
                finish()
            }
        }.start()
    }

    private fun urisFor(intent: Intent): List<Uri> = when (intent.action) {
        Intent.ACTION_SEND -> listOfNotNull(uriOf(intent))
        Intent.ACTION_SEND_MULTIPLE -> urisOf(intent)
        else -> emptyList()
    }

    /** Texto para el diálogo de confirmación, o null si no hay nada que enviar. */
    private fun describe(intent: Intent): String? {
        val uris = urisFor(intent)
        if (uris.isEmpty()) {
            val t = intent.getStringExtra(Intent.EXTRA_TEXT)
            return if (t.isNullOrBlank()) null else "Texto de ${t.length} caracteres:\n\n${t.take(200)}${if (t.length > 200) "…" else ""}"
        }
        var total = 0L
        val names = uris.take(4).map { u ->
            var name = u.lastPathSegment ?: "archivo"
            contentResolver.query(u, null, null, null, null)?.use { c ->
                if (c.moveToFirst()) {
                    val ni = c.getColumnIndex(OpenableColumns.DISPLAY_NAME)
                    val si = c.getColumnIndex(OpenableColumns.SIZE)
                    if (ni >= 0) name = c.getString(ni) ?: name
                    if (si >= 0 && !c.isNull(si)) total += c.getLong(si)
                }
            }
            name.take(60)
        }
        val mb = total / 1048576.0
        return "${uris.size} archivo(s), ${"%.1f".format(mb)} MB\n\n" + names.joinToString("\n") +
            (if (uris.size > 4) "\n…y ${uris.size - 4} más" else "")
    }

    private fun share(intent: Intent): String {
        if (Link.ws == null) return "Sin conexión con el PC"
        val uris = ArrayList<Uri>(urisFor(intent))
        val text = intent.getStringExtra(Intent.EXTRA_TEXT)
        if (uris.isEmpty()) {
            if (text.isNullOrBlank()) return "No hay nada que enviar"
            val ok = Link.send(JSONObject().put("t", "share_text").put("text", text.take(100_000)))
            return if (ok) "Texto enviado al PC" else "Sin conexión con el PC"
        }
        var sent = 0
        for (u in uris) if (sendFile(u)) sent++
        return if (sent == uris.size) "Enviado al PC: $sent archivo(s)" else "Se enviaron $sent de ${uris.size} archivos"
    }

    @Suppress("DEPRECATION")
    private fun uriOf(i: Intent): Uri? =
        if (Build.VERSION.SDK_INT >= 33) i.getParcelableExtra(Intent.EXTRA_STREAM, Uri::class.java)
        else i.getParcelableExtra(Intent.EXTRA_STREAM)

    @Suppress("DEPRECATION")
    private fun urisOf(i: Intent): List<Uri> =
        (if (Build.VERSION.SDK_INT >= 33) i.getParcelableArrayListExtra(Intent.EXTRA_STREAM, Uri::class.java)
        else i.getParcelableArrayListExtra<Uri>(Intent.EXTRA_STREAM)) ?: emptyList()

    private fun sendFile(uri: Uri): Boolean {
        var name = uri.lastPathSegment ?: "archivo"
        var size = -1L
        contentResolver.query(uri, null, null, null, null)?.use { c ->
            if (c.moveToFirst()) {
                val ni = c.getColumnIndex(OpenableColumns.DISPLAY_NAME)
                val si = c.getColumnIndex(OpenableColumns.SIZE)
                if (ni >= 0) name = c.getString(ni) ?: name
                if (si >= 0 && !c.isNull(si)) size = c.getLong(si)
            }
        }
        if (size < 0) size = contentResolver.openAssetFileDescriptor(uri, "r")?.use { it.length } ?: -1L
        if (size < 0 || size > maxFile) return false // sin tamaño no se puede verificar la recepción; y hay un tope

        val id = System.currentTimeMillis() * 1000 + (0..999).random()
        if (!Link.send(JSONObject().put("t", "share_begin").put("id", id).put("name", name).put("size", size))) return false
        contentResolver.openInputStream(uri)?.use { input ->
            val buf = ByteArray(192 * 1024)
            while (true) {
                val n = input.read(buf)
                if (n < 0) break
                // Espera a que se vacíe la cola de envío para no acumular el archivo entero en memoria.
                var waited = 0
                while ((Link.ws?.queueSize() ?: 0) > 4L * 1024 * 1024) {
                    Thread.sleep(20)
                    if (++waited > 1500) return false
                }
                val ok = Link.send(
                    JSONObject().put("t", "share_chunk").put("id", id)
                        .put("data", Base64.encodeToString(buf, 0, n, Base64.NO_WRAP))
                )
                if (!ok) return false
            }
        } ?: return false
        return Link.send(JSONObject().put("t", "share_end").put("id", id))
    }
}
