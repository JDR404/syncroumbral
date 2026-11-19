package app.syncroapp.umbral

import android.content.Context
import android.media.MediaScannerConnection
import android.os.Build
import android.os.Environment
import android.util.Base64
import org.json.JSONArray
import org.json.JSONObject
import java.io.File
import java.io.RandomAccessFile

/**
 * Operaciones de archivos pedidas por el PC. Las rutas son relativas al almacenamiento
 * interno compartido ("" = raíz) y nunca pueden salir de él.
 */
object FileOps {
    private const val PART = ".umbral-part"
    private const val MAX_CHUNK = 2 * 1024 * 1024

    private val root: File get() = Environment.getExternalStorageDirectory().canonicalFile

    fun hasAccess(): Boolean =
        if (Build.VERSION.SDK_INT >= 30) Environment.isExternalStorageManager()
        else root.canRead()

    internal fun resolve(path: String): File {
        val rel = path.trim().trimStart('/', '\\')
        val f = (if (rel.isEmpty()) root else File(root, rel)).canonicalFile
        require(f.path == root.path || f.path.startsWith(root.path + File.separator)) {
            "Ruta fuera del almacenamiento"
        }
        return f
    }

    internal fun relative(f: File): String =
        if (f.path == root.path) "" else f.path.removePrefix(root.path + File.separator).replace('\\', '/')

    /** Ejecuta la operación `op`. Lanza excepción con un mensaje legible si falla. */
    fun handle(ctx: Context, op: String, a: JSONObject): JSONObject {
        if (!hasAccess()) {
            throw IllegalStateException("Falta el permiso de acceso a todos los archivos en el móvil")
        }
        return when (op) {
            "ls" -> ls(a.optString("path"))
            "read" -> read(a.getString("path"), a.getLong("offset"), a.getInt("len"))
            "write" -> write(ctx, a.getString("path"), a.getLong("offset"), a.getString("data"), a.optBoolean("final"))
            "abort" -> abort(a.getString("path"))
            "mkdir" -> mkdir(a.getString("path"))
            "delete" -> delete(ctx, a.getString("path"))
            "rename" -> rename(ctx, a.getString("path"), a.getString("name"))
            else -> throw IllegalArgumentException("Operación desconocida: $op")
        }
    }

    private fun ls(path: String): JSONObject {
        val dir = resolve(path)
        if (!dir.isDirectory) throw IllegalArgumentException("No es una carpeta")
        val items = dir.listFiles() ?: throw IllegalStateException("No se puede leer la carpeta")
        val sorted = items
            .filter { !it.name.endsWith(PART) }
            .sortedWith(compareBy<File>({ !it.isDirectory }, { it.name.lowercase() }))
        val arr = JSONArray()
        for (f in sorted) {
            arr.put(
                JSONObject().put("name", f.name).put("dir", f.isDirectory)
                    .put("size", if (f.isDirectory) 0 else f.length()).put("mtime", f.lastModified())
            )
        }
        return JSONObject().put("path", relative(dir)).put("items", arr)
            .put("free", root.freeSpace).put("total", root.totalSpace)
    }

    private fun read(path: String, offset: Long, len: Int): JSONObject {
        val f = resolve(path)
        if (!f.isFile) throw IllegalArgumentException("No es un archivo")
        RandomAccessFile(f, "r").use { raf ->
            val size = raf.length()
            raf.seek(offset)
            val buf = ByteArray(len.coerceIn(0, MAX_CHUNK))
            var n = 0
            while (n < buf.size) {
                val r = raf.read(buf, n, buf.size - n)
                if (r < 0) break
                n += r
            }
            return JSONObject().put("n", n).put("size", size).put("eof", offset + n >= size)
                .put("data", Base64.encodeToString(buf, 0, n, Base64.NO_WRAP))
        }
    }

    /** Escribe en un temporal y lo renombra al terminar: nunca queda un archivo a medias con el nombre final. */
    /**
     * Avisa a Android de los archivos nuevos, borrados o renombrados. Sin esto el administrador de archivos y la galería
     * del móvil no los muestran (o siguen mostrando los borrados) porque consultan el índice de medios, no el disco.
     */
    private fun scan(ctx: Context, vararg files: File) {
        val paths = files.map { it.path }.distinct().take(500).toTypedArray()
        if (paths.isNotEmpty()) MediaScannerConnection.scanFile(ctx.applicationContext, paths, null, null)
    }

    private fun write(ctx: Context, path: String, offset: Long, data: String, final: Boolean): JSONObject {
        val target = resolve(path)
        val part = File(target.path + PART)
        part.parentFile?.mkdirs()
        val bytes = Base64.decode(data, Base64.NO_WRAP)
        RandomAccessFile(part, "rw").use { raf ->
            if (offset == 0L) raf.setLength(0)
            raf.seek(offset)
            raf.write(bytes)
        }
        if (final) {
            if (target.exists()) target.delete()
            if (!part.renameTo(target)) throw IllegalStateException("No se pudo guardar el archivo")
            scan(ctx, target)
            return JSONObject().put("n", bytes.size).put("size", target.length())
        }
        return JSONObject().put("n", bytes.size)
    }

    /** Borra solo el temporal de una subida interrumpida; nunca el archivo final. */
    private fun abort(path: String): JSONObject {
        File(resolve(path).path + PART).delete()
        return JSONObject()
    }

    private fun mkdir(path: String): JSONObject {
        val d = resolve(path)
        if (!d.exists() && !d.mkdirs()) throw IllegalStateException("No se pudo crear la carpeta")
        return JSONObject()
    }

    private fun delete(ctx: Context, path: String): JSONObject {
        val f = resolve(path)
        require(f.path != root.path) { "No se puede borrar la raíz" }
        val removed = f.walkTopDown().filter { it.isFile }.take(500).toList() // se anotan antes de borrar
        if (!f.deleteRecursively()) throw IllegalStateException("No se pudo borrar")
        scan(ctx, f, *removed.toTypedArray())
        return JSONObject()
    }

    private fun rename(ctx: Context, path: String, name: String): JSONObject {
        require(name.isNotBlank() && !name.contains('/') && !name.contains('\\')) { "Nombre no válido" }
        val f = resolve(path)
        require(f.path != root.path) { "No se puede renombrar la raíz" }
        val dest = File(f.parentFile, name)
        if (dest.exists()) throw IllegalStateException("Ya existe un elemento con ese nombre")
        val moved = if (f.isDirectory) f.walkTopDown().filter { it.isFile }.take(500).toList() else listOf(f)
        if (!f.renameTo(dest)) throw IllegalStateException("No se pudo renombrar")
        // Se indexa el nuevo nombre y se retiran del índice las rutas antiguas.
        val newOnes = moved.map { File(dest, it.relativeTo(f).path).takeIf { _ -> f.isDirectory } ?: dest }
        scan(ctx, *moved.toTypedArray(), *newOnes.toTypedArray())
        return JSONObject()
    }
}
