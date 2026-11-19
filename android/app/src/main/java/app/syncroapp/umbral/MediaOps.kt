package app.syncroapp.umbral

import android.graphics.Bitmap
import android.graphics.BitmapFactory
import android.media.ThumbnailUtils
import android.os.Build
import android.os.Environment
import android.util.Base64
import android.util.Size
import org.json.JSONArray
import org.json.JSONObject
import java.io.ByteArrayOutputStream
import java.io.File

/** Galería: fotos y vídeos recientes con miniaturas, para verlos en el PC. Usa el permiso de archivos que ya tiene la app. */
object MediaOps {
    private val IMAGES = setOf("jpg", "jpeg", "png", "webp", "gif", "heic", "heif", "bmp")
    private val VIDEOS = setOf("mp4", "mov", "mkv", "3gp", "webm")
    private const val MAX_PAGE = 60
    private const val MAX_THUMBS = 12
    private const val THUMB_PX = 240
    private const val CACHE_MS = 30_000L

    private var cache: List<File> = emptyList()
    private var cachedAt = 0L

    fun handle(op: String, a: JSONObject): JSONObject {
        if (!FileOps.hasAccess()) throw IllegalStateException("Falta el permiso de acceso a todos los archivos en el móvil")
        return when (op) {
            "media_recent" -> recent(a.optInt("offset", 0), a.optInt("limit", 40).coerceIn(1, MAX_PAGE), a.optBoolean("refresh"))
            "media_thumb" -> thumbs(a.getJSONArray("paths"))
            else -> throw IllegalArgumentException("Operación desconocida: $op")
        }
    }

    private fun scan(): List<File> {
        val root = Environment.getExternalStorageDirectory()
        val out = ArrayList<File>()
        fun walk(d: File, depth: Int) {
            if (out.size >= 20_000) return
            for (f in d.listFiles() ?: return) {
                if (f.name.startsWith(".")) continue // .thumbnails, .trashed-...
                if (f.isDirectory) { if (depth < 3) walk(f, depth + 1) }
                else if (f.extension.lowercase().let { it in IMAGES || it in VIDEOS }) out += f
            }
        }
        for (d in listOf("DCIM", "Pictures", "Movies")) walk(File(root, d), 0)
        return out.sortedByDescending { it.lastModified() }
    }

    @Synchronized
    private fun recent(offset: Int, limit: Int, refresh: Boolean): JSONObject {
        val now = System.currentTimeMillis()
        if (refresh || now - cachedAt > CACHE_MS) { cache = scan(); cachedAt = now }
        val arr = JSONArray()
        for (f in cache.drop(offset.coerceAtLeast(0)).take(limit)) {
            arr.put(
                JSONObject().put("path", FileOps.relative(f.canonicalFile)).put("name", f.name)
                    .put("mtime", f.lastModified()).put("size", f.length()).put("video", f.extension.lowercase() in VIDEOS)
            )
        }
        return JSONObject().put("items", arr).put("total", cache.size)
    }

    private fun thumbs(paths: JSONArray): JSONObject {
        val out = JSONObject()
        for (i in 0 until minOf(paths.length(), MAX_THUMBS)) {
            val p = paths.getString(i)
            try {
                val f = FileOps.resolve(p)
                val bmp = thumb(f) ?: continue
                val bos = ByteArrayOutputStream()
                bmp.compress(Bitmap.CompressFormat.JPEG, 72, bos)
                out.put(p, Base64.encodeToString(bos.toByteArray(), Base64.NO_WRAP))
            } catch (_: Exception) { /* una miniatura que falla no debe romper las demás */ }
        }
        return JSONObject().put("thumbs", out)
    }

    private fun thumb(f: File): Bitmap? {
        val video = f.extension.lowercase() in VIDEOS
        if (Build.VERSION.SDK_INT >= 29) {
            val size = Size(THUMB_PX, THUMB_PX)
            return if (video) ThumbnailUtils.createVideoThumbnail(f, size, null) else ThumbnailUtils.createImageThumbnail(f, size, null)
        }
        if (video) return null
        val bounds = BitmapFactory.Options().apply { inJustDecodeBounds = true }
        BitmapFactory.decodeFile(f.path, bounds)
        var s = 1
        while (bounds.outWidth / (s * 2) >= THUMB_PX && bounds.outHeight / (s * 2) >= THUMB_PX) s *= 2
        return BitmapFactory.decodeFile(f.path, BitmapFactory.Options().apply { inSampleSize = s })
    }
}
