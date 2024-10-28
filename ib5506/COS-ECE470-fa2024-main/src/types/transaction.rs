use serde::{Serialize,Deserialize};
use ring::signature::{Ed25519KeyPair, KeyPair, Signature, UnparsedPublicKey, ED25519};
use rand::Rng;
use crate::types::hash::{Hashable, H256};
use bincode;
use ring::digest;

// Assuming Address struct is defined in another module
use crate::types::address::Address;

#[derive(Serialize, Deserialize, Debug, Default, Clone)]
pub struct Transaction {
    sender: Address,
    receiver: Address,
    value: u64,
}

#[derive(Serialize, Deserialize, Debug, Default, Clone)]
pub struct SignedTransaction {
    transaction: Transaction,
    signature: Vec<u8>,
    public_key: Vec<u8>,
}

impl SignedTransaction {
    pub fn new(transaction: Transaction, key_pair: &Ed25519KeyPair) -> Self {
        // Serialize the transaction
        let serialized_tx = bincode::serialize(&transaction).expect("Failed to serialize transaction");

        // Sign the serialized transaction
        let signature = key_pair.sign(&serialized_tx);
        let signature_vector: Vec<u8> = signature.as_ref().to_vec();

        // Get the public key as a vector of bytes
        let public_key_vector: Vec<u8> = key_pair.public_key().as_ref().to_vec();

        SignedTransaction {
            transaction,
            signature: signature_vector,
            public_key: public_key_vector,
        }
    }
}

impl Hashable for SignedTransaction {
    fn hash(&self) -> H256 {
        ring::digest::digest(&ring::digest::SHA256, &bincode::serialize(self).expect("Failed to serialize SignedTransaction")).into()
    }
}

/// Create digital signature of a transaction
pub fn sign(t: &Transaction, key: &Ed25519KeyPair) -> Signature {
    // Serialize the transaction
    let serialized_tx = bincode::serialize(t).expect("Failed to serialize transaction");

    // Sign the serialized transaction
    key.sign(&serialized_tx)
}

/// Verify digital signature of a transaction, using public key instead of secret key
pub fn verify(t: &Transaction, public_key: &[u8], signature: &[u8]) -> bool {
    // Serialize the transaction
    let serialized_tx = bincode::serialize(t).expect("Failed to serialize transaction");

    // Create an unparsed public key
    let public_key = UnparsedPublicKey::new(&ED25519, public_key);

    // Verify the signature
    public_key.verify(&serialized_tx, signature).is_ok()
}

#[cfg(any(test, test_utilities))]
pub fn generate_random_transaction() -> Transaction {
    let mut rng = rand::thread_rng();

    // Generate random addresses
    let mut sender_bytes = [0u8; 20];
    let mut receiver_bytes = [0u8; 20];
    rng.fill(&mut sender_bytes);
    rng.fill(&mut receiver_bytes);

    let sender = Address::from(sender_bytes);
    let receiver = Address::from(receiver_bytes);
    
    // Generate a random value
    let value = rng.gen_range(1..1000);

    Transaction {
        sender,
        receiver,
        value,
    }
}

// DO NOT CHANGE THIS COMMENT, IT IS FOR AUTOGRADER. BEFORE TEST

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::key_pair;
    use ring::signature::KeyPair;
    use ring::digest;



    #[test]
    fn sign_verify() {
        let t = generate_random_transaction();
        let key = key_pair::random();
        let signature = sign(&t, &key);
        assert!(verify(&t, key.public_key().as_ref(), signature.as_ref()));
    }
    #[test]
    fn sign_verify_two() {
        let t = generate_random_transaction();
        let key = key_pair::random();
        let signature = sign(&t, &key);
        let key_2 = key_pair::random();
        let t_2 = generate_random_transaction();
        assert!(!verify(&t_2, key.public_key().as_ref(), signature.as_ref()));
        assert!(!verify(&t, key_2.public_key().as_ref(), signature.as_ref()));
    }
}

// DO NOT CHANGE THIS COMMENT, IT IS FOR AUTOGRADER. AFTER TEST