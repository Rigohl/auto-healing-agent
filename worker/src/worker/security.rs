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
                out.push_str(&format!("{:02X}", byte));
            }
        }
    }
    out
}

// ---------------- SHA-256 puro (FIPS 180-4) ----------------
// Sin dependencias nuevas: el mismo codigo compila en host (worker-test)
// y en wasm32 (worker-check). Solo autentica bodies de webhook de GitHub
// (payloads de incidente acotados), no archivos arbitrarios.

/// Constantes de ronda de SHA-256 (FIPS 180-4, seccion 4.2.2).
const SHA256_K: [u32; 64] = [
    0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4, 0xab1c5ed5,
    0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe, 0x9bdc06a4, 0xc19bf174,
    0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f, 0x4a7484aa, 0x5cb0a9dc, 0x76f988da,
    0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7, 0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967,
    0x27b70a85, 0x2e1b2138, 0x4d2c6dfc, 0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85,
    0xa2bfe8a1, 0xa81a664b, 0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070,
    0x19a4c116, 0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
    0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7, 0xc67178f2,
];

/// SHA-256 incremental (FIPS 180-4).
struct Sha256 {
    state: [u32; 8],
    buf: [u8; 64],
    buf_len: usize,
    total: u64,
}

impl Sha256 {
    fn new() -> Self {
        Self {
            state: [
                0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a, 0x510e527f, 0x9b05688c, 0x1f83d9ab,
                0x5be0cd19,
            ],
            buf: [0; 64],
            buf_len: 0,
            total: 0,
        }
    }

    fn update(&mut self, mut data: &[u8]) {
        self.total = self.total.wrapping_add(data.len() as u64);
        if self.buf_len > 0 {
            let take = (64 - self.buf_len).min(data.len());
            self.buf[self.buf_len..self.buf_len + take].copy_from_slice(&data[..take]);
            self.buf_len += take;
            data = &data[take..];
            if self.buf_len == 64 {
                let block = self.buf;
                self.compress(&block);
                self.buf_len = 0;
            }
        }
        while data.len() >= 64 {
            let mut block = [0u8; 64];
            block.copy_from_slice(&data[..64]);
            self.compress(&block);
            data = &data[64..];
        }
        if !data.is_empty() {
            self.buf[..data.len()].copy_from_slice(data);
            self.buf_len = data.len();
        }
    }

    fn finalize(mut self) -> [u8; 32] {
        let bits = self.total.wrapping_mul(8);
        // Padding FIPS 180-4: 0x80, ceros hasta 56 mod 64, longitud en bits.
        self.update(&[0x80]);
        while self.buf_len % 64 != 56 {
            self.update(&[0x00]);
        }
        self.update(&bits.to_be_bytes());
        let mut out = [0u8; 32];
        for (word, bytes) in self.state.iter().zip(out.chunks_exact_mut(4)) {
            bytes.copy_from_slice(&word.to_be_bytes());
        }
        out
    }

    fn compress(&mut self, block: &[u8; 64]) {
        let mut w = [0u32; 64];
        for (i, chunk) in block.chunks_exact(4).enumerate() {
            w[i] = u32::from_be_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]);
        }
        for i in 16..64 {
            let s0 = w[i - 15].rotate_right(7) ^ w[i - 15].rotate_right(18) ^ (w[i - 15] >> 3);
            let s1 = w[i - 2].rotate_right(17) ^ w[i - 2].rotate_right(19) ^ (w[i - 2] >> 10);
            w[i] = w[i - 16]
                .wrapping_add(s0)
                .wrapping_add(w[i - 7])
                .wrapping_add(s1);
        }
        let [mut a, mut b, mut c, mut d, mut e, mut f, mut g, mut h] = self.state;
        for i in 0..64 {
            let s1 = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
            let ch = (e & f) ^ (!e & g);
            let t1 = h
                .wrapping_add(s1)
                .wrapping_add(ch)
                .wrapping_add(SHA256_K[i])
                .wrapping_add(w[i]);
            let s0 = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
            let maj = (a & b) ^ (a & c) ^ (b & c);
            let t2 = s0.wrapping_add(maj);
            h = g;
            g = f;
            f = e;
            e = d.wrapping_add(t1);
            d = c;
            c = b;
            b = a;
            a = t1.wrapping_add(t2);
        }
        let add = [a, b, c, d, e, f, g, h];
        for (s, v) in self.state.iter_mut().zip(add) {
            *s = s.wrapping_add(v);
        }
    }
}

/// SHA-256 de una entrada completa (FIPS 180-4).
pub fn sha256(data: &[u8]) -> [u8; 32] {
    let mut h = Sha256::new();
    h.update(data);
    h.finalize()
}

