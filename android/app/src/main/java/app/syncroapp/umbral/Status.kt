package app.syncroapp.umbral

/** Estado visible de la conexión (compartido entre servicio y pantalla). */
object Status {
    @Volatile var text: String = "Sin vincular"
        private set
    @Volatile var connected: Boolean = false
        private set
    @Volatile var onChange: (() -> Unit)? = null

    fun set(text: String, connected: Boolean = false) {
        this.text = text
        this.connected = connected
        onChange?.invoke()
    }
}
