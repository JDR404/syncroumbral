package app.syncroapp.umbral

import android.app.Activity
import android.app.Dialog
import android.graphics.Color
import android.graphics.Typeface
import android.graphics.drawable.ColorDrawable
import android.graphics.drawable.GradientDrawable
import android.view.Gravity
import android.view.ViewGroup
import android.view.Window
import android.widget.Button
import android.widget.ImageView
import android.widget.LinearLayout
import android.widget.ScrollView
import android.widget.TextView

/** Hoja inferior de confirmación para «Compartir con SyncroUmbral», con la paleta y las formas de la app. */
object ShareSheet {
    private const val SURFACE = 0xFF171D1E.toInt()
    private const val SURFACE2 = 0xFF1F2A2C.toInt()
    private const val PRIMARY = 0xFF4FD8EB.toInt()
    private const val ON_PRIMARY = 0xFF00363D.toInt()
    private const val TEXT = 0xFFDEE3E5.toInt()
    private const val MUTED = 0xFFA3ADAF.toInt()

    fun show(activity: Activity, title: String, summary: String, onSend: () -> Unit, onCancel: () -> Unit) {
        fun dp(v: Int) = (v * activity.resources.displayMetrics.density).toInt()
        fun shape(color: Int, radius: Int) = GradientDrawable().apply { setColor(color); cornerRadius = dp(radius).toFloat() }
        fun label(s: String, sp: Float, color: Int, bold: Boolean = false) = TextView(activity).apply {
            text = s
            textSize = sp
            setTextColor(color)
            if (bold) setTypeface(typeface, Typeface.BOLD)
        }

        val dialog = Dialog(activity)
        dialog.requestWindowFeature(Window.FEATURE_NO_TITLE)

        val root = LinearLayout(activity).apply {
            orientation = LinearLayout.VERTICAL
            background = shape(SURFACE, 28)
            setPadding(dp(22), dp(20), dp(22), dp(18))
        }
        val head = LinearLayout(activity).apply { orientation = LinearLayout.HORIZONTAL; gravity = Gravity.CENTER_VERTICAL }
        head.addView(ImageView(activity).apply {
            setImageResource(R.drawable.ic_logo)
            background = shape(SURFACE2, 12)
            setPadding(dp(6), dp(6), dp(6), dp(6))
            contentDescription = "SyncroUmbral"
        }, LinearLayout.LayoutParams(dp(40), dp(40)).apply { marginEnd = dp(12) })
        head.addView(label(title, 18f, TEXT, true), LinearLayout.LayoutParams(0, ViewGroup.LayoutParams.WRAP_CONTENT, 1f))
        root.addView(head)

        // La primera línea del resumen es la cabecera («1 archivo(s), 0,4 MB»); el resto, los nombres o el texto.
        val first = summary.substringBefore("\n\n")
        val rest = summary.substringAfter("\n\n", "")
        root.addView(label(first, 13f, MUTED).apply { setPadding(0, dp(14), 0, dp(8)) })
        if (rest.isNotEmpty()) {
            val box = ScrollView(activity).apply {
                background = shape(SURFACE2, 18)
                addView(label(rest, 14f, TEXT).apply { setPadding(dp(14), dp(12), dp(14), dp(12)) })
            }
            root.addView(box, LinearLayout.LayoutParams(ViewGroup.LayoutParams.MATCH_PARENT, ViewGroup.LayoutParams.WRAP_CONTENT))
        }
        root.addView(label("Llega a Descargas, carpeta SyncroUmbral, en el PC.", 12f, MUTED).apply { setPadding(0, dp(10), 0, dp(14)) })

        var sending = false
        fun button(s: String, filled: Boolean, click: () -> Unit) = Button(activity).apply {
            text = s
            isAllCaps = false
            textSize = 15f
            stateListAnimator = null
            setTextColor(if (filled) ON_PRIMARY else TEXT)
            background = if (filled) shape(PRIMARY, 24) else GradientDrawable().apply {
                setColor(Color.TRANSPARENT)
                cornerRadius = dp(24).toFloat()
                setStroke(dp(1), 0xFF3F4849.toInt())
            }
            minHeight = dp(48)
            minimumHeight = dp(48)
            setOnClickListener { click() }
        }
        val row = LinearLayout(activity).apply { orientation = LinearLayout.HORIZONTAL }
        row.addView(button("Cancelar", false) { dialog.cancel() },
            LinearLayout.LayoutParams(0, ViewGroup.LayoutParams.WRAP_CONTENT, 1f).apply { marginEnd = dp(10) })
        row.addView(button("Enviar", true) { sending = true; dialog.dismiss(); onSend() },
            LinearLayout.LayoutParams(0, ViewGroup.LayoutParams.WRAP_CONTENT, 1f))
        root.addView(row)

        dialog.setContentView(root)
        dialog.setOnCancelListener { if (!sending) onCancel() }
        dialog.window?.apply {
            setBackgroundDrawable(ColorDrawable(Color.TRANSPARENT))
            setGravity(Gravity.BOTTOM)
            setDimAmount(0.6f)
            addFlags(android.view.WindowManager.LayoutParams.FLAG_DIM_BEHIND)
        }
        dialog.show()
        // Ancho completo con un margen alrededor (se fija después de mostrarla).
        dialog.window?.setLayout(activity.resources.displayMetrics.widthPixels - dp(20), ViewGroup.LayoutParams.WRAP_CONTENT)
        dialog.window?.attributes = dialog.window?.attributes?.apply { y = dp(24) }
    }
}
