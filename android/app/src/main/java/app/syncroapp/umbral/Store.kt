package app.syncroapp.umbral

import android.content.Context
import org.json.JSONArray
import org.json.JSONObject

/** Datos del PC emparejado. No guarda nada sensible: la clave privada vive en el Keystore. */
class Store(context: Context) {
    private val prefs = context.getSharedPreferences("umbral", Context.MODE_PRIVATE)

    var pcName: String
        get() = prefs.getString("pcName", "") ?: ""
        set(v) = prefs.edit().putString("pcName", v).apply()

    var port: Int
        get() = prefs.getInt("port", 47600)
        set(v) = prefs.edit().putInt("port", v).apply()

    /** Huella SHA-256 (hex) del certificado del PC. */
    var fingerprint: String
        get() = prefs.getString("fp", "") ?: ""
        set(v) = prefs.edit().putString("fp", v).apply()

    var hosts: List<String>
        get() {
            val arr = JSONArray(prefs.getString("hosts", "[]"))
            return List(arr.length()) { arr.getString(it) }
        }
        set(v) = prefs.edit().putString("hosts", JSONArray(v).toString()).apply()

    /** Token de emparejamiento pendiente; se borra cuando el PC confirma. */
    var pendingToken: String?
        get() = prefs.getString("pendingTok", null)
        set(v) = prefs.edit().apply { if (v == null) remove("pendingTok") else putString("pendingTok", v) }.apply()

    var paired: Boolean
        get() = prefs.getBoolean("paired", false)
        set(v) = prefs.edit().putBoolean("paired", v).apply()

    /** Mantener CPU y Wi-Fi despiertos mientras hay sesión con el PC (conexión estable con la pantalla apagada). Activado por defecto. */
    var keepAlive: Boolean
        get() = prefs.getBoolean("keepAlive", true)
        set(v) = prefs.edit().putBoolean("keepAlive", v).apply()

    val hasPc: Boolean get() = fingerprint.isNotEmpty() && hosts.isNotEmpty()

    /** Lo que trae el QR del PC, ya validado. Nada se guarda hasta que el usuario lo confirma. */
    class Pairing(val hosts: List<String>, val port: Int, val fp: String, val name: String, val token: String) {
        /** Huella corta para que el usuario la compare con la que muestra el PC: "ab12-cd34-ef56". */
        val shortFp: String get() = fp.take(12).chunked(4).joinToString("-")
    }

    companion object {
        private val IPV4 = Regex("""^(25[0-5]|2[0-4]\d|1?\d?\d)(\.(25[0-5]|2[0-4]\d|1?\d?\d)){3}$""")

        /** Valida el contenido del QR: direcciones IPv4 literales de red local, puerto, huella y código con su formato. */
        fun parsePairing(json: String): Pairing {
            val o = JSONObject(json)
            require(o.optInt("v") == 1) { "Versión de protocolo no soportada" }
            val arr = o.getJSONArray("hosts")
            val hosts = List(arr.length()) { arr.getString(it) }
            require(hosts.isNotEmpty() && hosts.size <= 8) { "Direcciones inválidas" }
            for (h in hosts) {
                require(IPV4.matches(h)) { "Dirección inválida: $h" }
                require(!h.startsWith("127.") && !h.startsWith("0.") && !h.startsWith("169.254.") && !h.startsWith("224.") && h != "255.255.255.255") {
                    "Dirección no permitida: $h"
                }
            }
            val port = o.getInt("port")
            require(port in 1024..65535) { "Puerto inválido" }
            val fp = o.getString("fp").lowercase()
            require(Regex("^[0-9a-f]{64}$").matches(fp)) { "Huella inválida" }
            val tok = o.getString("tok")
            require(Regex("^[0-9a-f]{32}$").matches(tok)) { "Código inválido" }
            // Nombre solo informativo: se limpia para que no pueda colar texto raro en la interfaz.
            val name = o.optString("name", "PC").filter { it.isLetterOrDigit() || it in " -_." }.trim().take(40).ifEmpty { "PC" }
            return Pairing(hosts, port, fp, name, tok)
        }
    }

    fun savePairing(p: Pairing) {
        hosts = p.hosts
        port = p.port
        fingerprint = p.fp
        pcName = p.name
        pendingToken = p.token
        paired = false
    }

    fun clear() {
        prefs.edit().clear().apply()
    }
}
