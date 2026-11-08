package app.syncroapp.umbral

import android.content.Context
import android.content.pm.ApplicationInfo
import android.content.pm.PackageInfo
import android.content.pm.PackageManager
import android.graphics.Bitmap
import android.graphics.Canvas
import android.os.Build
import android.util.Base64
import org.json.JSONArray
import org.json.JSONObject
import java.io.ByteArrayOutputStream
import java.io.File

/** Información de las apps instaladas, para mostrarla en el PC. Solo lee; las acciones las hace el PC por adb. */
object AppOps {
    private const val ICON_PX = 48
    private const val MAX_ICONS = 40

    fun handle(ctx: Context, op: String, a: JSONObject): JSONObject = when (op) {
        "apps" -> list(ctx)
        "icons" -> icons(ctx, a.getJSONArray("pkgs"))
        else -> throw IllegalArgumentException("Operación desconocida: $op")
    }

    @Suppress("DEPRECATION")
    private fun list(ctx: Context): JSONObject {
        val pm = ctx.packageManager
        val arr = JSONArray()
        val sorted = pm.getInstalledPackages(0).sortedBy { label(pm, it).lowercase() }
        for (pi in sorted) {
            val ai = pi.applicationInfo ?: continue
            val system = (ai.flags and ApplicationInfo.FLAG_SYSTEM) != 0
            arr.put(
                JSONObject()
                    .put("pkg", pi.packageName)
                    .put("label", label(pm, pi))
                    .put("version", pi.versionName ?: "")
                    .put("code", versionCode(pi))
                    .put("system", system)
                    .put("enabled", ai.enabled)
                    .put("launchable", pm.getLaunchIntentForPackage(pi.packageName) != null)
                    .put("installed", pi.firstInstallTime)
                    .put("updated", pi.lastUpdateTime)
                    .put("size", apkSize(ai))
            )
        }
        return JSONObject().put("apps", arr)
    }

    private fun label(pm: PackageManager, pi: PackageInfo): String =
        pi.applicationInfo?.let { pm.getApplicationLabel(it).toString() } ?: pi.packageName

    @Suppress("DEPRECATION")
    private fun versionCode(pi: PackageInfo): Long =
        if (Build.VERSION.SDK_INT >= 28) pi.longVersionCode else pi.versionCode.toLong()

    private fun apkSize(ai: ApplicationInfo): Long {
        var n = File(ai.sourceDir).length()
        ai.splitSourceDirs?.forEach { n += File(it).length() }
        return n
    }

    private fun icons(ctx: Context, pkgs: JSONArray): JSONObject {
        val pm = ctx.packageManager
        val out = JSONObject()
        // Un único bitmap pequeño reutilizado para todo el lote: no se acumulan imágenes en memoria.
        val bmp = Bitmap.createBitmap(ICON_PX, ICON_PX, Bitmap.Config.ARGB_8888)
        val canvas = Canvas(bmp)
        val bos = ByteArrayOutputStream()
        try {
            for (i in 0 until minOf(pkgs.length(), MAX_ICONS)) {
                val pkg = pkgs.getString(i)
                try {
                    val d = pm.getApplicationIcon(pkg)
                    bmp.eraseColor(0)
                    d.setBounds(0, 0, ICON_PX, ICON_PX)
                    d.draw(canvas)
                    bos.reset()
                    bmp.compress(Bitmap.CompressFormat.PNG, 100, bos)
                    out.put(pkg, Base64.encodeToString(bos.toByteArray(), Base64.NO_WRAP))
                } catch (_: Exception) {
                    // sin icono para esta app
                }
            }
        } finally {
            bmp.recycle()
        }
        return JSONObject().put("icons", out)
    }
}
