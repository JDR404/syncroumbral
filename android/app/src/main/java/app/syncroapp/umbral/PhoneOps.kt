package app.syncroapp.umbral

import android.content.Context
import android.content.pm.PackageManager
import android.media.AudioAttributes
import android.media.AudioManager
import android.media.Ringtone
import android.media.RingtoneManager
import android.net.Uri
import android.os.Handler
import android.os.Looper
import android.provider.CallLog
import android.provider.ContactsContract.CommonDataKinds.Phone as P
import android.telecom.TelecomManager
import org.json.JSONArray
import org.json.JSONObject

/** Contactos, historial de llamadas, marcar y "buscar mi móvil". Cada permiso se pide en la pestaña Permisos de la app. */
object PhoneOps {
    private const val MAX_CONTACTS = 400
    private val NUMBER = Regex("^[0-9+*#() -]{1,32}$")

    fun handle(ctx: Context, op: String, a: JSONObject): JSONObject = when (op) {
        "contacts" -> contacts(ctx, a.optString("q"))
        "calllog" -> callLog(ctx, a.optInt("limit", 60).coerceIn(1, 200))
        "dial" -> dial(ctx, a.getString("number"))
        "find_ring" -> ring(ctx, a.getBoolean("on"))
        "phone_perms" -> JSONObject().put("contacts", has(ctx, android.Manifest.permission.READ_CONTACTS))
            .put("call", has(ctx, android.Manifest.permission.CALL_PHONE)).put("log", has(ctx, android.Manifest.permission.READ_CALL_LOG))
        else -> throw IllegalArgumentException("Operación desconocida: $op")
    }

    private fun has(ctx: Context, p: String) = ctx.checkSelfPermission(p) == PackageManager.PERMISSION_GRANTED

    private fun need(ctx: Context, p: String, what: String) {
        if (!has(ctx, p)) throw IllegalStateException("Falta el permiso \"$what\" en el móvil (pestaña Permisos de la app)")
    }

    private fun contacts(ctx: Context, q: String): JSONObject {
        need(ctx, android.Manifest.permission.READ_CONTACTS, "Contactos")
        val sel = if (q.isBlank()) null else "${P.DISPLAY_NAME} LIKE ? OR ${P.NUMBER} LIKE ?"
        val args = if (q.isBlank()) null else arrayOf("%$q%", "%$q%")
        val byContact = LinkedHashMap<Long, Pair<String, MutableList<String>>>()
        ctx.contentResolver.query(
            P.CONTENT_URI, arrayOf(P.CONTACT_ID, P.DISPLAY_NAME, P.NUMBER),
            sel, args, "${P.DISPLAY_NAME} COLLATE NOCASE ASC"
        )?.use { c ->
            while (c.moveToNext() && byContact.size <= MAX_CONTACTS) {
                val e = byContact.getOrPut(c.getLong(0)) { (c.getString(1) ?: "") to mutableListOf() }
                val n = c.getString(2) ?: continue
                if (n !in e.second) e.second += n
            }
        }
        val arr = JSONArray()
        for (v in byContact.values.take(MAX_CONTACTS)) arr.put(JSONObject().put("name", v.first).put("numbers", JSONArray(v.second)))
        return JSONObject().put("items", arr)
    }

    private fun callLog(ctx: Context, limit: Int): JSONObject {
        need(ctx, android.Manifest.permission.READ_CALL_LOG, "Registro de llamadas")
        val arr = JSONArray()
        ctx.contentResolver.query(
            CallLog.Calls.CONTENT_URI, arrayOf(CallLog.Calls.NUMBER, CallLog.Calls.CACHED_NAME, CallLog.Calls.TYPE, CallLog.Calls.DATE, CallLog.Calls.DURATION),
            null, null, "${CallLog.Calls.DATE} DESC"
        )?.use { c ->
            while (c.moveToNext() && arr.length() < limit) {
                arr.put(
                    JSONObject().put("number", c.getString(0) ?: "").put("name", c.getString(1) ?: "").put("type", c.getInt(2))
                        .put("date", c.getLong(3)).put("secs", c.getLong(4))
                )
            }
        }
        return JSONObject().put("items", arr)
    }

    private fun dial(ctx: Context, number: String): JSONObject {
        need(ctx, android.Manifest.permission.CALL_PHONE, "Llamar")
        RateLimit.check("dial", 6, 10 * 60_000L, "llamadas")
        require(NUMBER.matches(number)) { "Número no válido" }
        val tm = ctx.getSystemService(Context.TELECOM_SERVICE) as TelecomManager
        @Suppress("MissingPermission")
        tm.placeCall(Uri.fromParts("tel", number.filter { it.isDigit() || it in "+*#" }, null), null)
        return JSONObject()
    }

    // ---- buscar mi móvil
    private var ringtone: Ringtone? = null
    private var savedVolume = -1
    private val main = Handler(Looper.getMainLooper())
    private var appCtx: Context? = null
    private val autoStop = Runnable { appCtx?.let { stopRing(it) } }

    @Synchronized
    private fun ring(ctx: Context, on: Boolean): JSONObject {
        appCtx = ctx.applicationContext
        if (on) startRing(ctx.applicationContext) else stopRing(ctx.applicationContext)
        return JSONObject().put("on", ringtone != null)
    }

    private fun startRing(ctx: Context) {
        if (ringtone != null) return
        val am = ctx.getSystemService(Context.AUDIO_SERVICE) as AudioManager
        savedVolume = am.getStreamVolume(AudioManager.STREAM_ALARM)
        am.setStreamVolume(AudioManager.STREAM_ALARM, am.getStreamMaxVolume(AudioManager.STREAM_ALARM), 0)
        val uri = RingtoneManager.getDefaultUri(RingtoneManager.TYPE_ALARM) ?: RingtoneManager.getDefaultUri(RingtoneManager.TYPE_RINGTONE)
        val r = RingtoneManager.getRingtone(ctx, uri)
        r.audioAttributes = AudioAttributes.Builder().setUsage(AudioAttributes.USAGE_ALARM).setContentType(AudioAttributes.CONTENT_TYPE_SONIFICATION).build()
        r.isLooping = true
        r.play()
        ringtone = r
        main.postDelayed(autoStop, 60_000) // nunca queda sonando para siempre
    }

    private fun stopRing(ctx: Context) {
        main.removeCallbacks(autoStop)
        ringtone?.stop()
        ringtone = null
        if (savedVolume >= 0) {
            (ctx.getSystemService(Context.AUDIO_SERVICE) as AudioManager).setStreamVolume(AudioManager.STREAM_ALARM, savedVolume, 0)
            savedVolume = -1
        }
    }
}
