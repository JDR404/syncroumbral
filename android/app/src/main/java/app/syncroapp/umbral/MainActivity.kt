package app.syncroapp.umbral

import android.Manifest
import android.app.Activity
import android.app.AlertDialog
import android.content.Intent
import android.content.pm.PackageManager
import android.graphics.Color
import android.graphics.drawable.GradientDrawable
import android.net.Uri
import android.os.Build
import android.os.Bundle
import android.os.Handler
import android.os.Looper
import android.os.PowerManager
import android.provider.Settings
import android.text.InputType
import android.view.Gravity
import android.view.View
import android.view.ViewGroup
import android.widget.Button
import android.widget.EditText
import android.widget.LinearLayout
import android.widget.ScrollView
import android.widget.Switch
import android.widget.TextView
import android.widget.Toast
import androidx.core.app.NotificationManagerCompat
import com.google.mlkit.vision.barcode.common.Barcode
import com.google.mlkit.vision.codescanner.GmsBarcodeScannerOptions
import com.google.mlkit.vision.codescanner.GmsBarcodeScanning

/**
 * Pantalla principal del móvil: estado de la conexión, vinculación con el PC y, sobre todo, la lista de permisos con
 * lo que ya está concedido y lo que falta (se actualiza sola cada segundo, así se ve al volver de los ajustes).
 */
class MainActivity : Activity() {
    private companion object {
        const val BG = 0xFF0E1415.toInt()
        const val SURFACE = 0xFF171D1E.toInt()
        const val SURFACE2 = 0xFF1F2A2C.toInt()
        const val PRIMARY = 0xFF4FD8EB.toInt()
        const val ON_PRIMARY = 0xFF00363D.toInt()
        const val TEXT = 0xFFDEE3E5.toInt()
        const val MUTED = 0xFFA3ADAF.toInt()
        const val OK = 0xFF78DC77.toInt()
        const val OK_BG = 0xFF12301A.toInt()
        const val BAD = 0xFFFFB4AB.toInt()
        const val BAD_BG = 0xFF3A1B18.toInt()
        const val WARN = 0xFFFFD8A8.toInt()
        const val WARN_BG = 0xFF3A2A14.toInt()
    }

    private lateinit var store: Store
    private lateinit var connView: TextView
    private lateinit var connSub: TextView
    private lateinit var permSummary: TextView
    private lateinit var warnCard: TextView
    private lateinit var unpairButton: Button
    private lateinit var keepAliveSwitch: Switch
    private val rows = ArrayList<PermRow>()
    private val handler = Handler(Looper.getMainLooper())
    private val refresh = object : Runnable {
        override fun run() {
            render()
            handler.postDelayed(this, 1000)
        }
    }

    /** Una fila de la lista de permisos. */
    private class PermRow(val title: String, val desc: String, val hint: String?, val granted: () -> Boolean, val ask: () -> Unit) {
        lateinit var pill: TextView
        lateinit var button: Button
        lateinit var hintView: TextView
    }

    private fun dp(v: Int) = (v * resources.displayMetrics.density).toInt()

    private fun shape(color: Int, radiusDp: Int) = GradientDrawable().apply {
        setColor(color)
        cornerRadius = dp(radiusDp).toFloat()
    }

    private fun text(s: String, sp: Float, color: Int = TEXT, bold: Boolean = false) = TextView(this).apply {
        text = s
        textSize = sp
        setTextColor(color)
        if (bold) setTypeface(typeface, android.graphics.Typeface.BOLD)
    }

    private fun card(): LinearLayout = LinearLayout(this).apply {
        orientation = LinearLayout.VERTICAL
        background = shape(SURFACE, 24)
        setPadding(dp(18), dp(16), dp(18), dp(16))
        layoutParams = LinearLayout.LayoutParams(ViewGroup.LayoutParams.MATCH_PARENT, ViewGroup.LayoutParams.WRAP_CONTENT)
            .apply { bottomMargin = dp(12) }
    }

