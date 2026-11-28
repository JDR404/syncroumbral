package app.syncroapp.umbral

/**
 * Textos del aviso destacado y consentimiento que Google Play exige antes de pedir cada permiso sensible (política de
 * Datos de Usuario): dentro de la app, antes del permiso, diciendo qué datos se usan, para qué, cuándo y a dónde van, con
 * aceptación expresa y la opción de no aceptar. Si cambian los datos que usa un permiso, hay que cambiar su texto y
 * `docs/PLAY.md` (declaraciones) y la política de privacidad.
 */
object Disclosures {
    const val PRIVACY_URL = "https://www.syncroapp.dev/syncroumbral/privacidad"

    private const val DESTINATION =
        "Los datos viajan cifrados, directamente por su red local, solo al PC que usted vincule. " +
            "SyncroUmbral no tiene servidores ni cuentas y no envía nada al desarrollador ni a terceros."

    val GENERAL =
        "SyncroUmbral conecta este móvil con su PC. Para eso, según los permisos que usted active, transmite a su PC: " +
            "archivos y fotos, notificaciones, contactos, registro de llamadas, mensajes SMS, la lista de apps y el " +
            "texto del portapapeles que usted envíe. Lo hace mientras el móvil está conectado a ese PC, también con la " +
            "app en segundo plano.\n\n$DESTINATION\n\nCada permiso se pide por separado y puede negarlo o retirarlo cuando quiera."

    val FILES =
        "SyncroUmbral transmite a su PC los nombres y el contenido de los archivos y fotos de este móvil, para que usted " +
            "los explore, copie, renombre y borre desde el PC. Ocurre cuando usted usa Archivos o Galería en el PC (o activa " +
            "allí la copia automática de fotos).\n\n$DESTINATION"

    val NOTIFICATIONS =
        "SyncroUmbral lee las notificaciones de este móvil (app, título, texto y botones, incluidos mensajes y códigos) y " +
            "las transmite a su PC para mostrarlas y responderlas desde allí. Ocurre en segundo plano mientras el móvil " +
            "está conectado a su PC.\n\n$DESTINATION"

    val APP_NOTICES =
        "SyncroUmbral muestra un aviso permanente que indica que está conectado a su PC, con botones para bloquear y " +
            "desbloquear. No transmite datos por este permiso."

    val ANSWER_CALLS =
        "SyncroUmbral permite que su PC conteste y cuelgue las llamadas de este móvil. Para avisarle en el PC transmite " +
            "quién llama, según lo muestre la notificación de la llamada.\n\n$DESTINATION"

    val CONTACTS =
        "SyncroUmbral transmite a su PC los nombres y números de sus contactos, para que usted los busque y llame desde " +
            "el PC. Ocurre cuando usted abre Llamadas en el PC.\n\n$DESTINATION"

    val CALLS =
        "SyncroUmbral transmite a su PC el registro de llamadas (números, nombres, fecha y duración) y permite marcar un " +
            "número desde el PC. Ocurre cuando usted usa Llamadas en el PC.\n\n$DESTINATION"

    val SMS =
        "SyncroUmbral lee sus mensajes SMS, los transmite a su PC y envía los SMS que usted escriba allí (su operador " +
            "puede cobrarlos). Ocurre cuando usted usa Mensajes en el PC.\n\n$DESTINATION"

    val OVERLAY =
        "SyncroUmbral necesita mostrarse sobre otras apps para abrir la pantalla de una llamada de WhatsApp u otra app " +
            "cuando usted pulsa Contestar en el PC. No lee lo que hay en su pantalla."

    val AUTOSTART =
        "En teléfonos Xiaomi, Redmi y POCO, el sistema cierra las apps en segundo plano aunque tengan todos los permisos. " +
            "En la pantalla que se abre, active «Inicio automático» para SyncroUmbral. Conviene además, en las apps recientes, " +
            "mantener pulsada SyncroUmbral y tocar el candado. No transmite datos."

    val BATTERY =
        "Para que Android no corte la conexión con su PC al apagarse la pantalla, quite a SyncroUmbral la optimización de " +
            "batería. En la pantalla que se abre, entre en Batería y elija «Sin restricciones». No transmite datos."
}
