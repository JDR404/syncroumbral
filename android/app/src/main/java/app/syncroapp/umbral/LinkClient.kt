package app.syncroapp.umbral

import android.content.Context
import android.util.Base64
import okhttp3.OkHttpClient
import okhttp3.Request
import okhttp3.Response
import okhttp3.WebSocket
import okhttp3.WebSocketListener
import org.json.JSONObject
import java.util.concurrent.CountDownLatch
import java.util.concurrent.Executors
import java.util.concurrent.ScheduledExecutorService
import java.util.concurrent.TimeUnit
import java.util.concurrent.atomic.AtomicBoolean

/**
 * Mantiene la conexión con el PC: se conecta, se autentica con la clave del Keystore
 * y envía latidos. Reintenta solo. Ver docs/PROTOCOLO.md.
 */
class LinkClient(private val ctx: Context) {
    @Volatile private var running = false
    @Volatile private var ws: WebSocket? = null
    private var thread: Thread? = null
    private var failures = 0

    fun start() {
        if (running) return
        running = true
        thread = Thread(::loop, "umbral-link").also { it.start() }
    }

    fun stop() {
        running = false
        ws?.cancel()
        thread?.interrupt()
        io.shutdownNow() // sin esto los hilos de peticiones quedarían vivos tras detener el servicio
        Status.set("Detenido")
    }

    private fun loop() {
        val store = Store(ctx)
        while (running) {
            try {
                if (!store.hasPc) {
                    Status.set("Sin vincular")
                    Thread.sleep(2000)
                    continue
                }
                // Además de las IP del QR se prueba 127.0.0.1: es la ruta del cable (adb reverse).
                val candidates = (store.hosts + "127.0.0.1").distinct()
                var reached = false
                for (host in candidates) {
                    if (!running) break
                    Status.set("Conectando a $host…")
                    if (runSession(store, host)) { reached = true; break }
                }
                if (!reached && running && store.hasPc) {
                    // Las direcciones guardadas no responden: se busca al PC en la red (puede haber cambiado de IP).
                    Status.set("Buscando al PC en la red…")
                    for (host in Discovery.find(ctx, store.fingerprint).filter { it !in candidates }) {
                        if (!running) break
                        Status.set("Conectando a $host…")
                        // La dirección solo se recuerda cuando el PC demostró ser el vinculado (sesión autenticada): un
                        // anuncio falso en la red no puede desplazar las direcciones buenas.
                        val remember = { store.hosts = (listOf(host) + store.hosts).distinct().take(4) }
                        if (runSession(store, host, remember)) {
                            reached = true
                            break
                        }
                    }
                }
                if (running) {
                    // Espera creciente (3 s hasta 30 s) mientras el PC no aparece: con el PC apagado no se gasta batería buscando sin parar.
                    failures = if (reached) 0 else (failures + 1).coerceAtMost(5)
                    Status.set(if (reached) "Desconectado, reintentando…" else "PC no encontrado, reintentando…")
                    Thread.sleep(if (reached) 1000 else minOf(30_000L, 3000L * (1L shl (failures - 1))))
                }
            } catch (_: InterruptedException) {
                return
            } catch (e: Exception) {
                Status.set("Error: ${e.message}")
                try { Thread.sleep(3000) } catch (_: InterruptedException) { return }
            }
        }
    }

