use std::collections::HashMap;
use std::error::Error;

use rust_decimal::prelude::*;
use simple_error::SimpleError;

#[derive(Debug, Eq, PartialEq, Copy, Clone)]
pub struct Transaction {
    transaction_id: u32,
    amount: Decimal,
}

impl Transaction {
    pub fn new(transaction_id: u32, amount: Decimal) -> Self {
        Self {
            transaction_id,
            amount,
        }
    }
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

    /// Perform a transaction on a clients account.
    /// Error can occur if any of:
    /// * the transaction causes an overflow
    /// * the transaction has already been recorded as occurring
    /// If the transaction is a withdrawal and would leave the account in negative balance the transaction will not occur and will not be recorded.
    /// If the account is locked, no action will be taken and the transaction will not be recorded.
    pub fn transact(
        &mut self,
        client_id: u16,
        transaction: Transaction,
    ) -> Result<(), Box<dyn Error>> {
        let account = self.account(client_id);

        if account.locked {
            return Ok(());
        }

        if account
            .transaction_history
            .contains_key(&transaction.transaction_id)
        {
            return Err(Box::new(SimpleError::new(
                "Transaction attempted twice with same id",
            )));
        }

        let new_balance = account
            .available
            .checked_add(transaction.amount)
            .ok_or_else(|| {
                SimpleError::new("Overflow occurred performing transaction on account")
            })?;
        // We only allow the transaction to occur if it is depositing or it leaves the account in
        // the positive
        let zero = Decimal::zero();
        if transaction.amount > zero || new_balance >= zero {
            account.available = new_balance;
            account
                .transaction_history
                .insert(transaction.transaction_id, transaction);
        }
        Ok(())
    }

    fn account(&mut self, client_id: u16) -> &mut Account {
        self.client_accounts
            .entry(client_id)
            .or_insert(Account::new(client_id))
    }
}

pub struct Account {
    pub client_id: u16,
    pub available: Decimal,
    pub held: Decimal,
    pub locked: bool,
    transaction_history: HashMap<u32, Transaction>,
}

impl Account {
    pub fn new(client_id: u16) -> Self {
        Self {
            client_id,
            available: Decimal::zero(),
            held: Decimal::zero(),
            locked: false,
            transaction_history: HashMap::new(),
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn do_not_withdraw_into_negative_amount_and_transaction_is_not_recorded(
    ) -> Result<(), Box<dyn Error>> {
        let mut bank = Bank::new();
        bank.transact(1, Transaction::new(1, Decimal::new(-10, 1)))?;
        assert_eq!(bank.account(1).available, Decimal::zero());
        assert!(bank.account(1).transaction_history.is_empty());
        Ok(())
    }

    #[test]
    fn transaction_for_positive_adds_to_total_and_is_recorded() -> Result<(), Box<dyn Error>> {
        let mut bank = Bank::new();
        let transaction = Transaction::new(2, Decimal::new(10, 1));
        bank.transact(1, transaction.clone())?;
        assert_eq!(bank.account(1).available, Decimal::new(10, 1));
        assert_eq!(
            *bank.account(1).transaction_history.get(&2).unwrap(),
            transaction
        );
        Ok(())
    }

    #[test]
    fn transaction_for_negative_lowers_balance_and_is_recorded() -> Result<(), Box<dyn Error>> {
        let mut bank = Bank::new();
        let transaction1 = Transaction::new(1, Decimal::new(1, 0));
        let transaction2 = Transaction::new(2, Decimal::new(-1, 1));
        bank.transact(1, transaction1.clone())?;
        bank.transact(1, transaction2.clone())?;
        assert_eq!(bank.account(1).available, Decimal::new(9, 1));
        assert_eq!(
            *bank.account(1).transaction_history.get(&1).unwrap(),
            transaction1
        );
        assert_eq!(
            *bank.account(1).transaction_history.get(&2).unwrap(),
            transaction2
        );
        Ok(())
    }

    #[test]
    fn account_deposit_with_huge_numbers_fails_and_is_not_recorded() -> Result<(), Box<dyn Error>> {
        let mut bank = Bank::new();
        let max_decimal = Decimal::max_value();
        bank.transact(1, Transaction::new(1, max_decimal))?;
        assert!(bank.transact(1, Transaction::new(2, max_decimal)).is_err());
        assert_eq!(bank.account(1).transaction_history.len(), 1);
        Ok(())
    }

    #[test]
    fn deposit_to_locked_account_fails_and_is_not_recorded() -> Result<(), Box<dyn Error>> {
        let mut bank = Bank::new();
        bank.account(1).locked = true;
        bank.transact(1, Transaction::new(1, Decimal::new(1, 1)))?;
        assert_eq!(bank.account(1).available, Decimal::zero());
        assert!(bank.account(1).transaction_history.is_empty());
        Ok(())
    }
}
