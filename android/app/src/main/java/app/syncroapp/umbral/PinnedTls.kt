package app.syncroapp.umbral

import java.security.SecureRandom
import java.security.cert.CertificateException
import java.security.cert.X509Certificate
import javax.net.ssl.SSLContext
import javax.net.ssl.SSLSocketFactory
import javax.net.ssl.X509TrustManager

/** Confía únicamente en el certificado cuya huella SHA-256 coincide con la del QR. */
class PinnedTls(private val expectedFp: String) {
    val trustManager = object : X509TrustManager {
        override fun checkClientTrusted(chain: Array<X509Certificate>, authType: String) {
            throw CertificateException("no se usa")
        }

        override fun checkServerTrusted(chain: Array<X509Certificate>, authType: String) {
            val fp = KeyManager.sha256Hex(chain[0].encoded)
            if (!fp.equals(expectedFp, ignoreCase = true)) {
                throw CertificateException("Huella del PC no coincide")
            }
        }

        override fun getAcceptedIssuers(): Array<X509Certificate> = emptyArray()
    }

    val socketFactory: SSLSocketFactory = SSLContext.getInstance("TLS").run {
        init(null, arrayOf(trustManager), SecureRandom())
        socketFactory
    }
}
