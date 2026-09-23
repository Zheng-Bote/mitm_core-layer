import re

with open('mitm_scheduler-server/src/main.rs', 'r') as f:
    content = f.read()

# Fix the base64 decoding at the top
content = re.sub(
    r'    // Decode MASTER_KEY if it is exactly 44 characters \(Base64 encoding of 32 bytes\)\n    let mk = if password.len\(\) == 44 \{\n        use base64::\{Engine as _, engine::general_purpose::STANDARD as BASE64\};\n        BASE64.decode\(&password\).unwrap_or_else\(\|_\| password.as_bytes\(\).to_vec\(\)\)\n    \} else \{\n        password.as_bytes\(\).to_vec\(\)\n    \};\n',
    r'    // Decode MASTER_KEY if it is exactly 44 characters (Base64 encoding of 32 bytes)\n    let kek = if password.len() == 44 {\n        use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};\n        BASE64.decode(&password).unwrap_or_else(|_| password.as_bytes().to_vec())\n    } else {\n        password.as_bytes().to_vec()\n    };\n',
    content
)

# Fix the clone inside the loop
content = re.sub(
    r'                        let mk = master_key_str\.clone\(\);\n                        let db_cfg = db_config_json\.clone\(\);',
    r'                        let mk = master_key_str.clone();\n                        let kek_clone = kek.clone();\n                        let db_cfg = db_config_json.clone();',
    content
)

# Fix the envelope_encrypt call
content = re.sub(
    r'mitm_common::crypto::envelope_encrypt\(mk\.as_bytes\(\), &wrapped_dek, &plaintext\)',
    r'mitm_common::crypto::envelope_encrypt(&kek_clone, &wrapped_dek, &plaintext)',
    content
)

# Fix the envelope_decrypt call
content = re.sub(
    r'mitm_common::crypto::envelope_decrypt\(mk\.as_bytes\(\), &wrapped_dek, &nonce, &ciphertext\)',
    r'mitm_common::crypto::envelope_decrypt(&kek_clone, &wrapped_dek, &nonce, &ciphertext)',
    content
)

with open('mitm_scheduler-server/src/main.rs', 'w') as f:
    f.write(content)
