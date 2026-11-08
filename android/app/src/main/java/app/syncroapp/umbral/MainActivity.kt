package app.syncroapp.umbral

import android.Manifest
import android.app.Activity
import android.app.AlertDialog
import android.content.Intent
import android.content.pm.PackageManager
import android.net.Uri
import android.os.Build
import android.os.Bundle
import android.os.Handler
import android.os.Looper
import android.provider.Settings
import android.text.InputType
import android.view.Gravity
import android.view.ViewGroup
import android.widget.Button
import android.widget.EditText
import android.widget.LinearLayout
import android.widget.TextView
import android.widget.Toast
import com.google.mlkit.vision.barcode.common.Barcode
import com.google.mlkit.vision.codescanner.GmsBarcodeScannerOptions
import com.google.mlkit.vision.codescanner.GmsBarcodeScanning

class MainActivity : Activity() {
    private lateinit var store: Store
    private lateinit var statusView: TextView
    private lateinit var pcView: TextView
    private lateinit var keepAliveButton: Button
    private val handler = Handler(Looper.getMainLooper())
    private val refresh = object : Runnable {
        override fun run() {
            render()
            handler.postDelayed(this, 1000)
        }
    }

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        store = Store(this)
        KeyManager.ensureKey()

        fun dp(v: Int) = (v * resources.displayMetrics.density).toInt()
        fun button(label: String, onClick: () -> Unit) = Button(this).apply {
            text = label
            setOnClickListener { onClick() }
        }

        val root = LinearLayout(this).apply {
            orientation = LinearLayout.VERTICAL
            setPadding(dp(24), dp(48), dp(24), dp(24))
            gravity = Gravity.TOP
        }
        root.addView(TextView(this).apply { text = "SyncroUmbral"; textSize = 32f })
        statusView = TextView(this).apply { textSize = 18f; setPadding(0, dp(16), 0, 0) }
        pcView = TextView(this).apply { textSize = 14f; setPadding(0, dp(4), 0, dp(24)) }
        root.addView(statusView)
        root.addView(pcView)

        root.addView(button("Escanear QR del PC") { scan() })

        val manual = EditText(this).apply {
            hint = "…o pega aquí el código de vinculación"
            inputType = InputType.TYPE_CLASS_TEXT or InputType.TYPE_TEXT_FLAG_MULTI_LINE
            minLines = 2
        }
        root.addView(manual, LinearLayout.LayoutParams(ViewGroup.LayoutParams.MATCH_PARENT, ViewGroup.LayoutParams.WRAP_CONTENT))
        root.addView(button("Vincular con el código") { pair(manual.text.toString().trim()) })

        root.addView(button("Permitir acceso a los archivos") { askFileAccess() })
        root.addView(button("Permitir acceso a las notificaciones") { askNotifAccess() })
        root.addView(button("Permitir contestar llamadas") { askAnswerCalls() })
        if (CallOps.overlayAvailable(this)) {
            root.addView(button("Permitir mostrar sobre otras apps (WhatsApp)") { askOverlay() })
        }
        root.addView(button("Enviar notificación de prueba") { TestNotification.post(this) })
        root.addView(button("Permitir uso en segundo plano (batería)") { ignoreBattery() })
        keepAliveButton = button("") {
            store.keepAlive = !store.keepAlive
            if (!store.keepAlive) KeepAlive.release()
            render()
        }
        root.addView(keepAliveButton)
        root.addView(button("Desvincular") { unpair() })
        setContentView(root)

        if (Build.VERSION.SDK_INT >= 33 &&
            checkSelfPermission(Manifest.permission.POST_NOTIFICATIONS) != PackageManager.PERMISSION_GRANTED
        ) {
            requestPermissions(arrayOf(Manifest.permission.POST_NOTIFICATIONS), 1)
        }
        if (store.hasPc) LinkService.start(this)
        handleIntent(intent)
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
        statusView.text = if (store.hasPc) Status.text else "Sin vincular"
        val files = if (FileOps.hasAccess()) "archivos: sí" else "archivos: no"
        val notifs = if (NotifService.instance != null) "notificaciones: sí" else "notificaciones: no"
        keepAliveButton.text = "Conexión con pantalla apagada: ${if (store.keepAlive) "mantenida" else "no mantenida"} (tocar para cambiar)"
        val calls = "contestar llamadas: ${if (CallOps.canAnswerPhone(this)) "sí" else "no"}" +
            if (CallOps.overlayAvailable(this)) "  ·  sobre otras apps: ${if (CallOps.canOverlay(this)) "sí" else "no"}" else ""
        pcView.text = (if (store.hasPc) {
            "PC: ${store.pcName}  ·  ${if (store.paired) "vinculado" else "pendiente de confirmar"}\n$files  ·  $notifs\n$calls"
        } else "") + batteryWarning() + accessibilityWarning()
    }

    /**
     * Los servicios de accesibilidad pueden leer la pantalla y pulsar botones por el usuario, incluidos los de
     * confirmación de SyncroUmbral. No se bloquea nada (muchos son legítimos), pero se avisa para que el usuario lo sepa.
     */
    /** El ahorro de batería y la optimización de SyncroUmbral son la causa típica de que la conexión se corte con la pantalla apagada. */
    private fun batteryWarning(): String {
        val pm = getSystemService(POWER_SERVICE) as android.os.PowerManager
        val parts = ArrayList<String>()
        if (pm.isPowerSaveMode) parts.add("el ahorro de batería está activo")
        if (!pm.isIgnoringBatteryOptimizations(packageName)) parts.add("SyncroUmbral está optimizado por la batería")
        if (parts.isEmpty()) return ""
        return "\n\nAviso: ${parts.joinToString(" y ")}. Android puede cortar la conexión con el PC al apagarse la pantalla: " +
            "pulsa \"Permitir uso en segundo plano\" y desactiva el ahorro de batería mientras uses SyncroUmbral."
    }

    private fun accessibilityWarning(): String {
        val raw = Settings.Secure.getString(contentResolver, Settings.Secure.ENABLED_ACCESSIBILITY_SERVICES).orEmpty()
        val pkgs = raw.split(':').mapNotNull { it.substringBefore('/').takeIf { p -> p.isNotBlank() } }.distinct()
        if (pkgs.isEmpty()) return ""
        return "\n\nAviso: hay servicios de accesibilidad activos (${pkgs.joinToString(", ")}). Pueden pulsar botones por ti, " +
            "incluidas las confirmaciones de SyncroUmbral; algunas apps de banco también se niegan a abrirse con ellos."
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
        if (CallOps.canAnswerPhone(this)) toast("Ya está permitido")
        else requestPermissions(arrayOf(Manifest.permission.ANSWER_PHONE_CALLS), 3)
    }

    /** Sin este permiso Android no deja contestar una llamada de WhatsApp (u otra app) cuando SyncroUmbral está en segundo plano. */
    private fun askOverlay() {
        if (CallOps.canOverlay(this)) toast("Ya está permitido")
        else startActivity(Intent(Settings.ACTION_MANAGE_OVERLAY_PERMISSION, Uri.parse("package:$packageName")))
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
