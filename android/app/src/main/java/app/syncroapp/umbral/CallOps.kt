package app.syncroapp.umbral

import android.app.ActivityOptions
import android.app.Notification
import android.app.PendingIntent
import android.content.Context
import android.content.Intent
import android.content.pm.PackageManager
import android.os.Build
import android.provider.Settings
import android.service.notification.StatusBarNotification
import android.telecom.TelecomManager
import org.json.JSONObject

/**
 * Contestar y colgar llamadas desde el PC. Hay dos tipos de llamada y cada una se atiende distinto:
 *
 * - **Normales** (línea del operador, SIM): las gestiona el sistema de telefonía de Android. Se contestan con
 *   `TelecomManager.acceptRingingCall()` y se cuelgan con `endCall()`; requieren el permiso "Contestar llamadas".
 * - **De apps** (WhatsApp, Telegram, Messenger...): la app publica una notificación con botones. Se pulsa el botón
 *   "Contestar" de esa notificación. Contestar abre la pantalla de la llamada, y Android 10 o superior prohíbe que una app
 *   en segundo plano abra pantallas; solo se permite si SyncroUmbral tiene "Mostrar sobre otras apps". Colgar no abre nada
 *   (es un aviso interno), por eso colgar siempre funcionaba y contestar no.
 */
object CallOps {
    private val ANSWER = Regex("(?i)contest|respond|answer|accept|acept|atender|aceptar")
    private val END = Regex("(?i)rechaz|declin|reject|colgar|hang|\\bend\\b|finaliz|cortar|terminar")

    fun handle(ctx: Context, op: String, a: JSONObject): JSONObject = when (op) {
        "call_answer" -> answer(ctx, a.optString("key"))
        "call_end" -> end(ctx, a.optString("key"))
        "call_perms" -> perms(ctx)
        else -> throw IllegalArgumentException("Operación desconocida: $op")
    }

    fun canAnswerPhone(ctx: Context): Boolean =
        ctx.checkSelfPermission(android.Manifest.permission.ANSWER_PHONE_CALLS) == PackageManager.PERMISSION_GRANTED

    /**
     * ¿Este APK declara el permiso "Mostrar sobre otras apps"? Es opcional y solo viene en la variante con superposición:
     * pedirlo junto al acceso a notificaciones es justo el patrón que Google Play Protect bloquea en apps instaladas a mano.
     */
    @Suppress("DEPRECATION")
    fun overlayAvailable(ctx: Context): Boolean = try {
        ctx.packageManager.getPackageInfo(ctx.packageName, PackageManager.GET_PERMISSIONS)
            .requestedPermissions.orEmpty().contains(android.Manifest.permission.SYSTEM_ALERT_WINDOW)
    } catch (_: Exception) { false }

    fun canOverlay(ctx: Context): Boolean = overlayAvailable(ctx) && Settings.canDrawOverlays(ctx)

    private fun perms(ctx: Context) = JSONObject().put("answerPhone", canAnswerPhone(ctx))
        .put("overlay", canOverlay(ctx)).put("overlayAvailable", overlayAvailable(ctx))

    private fun telecom(ctx: Context) = ctx.getSystemService(Context.TELECOM_SERVICE) as TelecomManager

    /** ¿La notificación es de la app de teléfono del sistema (llamada normal) y no de una app de mensajería? */
    private fun isCellular(ctx: Context, pkg: String): Boolean {
        val dialer = try { telecom(ctx).defaultDialerPackage } catch (_: Exception) { null }
        return pkg == dialer || pkg == "com.android.server.telecom" || pkg == "com.android.phone" ||
            pkg == "com.google.android.dialer" || pkg == "com.samsung.android.incallui" || pkg == "com.android.incallui"
    }

    private fun sbnFor(key: String): StatusBarNotification? =
        if (key.isEmpty()) null else NotifService.instance?.activeNotifications.orEmpty().firstOrNull { it.key == key }

    /** Primera llamada entrante visible si el PC no indicó cuál. */
    private fun pick(key: String): StatusBarNotification? =
        sbnFor(key) ?: NotifService.instance?.activeNotifications.orEmpty()
            .firstOrNull { it.notification.category == Notification.CATEGORY_CALL }

