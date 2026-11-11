use hmac::{Hmac, Mac};
use serde::Serialize;
use sha1::Sha1;
use sha2::{Sha256, Sha512};
use std::time::{SystemTime, UNIX_EPOCH};
use url::Url;

use crate::vault::Secret;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TotpCode {
    pub code: String,
    pub period: u64,
    pub remaining: u64,
    pub valid_until: u64,
}

pub fn generate(secret: &Secret) -> Result<TotpCode, String> {
    let uri =
        find_uri(secret).ok_or("Esta entrada não contém um URI TOTP compatível com pass-otp")?;
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| "Relógio do sistema inválido")?
        .as_secs();
    code_at(uri, now)
}

fn find_uri(secret: &Secret) -> Option<&str> {
    std::iter::once(secret.password.as_str())
        .chain(secret.details.lines())
        .find_map(|line| {
            let line = line.trim();
            if line.starts_with("otpauth://") {
                Some(line)
            } else if let Some((key, value)) = line.split_once(':') {
                (key.eq_ignore_ascii_case("otpauth") && value.trim().starts_with("otpauth://"))
                    .then_some(value.trim())
            } else {
                None
            }
        })
}

fn code_at(uri: &str, now: u64) -> Result<TotpCode, String> {
    if uri.len() > 8192 {
        return Err("URI TOTP muito longo".into());
    }
    let parsed = Url::parse(uri).map_err(|_| "URI TOTP inválido")?;
    if parsed.scheme() != "otpauth"
        || parsed.host_str() != Some("totp")
        || parsed.path().trim_matches('/').is_empty()
    {
        return Err("Somente URI otpauth://totp é suportado".into());
    }
    let mut secret = None;
    let mut algorithm = None;
    let mut digits = None;
    let mut period = None;
    for (key, value) in parsed.query_pairs() {
        match key.as_ref() {
            "secret" => {
                if secret.replace(value.into_owned()).is_some() {
                    return Err("Parâmetro TOTP duplicado".into());
                }
            }
            "algorithm" => {
                if algorithm.replace(value.into_owned()).is_some() {
                    return Err("Parâmetro TOTP duplicado".into());
                }
            }
            "digits" => {
                if digits.replace(value.into_owned()).is_some() {
                    return Err("Parâmetro TOTP duplicado".into());
                }
            }
            "period" => {
                if period.replace(value.into_owned()).is_some() {
                    return Err("Parâmetro TOTP duplicado".into());
                }
            }
            _ => {}
        }
    }
    let secret = decode_base32(secret.as_deref().ok_or("URI TOTP sem segredo")?)?;
    let algorithm = algorithm.as_deref().unwrap_or("SHA1").to_ascii_uppercase();
    let digits: u32 = digits
        .as_deref()
        .unwrap_or("6")
        .parse()
        .map_err(|_| "Quantidade de dígitos TOTP inválida")?;
    if !(6..=8).contains(&digits) {
        return Err("Quantidade de dígitos TOTP inválida".into());
    }
    let period: u64 = period
        .as_deref()
        .unwrap_or("30")
        .parse()
        .map_err(|_| "Período TOTP inválido")?;
    if !(1..=3600).contains(&period) {
        return Err("Período TOTP inválido".into());
    }
    let counter = (now / period).to_be_bytes();
    macro_rules! hmac_bytes {
        ($hash:ty) => {{
            let mut mac =
                Hmac::<$hash>::new_from_slice(&secret).map_err(|_| "Segredo TOTP inválido")?;
            mac.update(&counter);
            mac.finalize().into_bytes().to_vec()
        }};
    }
    let digest = match algorithm.as_str() {
        "SHA1" => hmac_bytes!(Sha1),
        "SHA256" => hmac_bytes!(Sha256),
        "SHA512" => hmac_bytes!(Sha512),
        _ => return Err("Algoritmo TOTP não suportado".into()),
    };
    let offset = usize::from(digest[digest.len() - 1] & 0x0f);
    let binary = u32::from_be_bytes(
        digest[offset..offset + 4]
            .try_into()
            .map_err(|_| "Código TOTP inválido")?,
    ) & 0x7fff_ffff;
    let code = format!(
        "{:0width$}",
        binary % 10u32.pow(digits),
        width = digits as usize
    );
    let remaining = period - (now % period);
    Ok(TotpCode {
        code,
        period,
        remaining,
        valid_until: now + remaining,
    })
}

