package app.syncroapp.umbral

import android.content.BroadcastReceiver
import android.content.Context
import android.content.Intent

class BootReceiver : BroadcastReceiver() {
    override fun onReceive(context: Context, intent: Intent) {
        val boot = intent.action == Intent.ACTION_BOOT_COMPLETED || intent.action == Intent.ACTION_MY_PACKAGE_REPLACED
        if (boot && Store(context).hasPc) {
            try {
                LinkService.start(context)
            } catch (_: Exception) {
                // Android puede negar el arranque en segundo plano; el usuario abre la app.
            }
        }
    }
}
