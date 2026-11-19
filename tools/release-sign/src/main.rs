//! Uso:
//!   release-sign gen  CLAVE.key             crea un par de claves; guarda la privada y muestra la pública (SPKI DER en base64)
//!   release-sign sign CLAVE.key ARCHIVO     escribe ARCHIVO.sig (firma ECDSA P-256 SHA-256, DER en base64)
//!   release-sign verify PUBLICA_B64 ARCHIVO ARCHIVO.sig
//! La clave privada nunca debe subirse al repositorio ni a GitHub: si se perdiera, hay que publicar una versión con otra clave.

use base64::{engine::general_purpose::STANDARD as B64, Engine};
use p256::ecdsa::{signature::{Signer, Verifier}, Signature, SigningKey, VerifyingKey};
use p256::pkcs8::{DecodePrivateKey, DecodePublicKey, EncodePrivateKey, EncodePublicKey};

fn main() {
    let a: Vec<String> = std::env::args().collect();
    match a.get(1).map(String::as_str) {
        Some("gen") => {
            let key = SigningKey::random(&mut rand::rngs::OsRng);
            let der = key.to_pkcs8_der().expect("clave");
            std::fs::write(&a[2], B64.encode(der.as_bytes())).expect("no se pudo guardar la clave");
            let public = key.verifying_key().to_public_key_der().expect("pública");
            println!("{}", B64.encode(public.as_bytes()));
        }
        Some("sign") => {
            let key = SigningKey::from_pkcs8_der(&B64.decode(std::fs::read_to_string(&a[2]).unwrap().trim()).unwrap()).expect("clave inválida");
            let data = std::fs::read(&a[3]).expect("archivo");
            let sig: Signature = key.sign(&data);
            std::fs::write(format!("{}.sig", a[3]), B64.encode(sig.to_der().as_bytes())).expect("no se pudo escribir la firma");
        }
        Some("verify") => {
            let key = VerifyingKey::from_public_key_der(&B64.decode(&a[2]).unwrap()).expect("pública inválida");
            let data = std::fs::read(&a[3]).expect("archivo");
            let sig = Signature::from_der(&B64.decode(std::fs::read_to_string(&a[4]).unwrap().trim()).unwrap()).expect("firma");
            match key.verify(&data, &sig) {
                Ok(()) => println!("firma válida"),
                Err(_) => {
                    println!("FIRMA NO VÁLIDA");
                    std::process::exit(1);
                }
            }
        }
        _ => eprintln!("uso: release-sign gen|sign|verify (ver el comentario de src/main.rs)"),
    }
}
