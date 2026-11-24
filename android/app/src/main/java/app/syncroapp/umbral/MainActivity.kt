package app.syncroapp.umbral

import android.Manifest
import android.app.Activity
import android.app.AlertDialog
import android.content.ClipData
import android.content.ClipboardManager
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
import android.widget.FrameLayout
import android.widget.LinearLayout
import android.widget.ScrollView
import android.widget.Switch
import android.widget.TextView
import android.widget.Toast
import androidx.core.app.NotificationManagerCompat
import com.google.mlkit.vision.barcode.common.Barcode
import com.google.mlkit.vision.codescanner.GmsBarcodeScannerOptions
import com.google.mlkit.vision.codescanner.GmsBarcodeScanning
import org.json.JSONObject

/**
 * Aplicación de gestión de SyncroUmbral en el móvil, separada en cuatro secciones:
 * Inicio (estado y acciones rápidas), Permisos, Ajustes (modo, conexión, vinculación, pruebas) e Información.
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
        const val REPO = "https://github.com/JDR404/syncroumbral"
        const val REQ_PICK = 10
        val PAGES = listOf("Inicio", "Permisos", "Ajustes", "Info")
    }

    private lateinit var store: Store
    private lateinit var connView: TextView
    private lateinit var connSub: TextView
    private lateinit var permAlert: LinearLayout
    private lateinit var permAlertText: TextView
    private lateinit var a11yAlert: LinearLayout
    private lateinit var a11yText: TextView
    private lateinit var batteryAlert: TextView
    private lateinit var pairCard: LinearLayout
    private lateinit var quickCard: LinearLayout
    private lateinit var permSummary: TextView
    private lateinit var linkedText: TextView
    private lateinit var unpairButton: Button
    private lateinit var keepAliveSwitch: Switch
    private lateinit var modeBasic: Button
    private lateinit var modeAdvanced: Button
    private lateinit var modeText: TextView
    private val rows = ArrayList<PermRow>()
    private val pages = ArrayList<View>()
    private val navButtons = ArrayList<TextView>()
    private var page = 0
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

    private fun lp(top: Int = 0, bottom: Int = 0) =
        LinearLayout.LayoutParams(ViewGroup.LayoutParams.MATCH_PARENT, ViewGroup.LayoutParams.WRAP_CONTENT).apply {
            topMargin = dp(top); bottomMargin = dp(bottom)
        }

    private fun card(): LinearLayout = LinearLayout(this).apply {
        orientation = LinearLayout.VERTICAL
        background = shape(SURFACE, 24)
        setPadding(dp(18), dp(16), dp(18), dp(16))
        layoutParams = lp(bottom = 12)
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

    private fun styleSeg(b: Button, active: Boolean) {
        b.background = shape(if (active) PRIMARY else SURFACE2, 22)
        b.setTextColor(if (active) ON_PRIMARY else TEXT)
    }

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        store = Store(this)
        KeyManager.ensureKey()
        buildRows()

        val outer = LinearLayout(this).apply { orientation = LinearLayout.VERTICAL; setBackgroundColor(BG) }

        // Cabecera: el logo va dentro de un recuadro con margen interno para que respire junto al título.
        val header = LinearLayout(this).apply {
            orientation = LinearLayout.HORIZONTAL
            gravity = Gravity.CENTER_VERTICAL
            setPadding(dp(20), dp(48), dp(20), dp(8))
        }
        header.addView(android.widget.ImageView(this).apply {
            setImageResource(R.drawable.ic_logo)
            contentDescription = "SyncroUmbral"
            background = shape(SURFACE, 14)
            setPadding(dp(7), dp(7), dp(7), dp(7))
        }, LinearLayout.LayoutParams(dp(46), dp(46)).apply { rightMargin = dp(14) })
        header.addView(text("SyncroUmbral", 26f, PRIMARY, true))
        outer.addView(header)

        val body = FrameLayout(this)
        pages += scroll(buildHome())
        pages += scroll(buildPermissions())
        pages += scroll(buildSettings())
        pages += scroll(buildInfo())
        for (p in pages) body.addView(p)
        outer.addView(body, LinearLayout.LayoutParams(ViewGroup.LayoutParams.MATCH_PARENT, 0, 1f))

        // Barra inferior de secciones
        val nav = LinearLayout(this).apply {
            orientation = LinearLayout.HORIZONTAL
            setBackgroundColor(SURFACE)
            setPadding(dp(8), dp(6), dp(8), dp(18))
        }
        PAGES.forEachIndexed { i, name ->
            val t = text(name, 14f, MUTED, true).apply {
                gravity = Gravity.CENTER
                setPadding(0, dp(12), 0, dp(12))
                setOnClickListener { show(i) }
            }
            navButtons += t
            nav.addView(t, LinearLayout.LayoutParams(0, ViewGroup.LayoutParams.WRAP_CONTENT, 1f).apply { marginStart = dp(3); marginEnd = dp(3) })
        }
        outer.addView(nav)
        setContentView(outer)
        // Android 15 dibuja bajo las barras del sistema: los márgenes se calculan con las barras reales (y el recorte de la cámara).
        androidx.core.view.ViewCompat.setOnApplyWindowInsetsListener(outer) { _, insets ->
            val bars = insets.getInsets(
                androidx.core.view.WindowInsetsCompat.Type.systemBars() or androidx.core.view.WindowInsetsCompat.Type.displayCutout()
            )
            header.setPadding(dp(20) + bars.left, bars.top + dp(16), dp(20) + bars.right, dp(8))
            nav.setPadding(dp(8) + bars.left, dp(6), dp(8) + bars.right, bars.bottom + dp(8))
            insets
        }
        outer.requestApplyInsets()
        show(savedInstanceState?.getInt("page") ?: 0)

        if (Build.VERSION.SDK_INT >= 33 &&
            checkSelfPermission(Manifest.permission.POST_NOTIFICATIONS) != PackageManager.PERMISSION_GRANTED
        ) {
            requestPermissions(arrayOf(Manifest.permission.POST_NOTIFICATIONS), 1)
        }
        if (store.hasPc) LinkService.start(this)
        handleIntent(intent)
    }

    private fun scroll(content: View) = ScrollView(this).apply {
        isVerticalScrollBarEnabled = false
        addView(content)
    }

    private fun column() = LinearLayout(this).apply {
        orientation = LinearLayout.VERTICAL
        setPadding(dp(18), dp(8), dp(18), dp(20))
    }

    private fun show(i: Int) {
        page = i
        pages.forEachIndexed { n, v -> v.visibility = if (n == i) View.VISIBLE else View.GONE }
        navButtons.forEachIndexed { n, t ->
            t.setTextColor(if (n == i) ON_PRIMARY else MUTED)
            t.background = if (n == i) shape(PRIMARY, 20) else null
        }
    }

    // ------------------------------------------------------------------ Inicio

    private fun buildHome(): View {
        val c = column()

        val conn = card()
        connView = text("", 20f, TEXT, true)
        connSub = text("", 14f, MUTED).apply { setPadding(0, dp(2), 0, 0) }
        conn.addView(connView)
        conn.addView(connSub)
        c.addView(conn)

        // Avisos compactos: solo aparecen si hay algo que decir
        permAlert = alertRow("Revisar") { show(1) }.also { permAlertText = it.getChildAt(0) as TextView }
        c.addView(permAlert)
        a11yAlert = alertRow("Entendido") {
            store.dismissedA11y = accessibilityPackages().joinToString(",")
            render()
        }.also { a11yText = it.getChildAt(0) as TextView }
        c.addView(a11yAlert)
        batteryAlert = text("", 13f, WARN).apply {
            background = shape(WARN_BG, 20)
            setPadding(dp(16), dp(12), dp(16), dp(12))
            layoutParams = lp(bottom = 12)
        }
        c.addView(batteryAlert)

        // Sin PC vinculado: vincular es lo único que importa
        pairCard = card()
        pairCard.addView(text("Vincular con el PC", 17f, TEXT, true))
        pairCard.addView(text("En el PC: Móviles y ajustes, \"Mostrar QR de vinculación\". ¿No tienes SyncroUmbral en el PC? Mira la sección Info.", 13f, MUTED)
            .apply { setPadding(0, dp(2), 0, dp(10)) })
        addPairingControls(pairCard)
        c.addView(pairCard)

        // Acciones rápidas (solo con el PC conectado)
        quickCard = card()
        quickCard.addView(text("Acciones rápidas", 17f, TEXT, true).apply { setPadding(0, 0, 0, dp(8)) })
        quickCard.addView(pill("Enviar archivo al PC") { pickFiles() }, lp(bottom = 8))
        quickCard.addView(pill("Enviar portapapeles al PC", filled = false) { sendClipboard() }, lp(bottom = 8))
        quickCard.addView(pill("Bloquear SyncroUmbral en el PC", filled = false, color = BAD) { lockPc() }, lp(bottom = 8))
        quickCard.addView(pill("Desbloquear SyncroUmbral en el PC", filled = false) { unlockPc() })
        c.addView(quickCard)
        return c
    }

    /** Fila de aviso: texto + botón corto a la derecha. */
    private fun alertRow(button: String, onClick: () -> Unit) = LinearLayout(this).apply {
        orientation = LinearLayout.HORIZONTAL
        gravity = Gravity.CENTER_VERTICAL
        background = shape(WARN_BG, 20)
        setPadding(dp(16), dp(8), dp(8), dp(8))
        layoutParams = lp(bottom = 12)
        addView(text("", 13f, WARN), LinearLayout.LayoutParams(0, ViewGroup.LayoutParams.WRAP_CONTENT, 1f))
        addView(Button(context).apply {
            text = button
            isAllCaps = false
            textSize = 13f
            setTextColor(WARN)
            background = null
            minHeight = dp(40); minimumHeight = dp(40)
            setOnClickListener { onClick() }
        })
    }

    private fun addPairingControls(target: LinearLayout) {
        target.addView(pill("Escanear QR del PC") { scan() })
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
        target.addView(manual, lp(top = 10, bottom = 8))
        target.addView(pill("Vincular con el código", filled = false) { pair(manual.text.toString().trim()) })
    }

    // ---------------------------------------------------------------- Permisos

    private fun buildPermissions(): View {
        val c = column()
        val permCard = card()
        permCard.addView(text("Permisos", 17f, TEXT, true))
        permSummary = text("", 13f, MUTED).apply { setPadding(0, dp(2), 0, dp(6)) }
        permCard.addView(permSummary)
        for (r in rows) permCard.addView(buildRow(r))
        c.addView(permCard)
        return c
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
            "Contactos", "Ver tus contactos en el PC para llamarles (opcional).", null,
            { checkSelfPermission(Manifest.permission.READ_CONTACTS) == PackageManager.PERMISSION_GRANTED },
            { requestPermissions(arrayOf(Manifest.permission.READ_CONTACTS), 4) }
        )
        rows += PermRow(
            "Llamar y registro de llamadas", "Marcar desde el PC y ver el historial de llamadas (opcional).", null,
            {
                checkSelfPermission(Manifest.permission.CALL_PHONE) == PackageManager.PERMISSION_GRANTED &&
                    checkSelfPermission(Manifest.permission.READ_CALL_LOG) == PackageManager.PERMISSION_GRANTED
            },
            { requestPermissions(arrayOf(Manifest.permission.CALL_PHONE, Manifest.permission.READ_CALL_LOG), 5) }
        )
        rows += PermRow(
            "Mensajes SMS", "Leer y enviar SMS desde el PC (opcional; no incluye MMS ni RCS).", null,
            {
                checkSelfPermission(Manifest.permission.READ_SMS) == PackageManager.PERMISSION_GRANTED &&
                    checkSelfPermission(Manifest.permission.SEND_SMS) == PackageManager.PERMISSION_GRANTED
            },
            { requestPermissions(arrayOf(Manifest.permission.READ_SMS, Manifest.permission.SEND_SMS), 6) }
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
            layoutParams = lp(top = 8)
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

    // ----------------------------------------------------------------- Ajustes

    private fun buildSettings(): View {
        val c = column()

        // Modo de uso (se sincroniza con el PC)
        val mode = card()
        mode.addView(text("Modo de uso", 17f, TEXT, true))
        val seg = LinearLayout(this).apply { orientation = LinearLayout.HORIZONTAL; setPadding(0, dp(10), 0, dp(6)) }
        modeBasic = pill("Básico", filled = false) { chooseMode(false) }
        modeAdvanced = pill("Avanzado", filled = false) { chooseMode(true) }
        seg.addView(modeBasic, LinearLayout.LayoutParams(0, ViewGroup.LayoutParams.WRAP_CONTENT, 1f).apply { marginEnd = dp(6) })
        seg.addView(modeAdvanced, LinearLayout.LayoutParams(0, ViewGroup.LayoutParams.WRAP_CONTENT, 1f))
        mode.addView(seg)
        modeText = text("", 12f, MUTED)
        mode.addView(modeText)
        c.addView(mode)

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
        c.addView(keepCard)

        // Dispositivo vinculado
        val linked = card()
        linked.addView(text("PC vinculado", 17f, TEXT, true))
        linkedText = text("", 13f, MUTED).apply { setPadding(0, dp(2), 0, dp(10)) }
        linked.addView(linkedText)
        unpairButton = pill("Desvincular", filled = false, color = BAD) { unpair() }
        linked.addView(unpairButton, lp(bottom = 8))
        linked.addView(text("Vincular otro PC", 14f, TEXT, true).apply { setPadding(0, dp(8), 0, dp(6)) })
        addPairingControls(linked)
        c.addView(linked)

        // Pruebas
        val tests = card()
        tests.addView(text("Pruebas", 17f, TEXT, true))
        tests.addView(text("Publica una notificación de prueba en este móvil para ver cómo llega al PC.", 12f, MUTED)
            .apply { setPadding(0, dp(2), 0, dp(10)) })
        tests.addView(pill("Enviar notificación de prueba", filled = false) { TestNotification.post(this) })
        c.addView(tests)
        return c
    }

    private fun chooseMode(advanced: Boolean) {
        store.advanced = advanced
        // Si hay conexión, el PC lo recibe al momento; si no, lo recibe al conectar.
        Link.send(JSONObject().put("t", "mode").put("advanced", advanced))
        render()
    }

    // -------------------------------------------------------------------- Info

    private fun buildInfo(): View {
        val c = column()
        val what = card()
        what.addView(text("Qué es SyncroUmbral", 17f, TEXT, true))
        what.addView(text(
            "Este móvil es la llave de la aplicación SyncroUmbral del PC (no de Windows): mientras esté en tu red, la aplicación " +
                "se mantiene abierta; si te alejas, se bloquea y solo abre con la clave de respaldo. No bloquea la sesión de Windows. " +
                "Además muestra aquí tus notificaciones en el PC (con respuesta), " +
                "contesta llamadas, comparte archivos y portapapeles y te deja ver los archivos del móvil desde el PC.",
            13f, MUTED
        ).apply { setPadding(0, dp(4), 0, 0) })
        c.addView(what)

        val download = card()
        download.addView(text("Instalar el programa del PC", 17f, TEXT, true))
        download.addView(text(
            "Necesitas la parte de Windows (10 u 11, 64 bits). El instalador se llama SyncroUmbral_0.2.0_x64-setup.exe y está en la " +
                "carpeta de entrega junto a este APK, y en el repositorio privado del proyecto (cuenta JDR404):\n$REPO\n\n" +
                "1. Instálalo en el PC y ábrelo (se queda en la bandeja y arranca con Windows).\n" +
                "2. Crea la clave de respaldo.\n" +
                "3. En \"Móviles y ajustes\" pulsa \"Mostrar QR de vinculación\".\n" +
                "4. Aquí, en Inicio, pulsa \"Escanear QR del PC\" y confirma que la huella coincide.",
            13f, MUTED
        ).apply { setPadding(0, dp(4), 0, dp(10)) })
        download.addView(pill("Copiar enlace del repositorio", filled = false) {
            (getSystemService(CLIPBOARD_SERVICE) as ClipboardManager).setPrimaryClip(ClipData.newPlainText("SyncroUmbral", REPO))
            toast("Enlace copiado")
        })
        c.addView(download)

        val modes = card()
        modes.addView(text("Modo básico y avanzado", 17f, TEXT, true))
        modes.addView(text(
            "Básico: bloqueo por presencia, archivos, notificaciones, llamadas, compartir, portapapeles, mensajería web y lista de apps. " +
                "No hace falta activar nada especial.\n\n" +
                "Avanzado: además abre las apps del móvil en ventanas del PC y permite detenerlas, borrar sus datos, desinstalarlas y " +
                "extraer su APK. Necesita la Depuración inalámbrica de Android, que da al PC emparejado control total del móvil " +
                "(y algunas apps de bancos no se abren con ella activa): actívala solo cuando la necesites.\n\n" +
                "El modo se cambia aquí o en el PC y los dos quedan iguales.",
            13f, MUTED
        ).apply { setPadding(0, dp(4), 0, 0) })
        c.addView(modes)

        val version = try { packageManager.getPackageInfo(packageName, 0).versionName } catch (_: Exception) { "?" }
        c.addView(text("SyncroUmbral $version · SyncroApp", 12f, MUTED).apply { setPadding(dp(6), dp(4), 0, 0) })
        return c
    }

    // ---------------------------------------------------------- Acciones rápidas

    private fun pickFiles() {
        if (Link.ws == null) { toast("Sin conexión con el PC"); return }
        val i = Intent(Intent.ACTION_GET_CONTENT).apply {
            type = "*/*"
            addCategory(Intent.CATEGORY_OPENABLE)
            putExtra(Intent.EXTRA_ALLOW_MULTIPLE, true)
        }
        startActivityForResult(Intent.createChooser(i, "Elige qué enviar al PC"), REQ_PICK)
    }

    @Deprecated("Deprecated in Java")
    override fun onActivityResult(requestCode: Int, resultCode: Int, data: Intent?) {
        super.onActivityResult(requestCode, resultCode, data)
        if (requestCode != REQ_PICK || resultCode != RESULT_OK || data == null) return
        val uris = ArrayList<Uri>()
        data.data?.let { uris += it }
        data.clipData?.let { cd -> for (n in 0 until cd.itemCount) uris += cd.getItemAt(n).uri }
        if (uris.isEmpty()) return
        // Se reutiliza el flujo de "Compartir" (confirmación incluida), que mantiene el permiso de lectura mientras envía.
        val send = Intent(this, ShareActivity::class.java).apply {
            action = if (uris.size == 1) Intent.ACTION_SEND else Intent.ACTION_SEND_MULTIPLE
            type = "*/*"
            if (uris.size == 1) putExtra(Intent.EXTRA_STREAM, uris[0]) else putParcelableArrayListExtra(Intent.EXTRA_STREAM, uris)
            addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION)
            clipData = ClipData.newRawUri("", uris[0]).also { cd -> for (u in uris.drop(1)) cd.addItem(ClipData.Item(u)) }
        }
        startActivity(send)
    }

    private fun sendClipboard() {
        if (Link.ws == null) { toast("Sin conexión con el PC"); return }
        val cm = getSystemService(CLIPBOARD_SERVICE) as ClipboardManager
        val t = cm.primaryClip?.takeIf { it.itemCount > 0 }?.getItemAt(0)?.coerceToText(this)?.toString()
        if (t.isNullOrBlank()) { toast("El portapapeles está vacío"); return }
        startActivity(Intent(this, ShareActivity::class.java).apply {
            action = Intent.ACTION_SEND
            type = "text/plain"
            putExtra(Intent.EXTRA_TEXT, t)
        })
    }

    private fun lockPc() {
        if (Link.ws == null) { toast("Sin conexión con el PC"); return }
        AlertDialog.Builder(this)
            .setTitle("Bloquear SyncroUmbral")
            .setMessage("La aplicación SyncroUmbral del PC se bloquea y solo abre con la clave de respaldo. No bloquea Windows.")
            .setPositiveButton("Bloquear") { _, _ ->
                toast(if (Link.send(JSONObject().put("t", "lock_pc"))) "Bloqueo enviado al PC" else "Sin conexión con el PC")
            }
            .setNegativeButton("Cancelar", null)
            .show()
    }

    private fun unlockPc() {
        toast(if (Link.send(JSONObject().put("t", "unlock_pc"))) "Desbloqueo enviado al PC" else "Sin conexión con el PC")
    }

    // Al girar el móvil se conserva la sección abierta.
    override fun onSaveInstanceState(outState: Bundle) {
        super.onSaveInstanceState(outState)
        outState.putInt("page", page)
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
        // Una intención de prueba se atiende una sola vez: al recrearse la pantalla no debe repetirse (el código de vinculación es de un solo uso).
        if (debuggable && intent?.action?.startsWith("app.syncroapp.umbral.TEST_") == true) setIntent(Intent())
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
            connected -> "SyncroUmbral en el PC se mantiene abierta mientras el móvil esté en la red."
            else -> "${Status.text}${if (store.paired) "" else "  ·  pendiente de confirmar"}"
        }
        pairCard.visibility = if (store.hasPc) View.GONE else View.VISIBLE
        quickCard.visibility = if (store.hasPc && connected) View.VISIBLE else View.GONE
        unpairButton.visibility = if (store.hasPc) View.VISIBLE else View.GONE
        linkedText.text = if (store.hasPc) "${store.pcName}  ·  huella ${store.fingerprint.take(12).chunked(4).joinToString("-")}" else "Ninguno."

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
        val missing = rows.size - done
        permSummary.text = if (missing == 0) "Todos los permisos concedidos." else "$done de ${rows.size} concedidos. Pulsa \"Permitir\" en los que faltan."
        permSummary.setTextColor(if (missing == 0) OK else MUTED)
        navButtons[1].text = if (missing == 0) "Permisos" else "Permisos ($missing)"
        permAlert.visibility = if (missing == 0) View.GONE else View.VISIBLE
        permAlertText.text = if (missing == 1) "Falta 1 permiso." else "Faltan $missing permisos."

        if (keepAliveSwitch.isChecked != store.keepAlive) keepAliveSwitch.isChecked = store.keepAlive
        styleSeg(modeBasic, !store.advanced)
        styleSeg(modeAdvanced, store.advanced)
        modeText.text = if (store.advanced)
            "Avanzado: además abre apps del móvil en el PC y permite detenerlas o desinstalarlas (necesita depuración inalámbrica)."
        else "Básico: todo funciona sin depuración. Las acciones con adb en el PC están apagadas."

        val battery = batteryWarning()
        batteryAlert.visibility = if (battery.isEmpty()) View.GONE else View.VISIBLE
        batteryAlert.text = battery
        val a11y = accessibilityPackages()
        val show = a11y.isNotEmpty() && a11y.joinToString(",") != store.dismissedA11y
        a11yAlert.visibility = if (show) View.VISIBLE else View.GONE
        a11yText.text = "Accesibilidad activa (${a11y.joinToString(", ")}): puede pulsar botones por ti, incluidas las confirmaciones de SyncroUmbral."
    }

    /** El ahorro de batería es la causa típica de que la conexión se corte con la pantalla apagada. */
    private fun batteryWarning(): String {
        val pm = getSystemService(POWER_SERVICE) as PowerManager
        if (!pm.isPowerSaveMode) return ""
        return "El ahorro de batería está activo: Android puede cortar la conexión con el PC al apagarse la pantalla."
    }

    /**
     * Los servicios de accesibilidad pueden leer la pantalla y pulsar botones por el usuario, incluidos los de
     * confirmación de SyncroUmbral. No se bloquea nada (muchos son legítimos); se avisa una vez y se puede descartar.
     */
    private fun accessibilityPackages(): List<String> {
        val raw = Settings.Secure.getString(contentResolver, Settings.Secure.ENABLED_ACCESSIBILITY_SERVICES).orEmpty()
        return raw.split(':').mapNotNull { it.substringBefore('/').takeIf { p -> p.isNotBlank() } }.distinct()
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
                show(0)
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