    /** Bloquea hasta que la sesión termina. Devuelve true si llegó a autenticarse. */
    private fun runSession(store: Store, host: String, onAuthed: () -> Unit = {}): Boolean {
        val tls = PinnedTls(store.fingerprint)
        val client = OkHttpClient.Builder()
            .sslSocketFactory(tls.socketFactory, tls.trustManager)
            .hostnameVerifier { _, _ -> true } // la identidad se verifica por huella, no por nombre
            .connectTimeout(3, TimeUnit.SECONDS)
            .pingInterval(5, TimeUnit.SECONDS)
            .build()

        val done = CountDownLatch(1)
        val authed = AtomicBoolean(false)
        var scheduler: ScheduledExecutorService? = null

        val listener = object : WebSocketListener() {
            override fun onOpen(webSocket: WebSocket, response: Response) {
                val tok = store.pendingToken
                if (tok != null) {
                    webSocket.send(
                        JSONObject()
                            .put("t", "pair").put("tok", tok)
                            .put("pub", Base64.encodeToString(KeyManager.publicKeyDer(), Base64.NO_WRAP))
                            .put("name", android.os.Build.MODEL)
                            .toString()
                    )
                } else {
                    sendHello(webSocket)
                }
            }

            override fun onMessage(webSocket: WebSocket, text: String) {
                val msg = JSONObject(text)
                when (msg.getString("t")) {
                    "paired" -> {
                        store.pendingToken = null
                        store.paired = true
                        sendHello(webSocket)
                    }
                    "challenge" -> {
                        val n = Base64.decode(msg.getString("n"), Base64.NO_WRAP)
                        val sig = KeyManager.sign("umbral-v1".toByteArray() + n)
                        webSocket.send(
                            JSONObject().put("t", "auth")
                                .put("sig", Base64.encodeToString(sig, Base64.NO_WRAP)).toString()
                        )
                    }
                    "ok" -> {
                        authed.set(true)
                        onAuthed()
                        if (store.keepAlive) KeepAlive.acquire(ctx)
                        Link.ws = webSocket
                        Status.set("Conectado a ${store.pcName}", connected = true)
                        val every = msg.optLong("hb", 2000)
                        var seq = 0L
                        scheduler = Executors.newSingleThreadScheduledExecutor().also {
                            it.scheduleAtFixedRate({
                                webSocket.send(JSONObject().put("t", "hb").put("seq", seq++).toString())
                            }, 0, every, TimeUnit.MILLISECONDS)
                            it.scheduleAtFixedRate({ Link.sendStatus(ctx) }, 1, 30, TimeUnit.SECONDS)
                        }
                    }
                    "req" -> io.execute { answer(webSocket, msg) }
                    "error" -> {
                        Status.set("PC: ${msg.optString("msg")}")
                        webSocket.close(1000, null)
                    }
                }
            }

            override fun onClosing(webSocket: WebSocket, code: Int, reason: String) {
                webSocket.close(1000, null)
            }

            override fun onClosed(webSocket: WebSocket, code: Int, reason: String) {
                done.countDown()
            }

            override fun onFailure(webSocket: WebSocket, t: Throwable, response: Response?) {
                done.countDown()
            }
        }

        ws = client.newWebSocket(Request.Builder().url("wss://$host:${store.port}/").build(), listener)
        try {
            done.await()
        } finally {
            Link.ws = null
            PowerOps.release() // sin conexión con el PC no hay motivo para mantener el teléfono despierto
            KeepAlive.release()
            scheduler?.shutdownNow()
            ws?.cancel()
            client.dispatcher.executorService.shutdown()
        }
        return authed.get()
    }

    /** Peticiones del PC (archivos). Se atienden fuera del hilo de red para no frenar los latidos. */
    private val io = Executors.newFixedThreadPool(4) { r -> Thread(r, "umbral-io").apply { isDaemon = true } }

    private fun answer(ws: WebSocket, req: JSONObject) {
        val res = JSONObject().put("t", "res").put("id", req.getLong("id"))
        try {
            val op = req.getString("op")
            val data = when {
                op == "apps" || op == "icons" -> AppOps.handle(ctx, op, req)
                op.startsWith("notif") -> NotifOps.handle(ctx, op, req)
                op.startsWith("call_") -> CallOps.handle(ctx, op, req)
                op.startsWith("clip_") -> ClipOps.handle(ctx, op, req)
                op == "keep_awake" -> PowerOps.handle(ctx, op, req)
                op == "set_mode" -> { Store(ctx).advanced = req.getBoolean("advanced"); JSONObject() }
                else -> FileOps.handle(ctx, op, req)
            }
            res.put("ok", true).put("data", data)
        } catch (e: Exception) {
            res.put("ok", false).put("err", e.message ?: e.javaClass.simpleName)
        }
        ws.send(res.toString())
    }

    private fun sendHello(ws: WebSocket) {
        ws.send(JSONObject().put("t", "hello").put("id", KeyManager.keyId()).toString())
    }
}
