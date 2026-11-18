package app.syncroapp.umbral

import android.app.Notification
import android.app.NotificationChannel
import android.app.NotificationManager
import android.app.Service
import android.content.Context
import android.content.Intent
import android.content.pm.ServiceInfo
import android.os.Build
import android.os.IBinder
import androidx.core.app.NotificationCompat
import androidx.core.app.ServiceCompat

/** Servicio en primer plano: evita que Android mate el latido en segundo plano. */
class LinkService : Service() {
    private var client: LinkClient? = null

    override fun onBind(intent: Intent?): IBinder? = null

    override fun onCreate() {
        super.onCreate()
        val nm = getSystemService(NotificationManager::class.java)
        nm.createNotificationChannel(
            NotificationChannel(CHANNEL, "Conexión con el PC", NotificationManager.IMPORTANCE_LOW)
        )
        ServiceCompat.startForeground(
            this, NOTIF_ID, buildNotification(),
            if (Build.VERSION.SDK_INT >= 34) ServiceInfo.FOREGROUND_SERVICE_TYPE_SPECIAL_USE else 0
        )
        Status.onChange = { nm.notify(NOTIF_ID, buildNotification()) }
        KeyManager.ensureKey()
        client = LinkClient(applicationContext).also { it.start() }
    }

    override fun onStartCommand(intent: Intent?, flags: Int, startId: Int): Int = START_STICKY

    override fun onDestroy() {
        client?.stop()
        Status.onChange = null
        super.onDestroy()
    }

    /** Acción de la notificación que envía un broadcast interno (nadie más puede dispararlo: explícito e inmutable). */
    private fun action(label: String, act: String, code: Int, unlockedPhoneRequired: Boolean): NotificationCompat.Action {
        val pi = android.app.PendingIntent.getBroadcast(
            this, code, Intent(act).setClass(this, ActionReceiver::class.java),
            android.app.PendingIntent.FLAG_IMMUTABLE or android.app.PendingIntent.FLAG_UPDATE_CURRENT
        )
        return NotificationCompat.Action.Builder(0, label, pi)
            // Desbloquear el PC desde la pantalla de bloqueo del móvil sería regalar la llave: exige desbloquear el teléfono.
            .setAuthenticationRequired(unlockedPhoneRequired)
            .build()
    }

    private fun buildNotification(): Notification =
        NotificationCompat.Builder(this, CHANNEL)
            .setSmallIcon(R.drawable.ic_stat_syncro)
            .setContentTitle("SyncroUmbral")
            .setContentText(Status.text)
            .apply {
                // Los botones solo tienen sentido con el PC conectado.
                if (Status.connected) {
                    addAction(action("Bloquear", ActionReceiver.ACTION_LOCK, 1, false))
                    addAction(action("Desbloquear", ActionReceiver.ACTION_UNLOCK, 2, true))
                }
            }
            // Al tocar el aviso se abre la app (antes había que buscarla).
            .setContentIntent(
                android.app.PendingIntent.getActivity(
                    this, 0, Intent(this, MainActivity::class.java).addFlags(Intent.FLAG_ACTIVITY_SINGLE_TOP),
                    android.app.PendingIntent.FLAG_IMMUTABLE or android.app.PendingIntent.FLAG_UPDATE_CURRENT
                )
            )
            .setOngoing(true)
            .setSilent(true)
            .build()

    companion object {
        private const val CHANNEL = "umbral_link"
        private const val NOTIF_ID = 1

        fun start(ctx: Context) {
            ctx.startForegroundService(Intent(ctx, LinkService::class.java))
        }

        fun stop(ctx: Context) {
            ctx.stopService(Intent(ctx, LinkService::class.java))
        }
    }
}
