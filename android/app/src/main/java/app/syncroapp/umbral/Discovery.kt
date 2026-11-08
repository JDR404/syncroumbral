package app.syncroapp.umbral

import android.content.Context
import android.net.nsd.NsdManager
import android.net.nsd.NsdServiceInfo
import android.util.Log
import java.util.concurrent.CountDownLatch
import java.util.concurrent.TimeUnit

/**
 * Busca al PC en la red local (mDNS) cuando las direcciones guardadas ya no responden, por ejemplo porque el
 * router le dio otra IP. Solo propone direcciones: la conexión sigue exigiendo la huella del certificado del PC.
 */
object Discovery {
    private const val TAG = "UmbralNsd"

    /** Direcciones IPv4 de PCs SyncroUmbral cuyo anuncio coincide con la huella guardada. Bloquea hasta `waitMs`. */
    fun find(ctx: Context, fingerprint: String, waitMs: Long = 5000): List<String> {
        val nsd = ctx.getSystemService(Context.NSD_SERVICE) as NsdManager
        val found = java.util.Collections.synchronizedList(mutableListOf<String>())
        val wanted = fingerprint.take(16).lowercase()
        val resolving = java.util.concurrent.Semaphore(1) // Android resuelve de uno en uno
        val done = CountDownLatch(1)

        val listener = object : NsdManager.DiscoveryListener {
            override fun onDiscoveryStarted(type: String) {}
            override fun onDiscoveryStopped(type: String) {}
            override fun onStartDiscoveryFailed(type: String, code: Int) { done.countDown() }
            override fun onStopDiscoveryFailed(type: String, code: Int) {}
            override fun onServiceLost(info: NsdServiceInfo) {}

            @Suppress("DEPRECATION")
            override fun onServiceFound(info: NsdServiceInfo) {
                Log.i(TAG, "encontrado: ${info.serviceName} tipo=${info.serviceType}")
                if (!resolving.tryAcquire(1, TimeUnit.SECONDS)) return
                nsd.resolveService(info, object : NsdManager.ResolveListener {
                    override fun onResolveFailed(i: NsdServiceInfo, code: Int) {
                        Log.i(TAG, "no se pudo resolver ${i.serviceName}: código $code")
                        resolving.release()
                    }
                    override fun onServiceResolved(i: NsdServiceInfo) {
                        val fp = i.attributes["fp"]?.toString(Charsets.UTF_8)?.lowercase()
                        val ip = i.host?.hostAddress
                        Log.i(TAG, "resuelto ${i.serviceName}: ip=$ip fp=$fp esperado=$wanted")
                        // Solo IPv4 de la red (no bucle local ni enlace local) y solo el PC cuya huella coincide.
                        if (fp == wanted && ip != null && !ip.contains(':') && !ip.startsWith("127.") && !ip.startsWith("169.254.")) {
                            found.add(ip)
                            done.countDown()
                        }
                        resolving.release()
                    }
                })
            }
        }
        try {
            nsd.discoverServices("_umbral._tcp", NsdManager.PROTOCOL_DNS_SD, listener)
            done.await(waitMs, TimeUnit.MILLISECONDS)
        } catch (_: Exception) {
            // sin descubrimiento disponible
        } finally {
            try { nsd.stopServiceDiscovery(listener) } catch (_: Exception) {}
        }
        return found.toList()
    }
}
