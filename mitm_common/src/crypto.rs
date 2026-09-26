/*
 * SPDX-License-Identifier: Apache-2.0
 */

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

pub fn envelope_decrypt(kek: &[u8], wrapped_key: &[u8], payload_nonce: &[u8], payload: &[u8]) -> Result<Vec<u8>, Box<dyn Error>> {
    let mut adjusted_kek = [0u8; 32];
    let len = std::cmp::min(kek.len(), 32);
    adjusted_kek[..len].copy_from_slice(&kek[..len]);

    if wrapped_key.len() < 12 {
        return Err("wrapped DEK too short".into());
    }
    
    let dek_nonce = Nonce::from_slice(&wrapped_key[..12]);
    let wrapped_cipher = &wrapped_key[12..];

    let kek_cipher = Aes256Gcm::new_from_slice(&adjusted_kek).map_err(|e| format!("Invalid KEK: {:?}", e))?;
    let dek = kek_cipher.decrypt(dek_nonce, wrapped_cipher)
        .map_err(|e| format!("Failed to decrypt DEK: {:?}", e))?;

    let dek_cipher = Aes256Gcm::new_from_slice(&dek).map_err(|e| format!("Invalid DEK: {:?}", e))?;
    
    if payload_nonce.len() != 12 {
        return Err(format!("Invalid payload nonce length: {}", payload_nonce.len()).into());
    }
    let nonce = Nonce::from_slice(payload_nonce);
    let plaintext = dek_cipher.decrypt(nonce, payload)
        .map_err(|e| format!("Failed to decrypt payload: {:?}", e))?;

    Ok(plaintext)
}

pub fn generate_wrapped_dek(kek: &[u8]) -> Result<Vec<u8>, Box<dyn Error>> {
    let mut adjusted_kek = [0u8; 32];
    let len = std::cmp::min(kek.len(), 32);
    adjusted_kek[..len].copy_from_slice(&kek[..len]);

    let mut dek = [0u8; 32];
    OsRng.fill_bytes(&mut dek);

    let mut dek_nonce_bytes = [0u8; 12];
    OsRng.fill_bytes(&mut dek_nonce_bytes);
    let dek_nonce = Nonce::from_slice(&dek_nonce_bytes);

    let kek_cipher = Aes256Gcm::new_from_slice(&adjusted_kek).map_err(|e| format!("Invalid KEK: {:?}", e))?;
    let wrapped_cipher = kek_cipher.encrypt(dek_nonce, dek.as_ref())
        .map_err(|e| format!("Failed to encrypt DEK: {:?}", e))?;

    let mut wrapped_key = Vec::with_capacity(dek_nonce_bytes.len() + wrapped_cipher.len());
    wrapped_key.extend_from_slice(&dek_nonce_bytes);
    wrapped_key.extend_from_slice(&wrapped_cipher);

    Ok(wrapped_key)
}

pub fn envelope_encrypt(kek: &[u8], wrapped_key: &[u8], plaintext: &[u8]) -> Result<(Vec<u8>, Vec<u8>), Box<dyn Error>> {
    let mut adjusted_kek = [0u8; 32];
    let len = std::cmp::min(kek.len(), 32);
    adjusted_kek[..len].copy_from_slice(&kek[..len]);

    if wrapped_key.len() < 12 {
        return Err("wrapped DEK too short".into());
    }

    let dek_nonce = Nonce::from_slice(&wrapped_key[..12]);
    let wrapped_cipher = &wrapped_key[12..];

    let kek_cipher = Aes256Gcm::new_from_slice(&adjusted_kek).map_err(|e| format!("Invalid KEK: {:?}", e))?;
    let dek = kek_cipher.decrypt(dek_nonce, wrapped_cipher)
        .map_err(|e| format!("Failed to decrypt DEK: {:?}", e))?;

    let dek_cipher = Aes256Gcm::new_from_slice(&dek).map_err(|e| format!("Invalid DEK: {:?}", e))?;

    let mut payload_nonce_bytes = [0u8; 12];
    OsRng.fill_bytes(&mut payload_nonce_bytes);
    let nonce = Nonce::from_slice(&payload_nonce_bytes);

    let ciphertext = dek_cipher.encrypt(nonce, plaintext)
        .map_err(|e| format!("Failed to encrypt payload: {:?}", e))?;

    Ok((ciphertext, payload_nonce_bytes.to_vec()))
}

#[cfg(test)]
mod envelope_tests {
    use super::*;

    #[test]
    fn test_envelope_encrypt_decrypt() {
        let kek = b"0123456789abcdef0123456789abcdef"; // 32 bytes
        let plaintext = b"some secret role assignments json";

        let wrapped_dek = generate_wrapped_dek(kek).unwrap();
        let (ciphertext, payload_nonce) = envelope_encrypt(kek, &wrapped_dek, plaintext).unwrap();
        let decrypted = envelope_decrypt(kek, &wrapped_dek, &payload_nonce, &ciphertext).unwrap();

        assert_eq!(plaintext, decrypted.as_slice());
    }
}
