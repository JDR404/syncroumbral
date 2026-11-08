# Reglas de R8 para la versión de producción.
# OkHttp usa proveedores TLS opcionales que no existen en Android: no es un error.
-dontwarn okhttp3.internal.platform.**
-dontwarn org.conscrypt.**
-dontwarn org.bouncycastle.**
-dontwarn org.openjsse.**