    /** Botón redondeado: relleno (acción principal) o con contorno. */
    private fun pill(label: String, filled: Boolean = true, color: Int = PRIMARY, onClick: () -> Unit) = Button(this).apply {
        text = label
        isAllCaps = false
        textSize = 14f
        setTextColor(if (filled) ON_PRIMARY else color)
        minHeight = dp(44)
        minimumHeight = dp(44)
        setPadding(dp(20), 0, dp(20), 0)
        background = if (filled) shape(color, 22) else GradientDrawable().apply {
            setColor(Color.TRANSPARENT)
            cornerRadius = dp(22).toFloat()
            setStroke(dp(1), 0xFF3F4849.toInt())
        }
        stateListAnimator = null
        setOnClickListener { onClick() }
    }

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        store = Store(this)
        KeyManager.ensureKey()

        val root = LinearLayout(this).apply {
            orientation = LinearLayout.VERTICAL
            setPadding(dp(18), dp(56), dp(18), dp(24))
        }
        // Cabecera con el logo
        val header = LinearLayout(this).apply {
            orientation = LinearLayout.HORIZONTAL
            gravity = Gravity.CENTER_VERTICAL
            setPadding(dp(6), 0, 0, dp(14))
        }
        header.addView(android.widget.ImageView(this).apply {
            setImageResource(R.drawable.ic_logo)
            contentDescription = "SyncroUmbral"
        }, LinearLayout.LayoutParams(dp(44), dp(44)).apply { rightMargin = dp(12) })
        header.addView(text("SyncroUmbral", 28f, PRIMARY, true))
        root.addView(header)

        // Conexión
        val conn = card()
        connView = text("", 20f, TEXT, true)
        connSub = text("", 14f, MUTED).apply { setPadding(0, dp(2), 0, 0) }
        conn.addView(connView)
        conn.addView(connSub)
        root.addView(conn)

        // Aviso (batería, accesibilidad): solo si hay algo que decir
        warnCard = text("", 13f, WARN).apply {
            background = shape(WARN_BG, 20)
            setPadding(dp(16), dp(12), dp(16), dp(12))
            layoutParams = LinearLayout.LayoutParams(ViewGroup.LayoutParams.MATCH_PARENT, ViewGroup.LayoutParams.WRAP_CONTENT)
                .apply { bottomMargin = dp(12) }
        }
        root.addView(warnCard)

        // Vinculación
        val pairCard = card()
        pairCard.addView(text("Vincular con el PC", 17f, TEXT, true))
        pairCard.addView(text("En el PC: Móviles y ajustes, \"Mostrar QR de vinculación\".", 13f, MUTED).apply { setPadding(0, dp(2), 0, dp(10)) })
        pairCard.addView(pill("Escanear QR del PC") { scan() })
        val manual = EditText(this).apply {
            hint = "…o pega aquí el código de vinculación"
            setHintTextColor(MUTED)
            setTextColor(TEXT)
            textSize = 13f
            inputType = InputType.TYPE_CLASS_TEXT or InputType.TYPE_TEXT_FLAG_MULTI_LINE
            minLines = 2
            background = shape(SURFACE2, 16)
            setPadding(dp(14), dp(10), dp(14), dp(10))
        }
        pairCard.addView(manual, LinearLayout.LayoutParams(ViewGroup.LayoutParams.MATCH_PARENT, ViewGroup.LayoutParams.WRAP_CONTENT)
            .apply { topMargin = dp(10); bottomMargin = dp(8) })
        pairCard.addView(pill("Vincular con el código", filled = false) { pair(manual.text.toString().trim()) })
        unpairButton = pill("Desvincular", filled = false, color = BAD) { unpair() }
        pairCard.addView(unpairButton, LinearLayout.LayoutParams(ViewGroup.LayoutParams.WRAP_CONTENT, ViewGroup.LayoutParams.WRAP_CONTENT)
            .apply { topMargin = dp(8) })
        root.addView(pairCard)

        // Permisos
        val permCard = card()
        permCard.addView(text("Permisos", 17f, TEXT, true))
        permSummary = text("", 13f, MUTED).apply { setPadding(0, dp(2), 0, dp(6)) }
        permCard.addView(permSummary)
        buildRows()
        for (r in rows) permCard.addView(buildRow(r))
        root.addView(permCard)

