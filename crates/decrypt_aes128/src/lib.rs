use aes::Aes128;
use anyhow::{Context, Result};
use block_modes::block_padding::Pkcs7;
use block_modes::{BlockMode, Cbc};

type Aes128Cbc = Cbc<Aes128, Pkcs7>;

pub fn decrypt_aes128(key: &[u8], iv: &[u8], data: &[u8]) -> Result<Vec<u8>> {
    let mut encrypted_data = data.to_owned();
    let cipher = Aes128Cbc::new_from_slices(&key, &iv).context("Création du chiffre AES-128 CBC")?;
    Ok(cipher.decrypt(&mut encrypted_data).context("Décryption")?.to_vec())
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
