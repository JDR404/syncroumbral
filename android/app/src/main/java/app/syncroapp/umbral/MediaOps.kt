package app.syncroapp.umbral

import android.content.Context
import android.graphics.Bitmap
import android.graphics.BitmapFactory
import android.media.ThumbnailUtils
import android.os.Build
import android.os.Environment
import android.provider.MediaStore
import android.provider.MediaStore.Files.FileColumns as FC
import android.util.Base64
import android.util.Size
import org.json.JSONArray
import org.json.JSONObject
import java.io.ByteArrayOutputStream
import java.io.File

/**
 * Galería: fotos y vídeos de todo el almacenamiento, por álbum (carpeta), con miniaturas, para verlos en el PC.
 * Primero se consulta MediaStore (todas las carpetas, también WhatsApp, capturas o descargas); si no devolviera nada se
 * recorren DCIM, Pictures y Movies. Usa el permiso de archivos que ya tiene la app.
 */
object MediaOps {
    private val IMAGES = setOf("jpg", "jpeg", "png", "webp", "gif", "heic", "heif", "bmp")
    private val VIDEOS = setOf("mp4", "mov", "mkv", "3gp", "webm")
    private const val MAX_PAGE = 60
    private const val MAX_THUMBS = 12
    private const val MAX_ALBUMS = 60
    private const val THUMB_PX = 240
    private const val CACHE_MS = 30_000L

    private class Entry(val file: File, val mtime: Long, val size: Long, val video: Boolean)

    private var cache: List<Entry> = emptyList()
    private var cachedAt = 0L

    fun handle(ctx: Context, op: String, a: JSONObject): JSONObject {
        if (!FileOps.hasAccess()) throw IllegalStateException("Falta el permiso de acceso a todos los archivos en el móvil")
        return when (op) {
            "media_albums" -> albums(ctx, a.optBoolean("refresh"))
            "media_recent" -> recent(ctx, a.optString("album"), a.optInt("offset", 0), a.optInt("limit", 40).coerceIn(1, MAX_PAGE), a.optBoolean("refresh"))
            "media_thumb" -> thumbs(a.getJSONArray("paths"))
            else -> throw IllegalArgumentException("Operación desconocida: $op")
        }
    }

    private fun fromMediaStore(ctx: Context): List<Entry> {
        val root = Environment.getExternalStorageDirectory().canonicalPath
        val out = ArrayList<Entry>()
        try {
            ctx.contentResolver.query(
                MediaStore.Files.getContentUri("external"),
                arrayOf(FC.DATA, FC.DATE_MODIFIED, FC.SIZE, FC.MEDIA_TYPE),
                "${FC.MEDIA_TYPE} IN (${FC.MEDIA_TYPE_IMAGE}, ${FC.MEDIA_TYPE_VIDEO})", null, "${FC.DATE_MODIFIED} DESC"
            )?.use { c ->
                while (c.moveToNext() && out.size < 30_000) {
                    val p = c.getString(0) ?: continue
                    if (!p.startsWith("$root/") || p.contains("/.") || p.startsWith("$root/Android/")) continue
                    out += Entry(File(p), c.getLong(1) * 1000, c.getLong(2), c.getInt(3) == FC.MEDIA_TYPE_VIDEO)
                }
            }
        } catch (_: Exception) { /* sin acceso a MediaStore: se usa el recorrido de carpetas */ }
        return out
    }

    private fun scan(): List<Entry> {
        val root = Environment.getExternalStorageDirectory()
        val out = ArrayList<Entry>()
        fun walk(d: File, depth: Int) {
            if (out.size >= 20_000) return
            for (f in d.listFiles() ?: return) {
                if (f.name.startsWith(".")) continue
                if (f.isDirectory) { if (depth < 3) walk(f, depth + 1) }
                else {
                    val ext = f.extension.lowercase()
                    if (ext in IMAGES || ext in VIDEOS) out += Entry(f, f.lastModified(), f.length(), ext in VIDEOS)
                }
            }
        }
        for (d in listOf("DCIM", "Pictures", "Movies")) walk(File(root, d), 0)
        return out.sortedByDescending { it.mtime }
    }

    @Synchronized
    private fun load(ctx: Context, refresh: Boolean): List<Entry> {
        val now = System.currentTimeMillis()
        if (refresh || now - cachedAt > CACHE_MS) {
            cache = fromMediaStore(ctx).ifEmpty { scan() }
            cachedAt = now
        }
        return cache
    }

    private fun albumOf(e: Entry) = FileOps.relative(e.file.parentFile?.canonicalFile ?: e.file)

    private fun albums(ctx: Context, refresh: Boolean): JSONObject {
        val groups = LinkedHashMap<String, MutableList<Entry>>()
        for (e in load(ctx, refresh)) groups.getOrPut(albumOf(e)) { mutableListOf() } += e
        val arr = JSONArray()
        for ((dir, list) in groups.entries.sortedByDescending { it.value.first().mtime }.take(MAX_ALBUMS)) {
            arr.put(JSONObject().put("path", dir).put("name", dir.substringAfterLast('/').ifEmpty { "Almacenamiento" }).put("count", list.size))
        }
        return JSONObject().put("items", arr).put("total", cache.size)
    }

    private fun recent(ctx: Context, album: String, offset: Int, limit: Int, refresh: Boolean): JSONObject {
        val all = load(ctx, refresh)
        val list = if (album.isEmpty()) all else all.filter { albumOf(it) == album }
        val arr = JSONArray()
        for (e in list.drop(offset.coerceAtLeast(0)).take(limit)) {
            arr.put(
                JSONObject().put("path", FileOps.relative(e.file.canonicalFile)).put("name", e.file.name)
                    .put("mtime", e.mtime).put("size", e.size).put("video", e.video)
            )
        }
        return JSONObject().put("items", arr).put("total", list.size)
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
