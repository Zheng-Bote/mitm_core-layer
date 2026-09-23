import re

with open('mitm_common/src/crypto.rs', 'r') as f:
    content = f.read()

content = re.sub(
    r'    let dek_cipher = Aes256Gcm::new_from_slice\(&dek\)\.map_err\(\|e\| format!\("Invalid DEK: \{\:\?\}", e\)\)\?;\n    \n    let nonce = Nonce::from_slice\(payload_nonce\);',
    r'    let dek_cipher = Aes256Gcm::new_from_slice(&dek).map_err(|e| format!("Invalid DEK: {:?}", e))?;\n    \n    if payload_nonce.len() != 12 {\n        return Err(format!("Invalid payload nonce length: {}", payload_nonce.len()).into());\n    }\n    let nonce = Nonce::from_slice(payload_nonce);',
    content
)

with open('mitm_common/src/crypto.rs', 'w') as f:
    f.write(content)
