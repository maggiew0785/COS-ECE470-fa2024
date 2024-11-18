use std::collections::HashMap;
use crate::types::address::Address;
use serde::{Serialize, Deserialize};
use crate::types::transaction::SignedTransaction;

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct AccountState {
    pub nonce: u32,
    pub balance: u64,
}

#[derive(Clone, Debug)]
pub struct State {
    // Map from account address to its state (nonce and balance)
    pub accounts: HashMap<Address, AccountState>,
}

impl State {
    pub fn new() -> Self {
        State {
            accounts: HashMap::new(),
        }
    }

    pub fn create_account(&mut self, address: Address, initial_balance: u64) {
        self.accounts.insert(address, AccountState {
            nonce: 0,
            balance: initial_balance,
        });
    }

    pub fn get_account_state(&self, address: &Address) -> Option<&AccountState> {
        self.accounts.get(address)
    }

    pub fn update_balance(&mut self, address: &Address, new_balance: u64) {
        if let Some(account) = self.accounts.get_mut(address) {
            account.balance = new_balance;
        }
    }

    pub fn increment_nonce(&mut self, address: &Address) {
        if let Some(account) = self.accounts.get_mut(address) {
            account.nonce += 1;
        }
    }

    pub fn process_transaction(&mut self, tx: &SignedTransaction) -> Result<(), String> {
        let sender = Address::from_public_key_bytes(&tx.public_key);
        
        // Get sender's account
        let sender_account = self.get_account_state(&sender)
            .ok_or("Sender account not found")?;
        
        // Verify signature (this proves ownership)
        if !tx.verify(&self) {
            return Err("Invalid signature".to_string());
        }

        // Update sender
        self.update_balance(&sender, sender_account.balance - tx.transaction.value);
        self.increment_nonce(&sender);
        
        // Update receiver
        let receiver = &tx.transaction.receiver;
        if let Some(receiver_account) = self.get_account_state(receiver) {
            self.update_balance(receiver, receiver_account.balance + tx.transaction.value);
        } else {
            self.create_account(*receiver, tx.transaction.value);
        }
        
        Ok(())
    }
}