    @Suppress("MissingPermission")
    private fun answer(ctx: Context, key: String): JSONObject {
        val sbn = pick(key)
        val cellular = sbn == null || isCellular(ctx, sbn.packageName)
        if (cellular) {
            if (!canAnswerPhone(ctx)) {
                // Sin ese permiso aún se puede probar el botón de la notificación (la app de teléfono lo trae).
                if (sbn != null && press(ctx, sbn, ANSWER, false)) return JSONObject().put("via", "notificacion")
                throw IllegalStateException("Falta el permiso \"Contestar llamadas\" en la app SyncroUmbral del móvil")
            }
            telecom(ctx).acceptRingingCall()
            return JSONObject().put("via", "telefonia")
        }
        if (!press(ctx, sbn!!, ANSWER, true)) {
            throw IllegalStateException("La notificación de la llamada no tiene botón para contestar")
        }
        // Enviar el botón no garantiza nada: si Android bloquea abrir la pantalla, no avisa. Se comprueba que la notificación
        // dejó de ofrecer "Contestar" (la llamada pasó a estar en curso o terminó).
        if (!waitAnswered(sbn.key)) {
            throw IllegalStateException("Android no dejó abrir la llamada desde segundo plano. Contéstala en el móvil.")
        }
        return JSONObject().put("via", "notificacion")
    }

    private fun waitAnswered(key: String): Boolean {
        val until = System.currentTimeMillis() + 3000
        while (System.currentTimeMillis() < until) {
            val s = sbnFor(key)
            if (s == null || !hasAnswerAction(s.notification)) return true
            Thread.sleep(200)
        }
        return false
    }

    @Suppress("MissingPermission")
    private fun end(ctx: Context, key: String): JSONObject {
        val sbn = pick(key)
        if (sbn != null && press(ctx, sbn, END, false)) return JSONObject().put("via", "notificacion")
        if (!canAnswerPhone(ctx)) {
            throw IllegalStateException("No se pudo colgar: falta el permiso \"Contestar llamadas\" en la app SyncroUmbral del móvil")
        }
        if (Build.VERSION.SDK_INT >= 28 && (sbn == null || isCellular(ctx, sbn.packageName)) && telecom(ctx).endCall()) {
            return JSONObject().put("via", "telefonia")
        }
        throw IllegalStateException("No hay ninguna llamada que colgar")
    }

    /**
     * Pulsa el botón de la notificación cuyo título coincide. `opensScreen`: la acción abre la pantalla de la llamada, así que
     * se envía permitiendo explícitamente abrir actividades desde segundo plano (hace falta "Mostrar sobre otras apps").
     */
    private fun press(ctx: Context, sbn: StatusBarNotification, wanted: Regex, opensScreen: Boolean): Boolean {
        val act = sbn.notification.actions?.firstOrNull { wanted.containsMatchIn(it.title?.toString().orEmpty()) }
        val pi: PendingIntent = act?.actionIntent ?: return false
        if (opensScreen && overlayAvailable(ctx) && !canOverlay(ctx)) {
            throw IllegalStateException(
                "Android no deja contestar desde segundo plano: activa \"Mostrar sobre otras apps\" para SyncroUmbral en el móvil"
            )
        }
        val options = if (opensScreen && Build.VERSION.SDK_INT >= 34) {
            ActivityOptions.makeBasic().apply {
                // Solo el modo del que envía (nosotros): el del creador lo decide la app dueña de la notificación.
                setPendingIntentBackgroundActivityStartMode(ActivityOptions.MODE_BACKGROUND_ACTIVITY_START_ALLOWED)
            }.toBundle()
        } else null
        try {
            pi.send(ctx, 0, Intent(), null, null, null, options)
        } catch (e: PendingIntent.CanceledException) {
            throw IllegalStateException("La llamada ya no está disponible")
        }
        return true
    }

    /** Heurística: ¿alguna acción es "Contestar"? Sirve para reconocer llamadas entrantes que no usan CallStyle. */
    fun hasAnswerAction(n: Notification): Boolean =
        n.actions?.any { ANSWER.containsMatchIn(it.title?.toString().orEmpty()) } == true
}
