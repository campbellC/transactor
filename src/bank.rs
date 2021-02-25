use std::collections::HashMap;
use std::error::Error;

use rust_decimal::prelude::*;
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
            TransactionType::WITHDRAWAL => account.withdraw(amount),
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

    pub fn withdraw(&mut self, withdrawal_amount: Decimal) {
        if self.balance >= withdrawal_amount {
            self.balance -=  withdrawal_amount;
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn account_does_not_withdraw_negative_amounts() {
        let mut account = Account::new(1);
        account.withdraw(Decimal::new(10000, 4));
        assert_eq!(account.balance, Decimal::zero());
    }

    #[test]
    fn account_deposit_adds_to_total() -> Result<(), Box<dyn Error>> {
        let mut account = Account::new(1);
        let amount = Decimal::new(1, 4);
        account.deposit(amount)?;
        account.deposit(amount)?;
        assert_eq!(account.balance, Decimal::new(2, 4));
        Ok(())
    }

    #[test]
    fn account_deposit_with_huge_numbers_fails_but_does_not_panic() -> Result<(), Box<dyn Error>> {
        let mut account = Account::new(1);
        let amount = Decimal::max_value();
        account.deposit(amount)?;
        assert!(account.deposit(amount).is_err());
        Ok(())
    }
}
