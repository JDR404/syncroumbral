package app.syncroapp.umbral

/**
 * Tope de acciones con coste o con efecto fuera del móvil que el PC puede pedir (llamar, enviar SMS). Si alguien controlara
 * el PC no podría hacer decenas de llamadas o SMS seguidos (por ejemplo a números de pago).
 */
object RateLimit {
    private val hits = HashMap<String, ArrayDeque<Long>>()

    @Synchronized
    fun check(key: String, max: Int, windowMs: Long, what: String) {
        val now = System.currentTimeMillis()
        val q = hits.getOrPut(key) { ArrayDeque() }
        while (q.isNotEmpty() && now - q.first() > windowMs) q.removeFirst()
        if (q.size >= max) throw IllegalStateException("Demasiadas $what en poco tiempo (máximo $max cada ${windowMs / 60_000} min). Inténtalo más tarde")
        q.addLast(now)
    }
}
