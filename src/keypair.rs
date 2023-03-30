use rand::rngs::OsRng;
use rsa::{PublicKeyParts, RsaPrivateKey};

#[derive(Clone, Debug)]
pub struct KeyPair {
    pub private_key: RsaPrivateKey,
    pub public_key: Box<[u8]>,
}

impl KeyPair {
    pub fn new() -> KeyPair {
        let private_key = RsaPrivateKey::new(&mut OsRng, 1024).unwrap();
        let public_key = rsa_der::public_key_to_der(
            &private_key.n().to_bytes_be(),
            &private_key.e().to_bytes_be(),
        )
        .into_boxed_slice();
        return KeyPair {
            private_key,
            public_key,
        };
    }
}
