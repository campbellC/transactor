use std::collections::HashMap;
use std::error::Error;

use rust_decimal::Decimal;
use serde::Deserialize;
use simple_error::SimpleError;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TransactionType {
    DEPOSIT,
    WITHDRAWAL,
}

pub struct Bank {
    client_accounts: HashMap<u16, Account>,
}

impl Bank {
    pub fn new() -> Self {
        Self {
            client_accounts: HashMap::new(),
        }
    }

    pub fn get_accounts(&self) -> impl Iterator<Item = &Account> {
        self.client_accounts.values()
    }

    pub fn transact(
        &mut self,
        transaction_type: TransactionType,
        client_id: u16,
        _transaction_id: u32,
        amount: Decimal,
    ) -> Result<(), Box<dyn Error>> {
        let account = self
            .client_accounts
            .entry(client_id)
            .or_insert(Account::new(client_id));
        match transaction_type {
            TransactionType::DEPOSIT => account.deposit(amount)?,
            TransactionType::WITHDRAWAL => account.withdraw(amount)?,
        }
        Ok(())
    }
}

pub struct Account {
    pub client_id: u16,
    pub balance: Decimal,
}

impl Account {
    pub fn new(client_id: u16) -> Self {
        Self {
            client_id,
            balance: Decimal::new(0, 4),
        }
    }

    pub fn deposit(&mut self, deposit: Decimal) -> Result<(), Box<dyn Error>> {
        Ok(self.balance = self
            .balance
            .checked_add(deposit)
            .ok_or(SimpleError::new("Overflow on depositing "))?)
    }

    pub fn withdraw(&mut self, withdrawal_amount: Decimal) -> Result<(), Box<dyn Error>> {
        if self.balance >= withdrawal_amount {
            self.balance -=  withdrawal_amount;
        }
        Ok(())
    }
}