        // Conexión estable
        val keepCard = card()
        keepAliveSwitch = Switch(this).apply {
            text = "Mantener la conexión con la pantalla apagada"
            setTextColor(TEXT)
            textSize = 14f
            isChecked = store.keepAlive
            setOnCheckedChangeListener { _, on ->
                store.keepAlive = on
                if (!on) KeepAlive.release()
            }
        }
        keepCard.addView(keepAliveSwitch)
        keepCard.addView(text("Evita que el PC se bloquee y desbloquee solo cuando el móvil duerme. Gasta algo de batería.", 12f, MUTED)
            .apply { setPadding(0, dp(4), 0, 0) })
        root.addView(keepCard)

        root.addView(pill("Enviar notificación de prueba", filled = false) { TestNotification.post(this) })

        setContentView(ScrollView(this).apply { setBackgroundColor(BG); addView(root) })

        if (Build.VERSION.SDK_INT >= 33 &&
            checkSelfPermission(Manifest.permission.POST_NOTIFICATIONS) != PackageManager.PERMISSION_GRANTED
        ) {
            requestPermissions(arrayOf(Manifest.permission.POST_NOTIFICATIONS), 1)
        }
        if (store.hasPc) LinkService.start(this)
        handleIntent(intent)
    }

    /** Los permisos que SyncroUmbral usa, qué hace cada uno y cómo se concede. */
    private fun buildRows() {
        rows += PermRow(
            "Acceso a los archivos", "Ver y copiar los archivos del móvil desde el PC.", null,
            { FileOps.hasAccess() }, { askFileAccess() }
        )
        rows += PermRow(
            "Acceso a las notificaciones", "Mostrar las notificaciones en el PC y responderlas.",
            "Si aparece atenuado: Ajustes, Aplicaciones, SyncroUmbral, menú de tres puntos, \"Permitir ajustes restringidos\".",
            { NotificationManagerCompat.getEnabledListenerPackages(this).contains(packageName) }, { askNotifAccess() }
        )
        rows += PermRow(
            "Avisos de la app", "Mostrar el aviso de que SyncroUmbral está conectado.", null,
            { Build.VERSION.SDK_INT < 33 || checkSelfPermission(Manifest.permission.POST_NOTIFICATIONS) == PackageManager.PERMISSION_GRANTED },
            { requestPermissions(arrayOf(Manifest.permission.POST_NOTIFICATIONS), 1) }
        )
        rows += PermRow(
            "Contestar llamadas", "Contestar y colgar llamadas normales desde el PC.", null,
            { CallOps.canAnswerPhone(this) }, { askAnswerCalls() }
        )
        rows += PermRow(
            "Mostrar sobre otras apps", "Necesario para contestar llamadas de WhatsApp y similares desde el PC.", null,
            { CallOps.canOverlay(this) }, { askOverlay() }
        )
        rows += PermRow(
            "Uso en segundo plano", "Que Android no corte la conexión al ahorrar batería.", null,
            { (getSystemService(POWER_SERVICE) as PowerManager).isIgnoringBatteryOptimizations(packageName) }, { ignoreBattery() }
        )
    }

    private fun buildRow(r: PermRow): View {
        val box = LinearLayout(this).apply {
            orientation = LinearLayout.VERTICAL
            background = shape(SURFACE2, 18)
            setPadding(dp(14), dp(12), dp(14), dp(12))
            layoutParams = LinearLayout.LayoutParams(ViewGroup.LayoutParams.MATCH_PARENT, ViewGroup.LayoutParams.WRAP_CONTENT)
                .apply { topMargin = dp(8) }
        }
        val top = LinearLayout(this).apply { orientation = LinearLayout.HORIZONTAL; gravity = Gravity.CENTER_VERTICAL }
        top.addView(text(r.title, 15f, TEXT, true), LinearLayout.LayoutParams(0, ViewGroup.LayoutParams.WRAP_CONTENT, 1f))
        r.pill = text("", 12f, OK, true).apply { setPadding(dp(12), dp(4), dp(12), dp(4)) }
        top.addView(r.pill)
        box.addView(top)
        box.addView(text(r.desc, 12f, MUTED).apply { setPadding(0, dp(2), 0, 0) })
        r.hintView = text(r.hint.orEmpty(), 12f, WARN).apply { setPadding(0, dp(4), 0, 0) }
        box.addView(r.hintView)
        r.button = pill("Permitir") { r.ask() }
        box.addView(r.button, LinearLayout.LayoutParams(ViewGroup.LayoutParams.WRAP_CONTENT, ViewGroup.LayoutParams.WRAP_CONTENT)
            .apply { topMargin = dp(8) })
        return box
    }

    override fun onNewIntent(intent: Intent) {
        super.onNewIntent(intent)
        handleIntent(intent)
    }

    /** Permite publicar la notificación de prueba desde adb: `am start -a app.syncroapp.umbral.TEST_NOTIF`. */
    private fun handleIntent(intent: Intent?) {
        // Solo en compilaciones de depuración: en la versión final otra app no puede disparar esto.
        val debuggable = applicationInfo.flags and android.content.pm.ApplicationInfo.FLAG_DEBUGGABLE != 0
        if (debuggable && intent?.action == "app.syncroapp.umbral.TEST_NOTIF") TestNotification.post(this)
        if (debuggable && intent?.action == "app.syncroapp.umbral.TEST_CALL") TestNotification.postCall(this)
        // La pantalla que abriría "Contestar" de una llamada de prueba: deja constancia en el registro y cierra la notificación.
        if (debuggable && intent?.action == "app.syncroapp.umbral.TEST_CALL_ANSWERED") {
            android.util.Log.i("UmbralCall", "CONTESTADA: la pantalla se abrio desde segundo plano")
            getSystemService(android.app.NotificationManager::class.java).cancel(4002)
        }
        // `am start -a app.syncroapp.umbral.TEST_PAIR --es payload '<json>'`: muestra el mismo diálogo de confirmación que el QR.
        if (debuggable && intent?.action == "app.syncroapp.umbral.TEST_PAIR") intent.getStringExtra("payload")?.let { pair(it) }
    }

    override fun onResume() {
        super.onResume()
        handler.post(refresh)
    }

    override fun onPause() {
        handler.removeCallbacks(refresh)
        super.onPause()
    }

    private fun render() {
        val connected = Link.ws != null
        connView.text = when {
            !store.hasPc -> "Sin vincular"
            connected -> "Conectado a ${store.pcName}"
            else -> "Buscando a ${store.pcName}…"
        }
        connView.setTextColor(if (store.hasPc && connected) OK else TEXT)
        // Conectado ya lo dice el título; el subtítulo solo aporta cuando algo falta o está en curso.
        connSub.text = when {
            !store.hasPc -> "Escanea el QR del PC para empezar."
            connected -> "El PC se desbloquea mientras el móvil esté en la red."
            else -> "${Status.text}${if (store.paired) "" else "  ·  pendiente de confirmar"}"
        }
        unpairButton.visibility = if (store.hasPc) View.VISIBLE else View.GONE

        var done = 0
        for (r in rows) {
            val ok = r.granted()
            if (ok) done++
            r.pill.text = if (ok) "Permitido" else "Falta"
            r.pill.setTextColor(if (ok) OK else BAD)
            r.pill.background = shape(if (ok) OK_BG else BAD_BG, 14)
            r.button.visibility = if (ok) View.GONE else View.VISIBLE
            r.hintView.visibility = if (!ok && r.hint != null) View.VISIBLE else View.GONE
        }
        permSummary.text = if (done == rows.size) "Todos los permisos concedidos." else "$done de ${rows.size} concedidos. Pulsa \"Permitir\" en los que faltan."
        permSummary.setTextColor(if (done == rows.size) OK else MUTED)

        if (keepAliveSwitch.isChecked != store.keepAlive) keepAliveSwitch.isChecked = store.keepAlive
        val warn = (batteryWarning() + accessibilityWarning()).trim()
        warnCard.visibility = if (warn.isEmpty()) View.GONE else View.VISIBLE
        warnCard.text = warn
    }

    /** El ahorro de batería y la optimización de SyncroUmbral son la causa típica de que la conexión se corte con la pantalla apagada. */
    private fun batteryWarning(): String {
        val pm = getSystemService(POWER_SERVICE) as PowerManager
        if (!pm.isPowerSaveMode) return ""
        return "El ahorro de batería está activo: Android puede cortar la conexión con el PC al apagarse la pantalla. Desactívalo mientras uses SyncroUmbral.\n\n"
    }

    /**
     * Los servicios de accesibilidad pueden leer la pantalla y pulsar botones por el usuario, incluidos los de
     * confirmación de SyncroUmbral. No se bloquea nada (muchos son legítimos), pero se avisa para que el usuario lo sepa.
     */
    private fun accessibilityWarning(): String {
        val raw = Settings.Secure.getString(contentResolver, Settings.Secure.ENABLED_ACCESSIBILITY_SERVICES).orEmpty()
        val pkgs = raw.split(':').mapNotNull { it.substringBefore('/').takeIf { p -> p.isNotBlank() } }.distinct()
        if (pkgs.isEmpty()) return ""
        return "Hay servicios de accesibilidad activos (${pkgs.joinToString(", ")}). Pueden pulsar botones por ti, incluidas las " +
            "confirmaciones de SyncroUmbral; algunas apps de banco también se niegan a abrirse con ellos."
    }

    private fun scan() {
        val options = GmsBarcodeScannerOptions.Builder().setBarcodeFormats(Barcode.FORMAT_QR_CODE).build()
        GmsBarcodeScanning.getClient(this, options).startScan()
            .addOnSuccessListener { pair(it.rawValue.orEmpty()) }
            .addOnFailureListener { toast("No se pudo escanear: ${it.message}") }
    }

    /**
     * Valida lo leído del QR y pide confirmación mostrando a qué PC se va a vincular: un QR ajeno (un cartel, una imagen
     * recibida) no debe poder vincular el móvil sin que el usuario lo vea. La huella corta se compara con la del PC.
     */
    private fun pair(payload: String) {
        val p = try {
            Store.parsePairing(payload)
        } catch (e: Exception) {
            toast("Código inválido")
            return
        }
        AlertDialog.Builder(this)
            .setTitle("Vincular con este PC")
            .setMessage(
                "Nombre: ${p.name}\nDirección: ${p.hosts.joinToString(", ")}\nHuella: ${p.shortFp}\n\n" +
                    "Confirma que la huella es la misma que muestra SyncroUmbral en tu PC. Quien se vincula puede ver tus " +
                    "archivos y notificaciones. Si no fuiste tú quien mostró este código, cancela."
            )
            .setPositiveButton("Vincular") { _, _ ->
                store.savePairing(p)
                LinkService.stop(this)
                LinkService.start(this)
                toast("Vinculando con ${store.pcName}…")
            }
            .setNegativeButton("Cancelar", null)
            .show()
    }

    private fun unpair() {
        LinkService.stop(this)
        store.clear()
        KeyManager.reset()
        KeyManager.ensureKey()
        Status.set("Sin vincular")
        toast("Desvinculado")
    }

    /** Permiso para contestar y colgar llamadas normales (línea del operador) desde el PC. */
    private fun askAnswerCalls() {
        requestPermissions(arrayOf(Manifest.permission.ANSWER_PHONE_CALLS), 3)
    }

    /** Sin este permiso Android no deja contestar una llamada de WhatsApp (u otra app) cuando SyncroUmbral está en segundo plano. */
    private fun askOverlay() {
        startActivity(Intent(Settings.ACTION_MANAGE_OVERLAY_PERMISSION, Uri.parse("package:$packageName")))
    }

    private fun askNotifAccess() {
        startActivity(Intent(Settings.ACTION_NOTIFICATION_LISTENER_SETTINGS))
    }

    private fun askFileAccess() {
        if (Build.VERSION.SDK_INT >= 30) {
            startActivity(
                Intent(Settings.ACTION_MANAGE_APP_ALL_FILES_ACCESS_PERMISSION, Uri.parse("package:$packageName"))
            )
        } else {
            requestPermissions(
                arrayOf(Manifest.permission.READ_EXTERNAL_STORAGE, Manifest.permission.WRITE_EXTERNAL_STORAGE), 2
            )
        }
    }

    private fun ignoreBattery() {
        startActivity(
            Intent(Settings.ACTION_REQUEST_IGNORE_BATTERY_OPTIMIZATIONS, Uri.parse("package:$packageName"))
        )
    }

    private fun toast(msg: String) = Toast.makeText(this, msg, Toast.LENGTH_LONG).show()
}
