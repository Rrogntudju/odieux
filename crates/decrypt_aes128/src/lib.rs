use anyhow::{Context, Result};
use libaes::Cipher;

pub fn decrypt_aes128(key: &[u8], iv: &[u8], data: &[u8]) -> Result<Vec<u8>> {
    let cipher = Cipher::new_128(key.try_into().with_context(|| "La clé n'a pas une longueur de 16 bytes")?);
    Ok(cipher.cbc_decrypt(iv, data))
}

#[test]
fn test_decrypt() {
    let key = "4567890123456789".as_bytes();
    let iv = "1234567890123456".as_bytes();
    let data = [
        0xDA, 0x52, 0xF9, 0x7B, 0xAB, 0xAE, 0x0A, 0x79, 0x7F, 0x1C, 0x11, 0xEC, 0xB2, 0x09, 0x9F, 0xB0,
    ];

    let result = decrypt_aes128(&key, &iv, &data).unwrap();
    assert_eq!(String::from_utf8(result).unwrap(), "DOH!");
}
