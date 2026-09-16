use aes_gcm::{
    aead::{Aead, KeyInit},
    Aes256Gcm, Nonce,
};
use argon2::{Algorithm, Argon2, Params, Version};
use rand::{rngs::OsRng, RngCore};
use std::error::Error;

const SALT_LEN: usize = 16;
const NONCE_LEN: usize = 12;

/// Derive a 32-byte key from password and salt using Argon2id.
pub fn derive_key(password: &[u8], salt: &[u8]) -> Result<[u8; 32], Box<dyn Error>> {
    let params = Params::new(64 * 1024, 3, 1, Some(32))
        .map_err(|e| format!("Invalid Argon2 params: {}", e))?;
    let argon2 = Argon2::new(Algorithm::Argon2id, Version::V0x13, params);
    
    let mut key = [0u8; 32];
    argon2.hash_password_into(password, salt, &mut key)
        .map_err(|e| format!("Argon2 derivation failed: {}", e))?;
    Ok(key)
}

/// Encrypt plaintext using AES-256-GCM.
pub fn encrypt(plaintext: &[u8], password: &[u8]) -> Result<Vec<u8>, Box<dyn Error>> {
    let mut salt = [0u8; SALT_LEN];
    OsRng.fill_bytes(&mut salt);

    let key = derive_key(password, &salt)?;
    let cipher = Aes256Gcm::new_from_slice(&key)
        .map_err(|e| format!("Invalid key length for AES-256: {}", e))?;

    let mut nonce_bytes = [0u8; NONCE_LEN];
    OsRng.fill_bytes(&mut nonce_bytes);
    let nonce = Nonce::from_slice(&nonce_bytes);

    let ciphertext = cipher.encrypt(nonce, plaintext)
        .map_err(|e| format!("Encryption failed: {:?}", e))?;

    let mut result = Vec::with_capacity(SALT_LEN + NONCE_LEN + ciphertext.len());
    result.extend_from_slice(&salt);
    result.extend_from_slice(&nonce_bytes);
    result.extend_from_slice(&ciphertext);

    Ok(result)
}

/// Decrypt ciphertext using AES-256-GCM.
pub fn decrypt(data: &[u8], password: &[u8]) -> Result<Vec<u8>, Box<dyn Error>> {
    if data.len() < SALT_LEN + NONCE_LEN {
        return Err("invalid encrypted data size".into());
    }

    let salt = &data[..SALT_LEN];
    let nonce_bytes = &data[SALT_LEN..SALT_LEN + NONCE_LEN];
    let ciphertext = &data[SALT_LEN + NONCE_LEN..];

    let key = derive_key(password, salt)?;
    let cipher = Aes256Gcm::new_from_slice(&key)
        .map_err(|e| format!("Invalid key length for AES-256: {}", e))?;
    let nonce = Nonce::from_slice(nonce_bytes);

    let plaintext = cipher.decrypt(nonce, ciphertext)
        .map_err(|e| format!("Decryption failed: {:?}", e))?;

    Ok(plaintext)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_encrypt_decrypt() {
        let plaintext = b"hello world";
        let password = b"supersecret";

        let encrypted = encrypt(plaintext, password).unwrap();
        assert_ne!(plaintext, encrypted.as_slice());

        let decrypted = decrypt(&encrypted, password).unwrap();
        assert_eq!(plaintext, decrypted.as_slice());
    }

    #[test]
    fn test_decrypt_wrong_password() {
        let plaintext = b"hello world";
        let password = b"supersecret";

        let encrypted = encrypt(plaintext, password).unwrap();
        let decrypted = decrypt(&encrypted, b"wrongpassword");
        assert!(decrypted.is_err());
    }
}
