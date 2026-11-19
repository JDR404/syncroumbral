package app.syncroapp.umbral

import android.content.Context
import android.content.pm.PackageManager
import android.net.Uri
import android.provider.ContactsContract
import android.provider.Telephony
import android.telephony.SmsManager
import org.json.JSONArray
import org.json.JSONObject

/**
 * SMS desde el PC: conversaciones, mensajes de una conversación y envío. Solo SMS: los MMS no se leen y RCS no es accesible
 * para apps de terceros. Un SMS enviado por una app que no es la predeterminada no queda guardado en la app Mensajes del móvil
 * (Android no lo permite), por eso el PC conserva en pantalla lo que envía.
 */
object SmsOps {
    private const val MAX_THREADS = 60
    private const val SCAN_ROWS = 1500
    private val ADDRESS = Regex("^[0-9+*#() -]{1,32}$")

    fun handle(ctx: Context, op: String, a: JSONObject): JSONObject = when (op) {
        "sms_threads" -> threads(ctx)
        "sms_list" -> list(ctx, a.getString("address"), a.optInt("limit", 80).coerceIn(1, 300))
        "sms_send" -> send(ctx, a.getString("address"), a.getString("text"))
        "sms_perms" -> JSONObject().put("read", has(ctx, android.Manifest.permission.READ_SMS)).put("send", has(ctx, android.Manifest.permission.SEND_SMS))
        else -> throw IllegalArgumentException("Operación desconocida: $op")
    }

    private fun has(ctx: Context, p: String) = ctx.checkSelfPermission(p) == PackageManager.PERMISSION_GRANTED

    private fun need(ctx: Context, p: String, what: String) {
        if (!has(ctx, p)) throw IllegalStateException("Falta el permiso \"$what\" en el móvil (pestaña Permisos de la app)")
    }

    private fun name(ctx: Context, address: String): String {
        if (!has(ctx, android.Manifest.permission.READ_CONTACTS)) return ""
        return try {
            ctx.contentResolver.query(
                Uri.withAppendedPath(ContactsContract.PhoneLookup.CONTENT_FILTER_URI, Uri.encode(address)),
                arrayOf(ContactsContract.PhoneLookup.DISPLAY_NAME), null, null, null
            )?.use { if (it.moveToFirst()) it.getString(0) ?: "" else "" } ?: ""
        } catch (_: Exception) { "" }
    }

    private fun threads(ctx: Context): JSONObject {
        need(ctx, android.Manifest.permission.READ_SMS, "SMS")
        val seen = LinkedHashMap<String, JSONObject>()
        ctx.contentResolver.query(
            Telephony.Sms.CONTENT_URI, arrayOf(Telephony.Sms.ADDRESS, Telephony.Sms.BODY, Telephony.Sms.DATE, Telephony.Sms.TYPE, Telephony.Sms.READ),
            null, null, "${Telephony.Sms.DATE} DESC"
        )?.use { c ->
            var rows = 0
            while (c.moveToNext() && rows++ < SCAN_ROWS && seen.size < MAX_THREADS) {
                val addr = c.getString(0) ?: continue
                if (addr in seen) continue
                seen[addr] = JSONObject().put("address", addr).put("snippet", (c.getString(1) ?: "").take(120)).put("date", c.getLong(2))
                    .put("mine", c.getInt(3) != 1).put("unread", c.getInt(3) == 1 && c.getInt(4) == 0)
            }
        }
        val arr = JSONArray()
        for (t in seen.values) arr.put(t.put("name", name(ctx, t.getString("address"))))
        return JSONObject().put("items", arr)
    }

    private fun list(ctx: Context, address: String, limit: Int): JSONObject {
        need(ctx, android.Manifest.permission.READ_SMS, "SMS")
        val arr = JSONArray()
        ctx.contentResolver.query(
            Telephony.Sms.CONTENT_URI, arrayOf(Telephony.Sms.BODY, Telephony.Sms.DATE, Telephony.Sms.TYPE),
            "${Telephony.Sms.ADDRESS} = ?", arrayOf(address), "${Telephony.Sms.DATE} DESC"
        )?.use { c ->
            while (c.moveToNext() && arr.length() < limit) {
                arr.put(JSONObject().put("text", c.getString(0) ?: "").put("date", c.getLong(1)).put("mine", c.getInt(2) != 1))
            }
        }
        return JSONObject().put("items", arr)
    }

    @Suppress("DEPRECATION")
    private fun send(ctx: Context, address: String, text: String): JSONObject {
        need(ctx, android.Manifest.permission.SEND_SMS, "Enviar SMS")
        require(ADDRESS.matches(address)) { "Número no válido" }
        require(text.isNotBlank() && text.length <= 1000) { "El mensaje está vacío o es demasiado largo" }
        val sm = ctx.getSystemService(SmsManager::class.java) ?: SmsManager.getDefault()
        sm.sendMultipartTextMessage(address, null, sm.divideMessage(text), null, null)
        return JSONObject()
    }
}