fn decode_base32(value: &str) -> Result<Vec<u8>, String> {
    let mut output = Vec::new();
    let mut buffer = 0u32;
    let mut bits = 0u8;
    let mut padding = false;
    for byte in value.bytes() {
        if byte.is_ascii_whitespace() {
            continue;
        }
        if byte == b'=' {
            padding = true;
            continue;
        }
        if padding {
            return Err("Segredo TOTP Base32 inválido".into());
        }
        let upper = byte.to_ascii_uppercase();
        let digit = match upper {
            b'A'..=b'Z' => upper - b'A',
            b'2'..=b'7' => upper - b'2' + 26,
            _ => return Err("Segredo TOTP Base32 inválido".into()),
        };
        buffer = (buffer << 5) | u32::from(digit);
        bits += 5;
        if bits >= 8 {
            bits -= 8;
            output.push((buffer >> bits) as u8);
            buffer &= (1 << bits) - 1;
        }
    }
    if output.is_empty() || buffer != 0 {
        return Err("Segredo TOTP Base32 inválido".into());
    }
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rfc6238_vectors_for_sha1_sha256_sha512() {
        let vectors = [
            ("GEZDGNBVGY3TQOJQGEZDGNBVGY3TQOJQ", "SHA1", "94287082"),
            ("GEZDGNBVGY3TQOJQGEZDGNBVGY3TQOJQGEZDGNBVGY3TQOJQGEZA", "SHA256", "46119246"),
            ("GEZDGNBVGY3TQOJQGEZDGNBVGY3TQOJQGEZDGNBVGY3TQOJQGEZDGNBVGY3TQOJQGEZDGNBVGY3TQOJQGEZDGNBVGY3TQOJQGEZDGNA", "SHA512", "90693936"),
        ];
        for (secret, algorithm, expected) in vectors {
            let uri = format!(
                "otpauth://totp/Test?secret={secret}&algorithm={algorithm}&digits=8&period=30"
            );
            assert_eq!(code_at(&uri, 59).unwrap().code, expected, "{algorithm}");
        }
    }

    #[test]
    fn finds_pass_otp_uri_and_rejects_invalid_parameters() {
        let entry = Secret {
            password: "password".into(),
            details: "login: alice\notpauth://totp/Example?secret=JBSWY3DPEHPK3PXP\n".into(),
            other_details: String::new(),
            username: None,
            urls: Vec::new(),
            ciphertext_hash: String::new(),
        };
        assert!(find_uri(&entry).is_some());
        let uri = "otpauth://totp/Test?secret=GEZDGNBVGY3TQOJQGEZDGNBVGY3TQOJQ";
        let code = code_at(uri, 59).unwrap();
        assert_eq!(code.code, "287082");
        assert_eq!(code.remaining, 1);
        assert_eq!(code.valid_until, 60);
        assert_eq!(code_at(uri, 60).unwrap().remaining, 30);
        assert!(code_at("otpauth://hotp/Test?secret=JBSWY3DPEHPK3PXP", 59).is_err());
        assert!(code_at("otpauth://totp/Test?secret=JBSWY3DPEHPK3PXP&period=0", 59).is_err());
        assert!(code_at("otpauth://totp/Test?secret=JBSWY3DPEHPK3PXP&digits=9", 59).is_err());
        assert!(code_at("otpauth://totp/Test?secret=bad!", 59).is_err());
        assert!(code_at("otpauth://totp/Test?secret=ABC&secret=DEF", 59).is_err());
    }
}