/// HMAC-SHA256 (RFC 2104) con bloque de 64 bytes: ipad 0x36 / opad 0x5c.
/// Clave mas larga que el bloque: se hashea primero (RFC 2104).
pub fn hmac_sha256(key: &[u8], msg: &[u8]) -> [u8; 32] {
    let mut block_key = [0u8; 64];
    if key.len() > 64 {
        block_key[..32].copy_from_slice(&sha256(key));
    } else {
        block_key[..key.len()].copy_from_slice(key);
    }
    let mut ipad = [0x36u8; 64];
    let mut opad = [0x5cu8; 64];
    for ((ip, op), k) in ipad
        .iter_mut()
        .zip(opad.iter_mut())
        .zip(block_key.iter())
    {
        *ip ^= *k;
        *op ^= *k;
    }
    let mut inner = Sha256::new();
    inner.update(&ipad);
    inner.update(msg);
    let inner_digest = inner.finalize();
    let mut outer = Sha256::new();
    outer.update(&opad);
    outer.update(&inner_digest);
    outer.finalize()
}

/// Hexadecimal en minusculas (GitHub envia el digest en minusculas).
pub fn hex_lower(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        out.push_str(&format!("{:02x}", b));
    }
    out
}

/// Verifica la firma nativa de GitHub (x-hub-signature-256):
/// "sha256=" + HMAC-SHA256(secret, body) en hex. Fail-closed: secret vacio,
/// header ausente o prefijo distinto => false, nunca abre el endpoint. El
/// digest se compara en tiempo constante con el MISMO constant_time_eq del
/// secret compartido. Camino ADITIVO (PART3 14.1): x-webhook-secret sigue
/// siendo la primera via y no cambia.
pub fn verify_github_signature(header: Option<&str>, body: &[u8], secret: &str) -> bool {
    if secret.is_empty() {
        return false;
    }
    let signature = match header {
        Some(h) => match h.strip_prefix("sha256=") {
            Some(s) => s.trim().to_ascii_lowercase(),
            None => return false,
        },
        None => return false,
    };
    let expected = hex_lower(&hmac_sha256(secret.as_bytes(), body));
    constant_time_eq(signature.as_bytes(), expected.as_bytes())
}


#[cfg(test)]
mod tests {
    use super::*;

#[test]
    fn sha256_fips_vectors() {
        // FIPS 180-4: vacio, "abc" y el vector clasico de dos bloques.
        assert_eq!(
            hex_lower(&sha256(b"")),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
        assert_eq!(
            hex_lower(&sha256(b"abc")),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
        assert_eq!(
            hex_lower(&sha256(
                b"abcdbcdecdefdefgefghfghighijhijkijkljklmklmnlmnomnopnopq"
            )),
            "248d6a61d20638b8e5c026930c3e6039a33ce45964ff2167f6ecedd419db06c1"
        );
    }

    #[test]
    fn hmac_sha256_rfc4231_vectors() {
        // RFC 4231 casos 1, 2, 3 y 7. El 7 ejercita clave mayor que el
        // bloque (se hashea antes de usarse).
        let key1 = [0x0bu8; 20];
        assert_eq!(
            hex_lower(&hmac_sha256(&key1, b"Hi There")),
            "b0344c61d8db38535ca8afceaf0bf12b881dc200c9833da726e9376c2e32cff7"
        );
        assert_eq!(
            hex_lower(&hmac_sha256(b"Jefe", b"what do ya want for nothing?")),
            "5bdcc146bf60754e6a042426089575c75a003f089d2739839dec58b964ec3843"
        );
        let key3 = [0xaau8; 20];
        let data3 = [0xddu8; 50];
        assert_eq!(
            hex_lower(&hmac_sha256(&key3, &data3)),
            "773ea91e36800e46854db8ebd09181a72959098b3ef8c122d9635514ced565fe"
        );
        let key7 = [0xaau8; 131];
        let data7: &[u8] = b"This is a test using a larger than block-size key \
        and a larger than block-size data. The key needs to be hashed \
        before being used by the HMAC algorithm.";
        assert_eq!(
            hex_lower(&hmac_sha256(&key7, data7)),
            "9b09ffa71b942fcb27635fbcd5b0e944bfdc63644f0713938a7f51535c3a35e2"
        );
    }

    #[test]
    fn github_signature_accepts_and_rejects() {
        let body = br#"{"error_code":"build_error"}"#;
        let secret = "gh-secret";
        let digest = hex_lower(&hmac_sha256(secret.as_bytes(), body));
        let good = format!("sha256={digest}");
        assert!(verify_github_signature(Some(&good), body, secret));
        // GitHub envia minusculas; se acepta tambien mayusculas (normalizado).
        let upper = format!("sha256={}", digest.to_uppercase());
        assert!(verify_github_signature(Some(&upper), body, secret));
        // Prefijo distinto, digest distinto, body alterado, header ausente y
        // secret vacio: todos fail-closed.
        assert!(!verify_github_signature(Some("sha1=00"), body, secret));
        assert!(!verify_github_signature(Some("sha256=00"), body, secret));
        assert!(!verify_github_signature(Some(&good), b"otro body", secret));
        assert!(!verify_github_signature(None, body, secret));
        assert!(!verify_github_signature(Some(&good), body, ""));
    }


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
