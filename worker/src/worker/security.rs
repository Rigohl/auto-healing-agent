//! Seguridad del webhook.
//!
//! El protocolo NO cambia: se mantiene el header x-webhook-secret con
//! igualdad contra el secreto configurado (los senders actuales - deploy.yml,
//! Actions - ya lo usan). Lo que SI cambia: la comparacion es en tiempo
//! constante y la idempotencia por delivery-id limita el replay de una misma
//! entrega. La evaluacion completa (HMAC, timestamp, replay window) y las
//! razones de diferir cada mecanismo estan en docs/PART3_CLOUDFLARE_RUNTIME.md
//! (seccion 14).

/// Comparacion en tiempo constante: acumula el XOR de todos los bytes sin
/// ramas dependientes del contenido. Con longitudes distintas devuelve false
/// de inmediato (la longitud del secreto no es secreta en este protocolo).
pub fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut acc: u8 = 0;
    for (x, y) in a.iter().zip(b.iter()) {
        acc |= x ^ y;
    }
    acc == 0
}

/// Verifica el header x-webhook-secret contra el secreto configurado.
pub fn verify_webhook_secret(header: Option<&str>, secret: &str) -> bool {
    match header {
        Some(h) => constant_time_eq(h.as_bytes(), secret.as_bytes()),
        None => false,
    }
}

/// FNV-1a 64 bits. SOLO para ids de correlacion y huellas NO criptologicas
/// (correlation_id, idem_key, patch fingerprint). No es un MAC.
pub fn fnv1a64(bytes: &[u8]) -> u64 {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    hash
}

/// Codifica un string para usarlo con seguridad en un query param.
pub fn urlencode(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for byte in s.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(byte as char);
            }
            _ => {
                out.push('%');
                out.pus
h_str(&format!("{:02X}", byte));
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn constant_time_eq_matches_and_differs() {
        assert!(constant_time_eq(b"abc", b"abc"));
        assert!(!constant_time_eq(b"abc", b"abd"));
        assert!(!constant_time_eq(b"abc", b"abcd"));
        assert!(constant_time_eq(b"", b""));
    }

    #[test]
    fn verify_secret_rejects_missing_header() {
        assert!(!verify_webhook_secret(None, "s"));
        assert!(verify_webhook_secret(Some("s"), "s"));
        assert!(!verify_webhook_secret(Some("x"), "s"));
    }

    #[test]
    fn fnv1a64_known_vectors() {
        // Vector de referencia publico de FNV-1a 64.
        assert_eq!(fnv1a64(b""), 0xcbf2_9ce4_8422_2325);
        assert_eq!(fnv1a64(b"a"), 0xaf63_dc4c_8601_ec8c);
    }

    #[test]
    fn urlencode_escapes_reserved() {
        assert_eq!(urlencode("a b/c"), "a%20b%2Fc");
        assert_eq!(urlencode("AZ09-_.~"), "AZ09-_.~");
    }
}
