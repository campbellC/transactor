use std::collections::{HashMap, HashSet};
use std::error::Error;

use rust_decimal::prelude::*;
use simple_error::SimpleError;

#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub struct TransactionId(pub u32);

#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub struct ClientId(pub u16);

#[derive(Debug, Eq, PartialEq, Copy, Clone)]
pub struct Transaction {
    transaction_id: TransactionId,
    amount: Decimal,
}

impl Transaction {
    pub fn new(transaction_id: TransactionId, amount: Decimal) -> Self {
        Self {
            transaction_id,
            amount,
        }
    }
}

#[derive(Debug, Eq, PartialEq, Copy, Clone, Hash)]
pub struct Dispute {
    transaction_id: TransactionId,
}

impl Dispute {
    pub fn new(transaction_id: TransactionId) -> Self {
        Self { transaction_id }
    }
}

pub struct Bank {
    client_accounts: HashMap<ClientId, Account>,
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
        client_id: ClientId,
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

    /// Handle a dispute on a transaction.
    /// If the transaction does not exist this will be ignored.
    /// If the transaction has already been disputed this will be ignored.
    /// This can fail if moving the disputed funds causes an overflow
    pub fn handle_dispute(
        &mut self,
        client_id: ClientId,
        dispute: Dispute,
    ) -> Result<(), Box<dyn Error>> {
        let account = self.account(client_id);
        // Only handle disputes that have not been handled and only if the transaction has been enacted.
        if account.open_disputes.contains(&dispute)
            || !account
                .transaction_history
                .contains_key(&dispute.transaction_id)
        {
            return Ok(());
        }
        let transaction_amount = account.transaction_history[&dispute.transaction_id].amount;
        // no matter if this is a withdrawal or a deposit we need to
        // withhold the absolute value of the funds
        let disputed_amount = transaction_amount.abs();
        Bank::move_funds_from_available_to_held(account, disputed_amount)?;
        self.account(client_id).open_disputes.insert(dispute);
        Ok(())
    }

    fn move_funds_from_available_to_held(
        account: &mut Account,
        amount: Decimal,
    ) -> Result<(), Box<dyn Error>> {
        let new_available = account.available.checked_sub(amount);
        let new_held = account.held.checked_add(amount);
        return if let (Some(available), Some(held)) = (new_available, new_held) {
            account.available = available;
            account.held = held;
            Ok(())
        } else {
            return Err(Box::new(SimpleError::new(
                "Overflow on moving between available and held",
            )));
        };
    }

    fn account(&mut self, client_id: ClientId) -> &mut Account {
        self.client_accounts
            .entry(client_id)
            .or_insert(Account::new(client_id))
    }
}

pub struct Account {
    pub client_id: ClientId,
    pub available: Decimal,
    pub held: Decimal,
    pub locked: bool,
    transaction_history: HashMap<TransactionId, Transaction>,
    open_disputes: HashSet<Dispute>,
}

impl Account {
    pub fn new(client_id: ClientId) -> Self {
        Self {
            client_id,
            available: Decimal::zero(),
            held: Decimal::zero(),
            locked: false,
            transaction_history: HashMap::new(),
            open_disputes: HashSet::new(),
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn do_not_withdraw_into_negative_amount_and_transaction_is_not_recorded(
    ) -> Result<(), Box<dyn Error>> {
        let client = ClientId(1);
        let mut bank = Bank::new();
        bank.transact(
            client,
            Transaction::new(TransactionId(1), Decimal::new(-10, 1)),
        )?;
        assert_eq!(bank.account(client).available, Decimal::zero());
        assert!(bank.account(client).transaction_history.is_empty());
        Ok(())
    }

    #[test]
    fn transaction_for_positive_adds_to_total_and_is_recorded() -> Result<(), Box<dyn Error>> {
        let mut bank = Bank::new();
        let client = ClientId(1);
        let tx = TransactionId(2);
        let transaction = Transaction::new(tx, Decimal::new(10, 1));
        bank.transact(client, transaction.clone())?;
        assert_eq!(bank.account(client).available, Decimal::new(10, 1));
        assert_eq!(
            *bank.account(client).transaction_history.get(&tx).unwrap(),
            transaction
        );
        Ok(())
    }

    #[test]
    fn transaction_for_negative_lowers_balance_and_is_recorded() -> Result<(), Box<dyn Error>> {
        let mut bank = Bank::new();
        let client = ClientId(1);
        let transaction_id1 = TransactionId(1);
        let transaction_id2 = TransactionId(2);
        let transaction1 = Transaction::new(transaction_id1, Decimal::new(1, 0));
        let transaction2 = Transaction::new(transaction_id2, Decimal::new(-1, 1));

        bank.transact(client, transaction1.clone())?;
        bank.transact(client, transaction2.clone())?;

        assert_eq!(bank.account(client).available, Decimal::new(9, 1));
        assert_eq!(
            *bank
                .account(client)
                .transaction_history
                .get(&transaction_id1)
                .unwrap(),
            transaction1
        );
        assert_eq!(
            *bank
                .account(client)
                .transaction_history
                .get(&transaction_id2)
                .unwrap(),
            transaction2
        );
        Ok(())
    }

    #[test]
    fn account_deposit_with_huge_numbers_fails_and_is_not_recorded() -> Result<(), Box<dyn Error>> {
        let mut bank = Bank::new();
        let client = ClientId(1);
        let max_decimal = Decimal::max_value();
        let transaction_id1 = TransactionId(1);
        let transaction_id2 = TransactionId(2);
        bank.transact(client, Transaction::new(transaction_id1, max_decimal))?;
        assert!(bank
            .transact(client, Transaction::new(transaction_id2, max_decimal))
            .is_err());
        assert_eq!(bank.account(client).transaction_history.len(), 1);
        Ok(())
    }

    #[test]
    fn deposit_to_locked_account_fails_and_is_not_recorded() -> Result<(), Box<dyn Error>> {
        let mut bank = Bank::new();
        let client = ClientId(1);
        let transaction_id = TransactionId(1);
        bank.account(client).locked = true;
        bank.transact(client, Transaction::new(transaction_id, Decimal::new(1, 1)))?;
        assert_eq!(bank.account(client).available, Decimal::zero());
        assert!(bank.account(client).transaction_history.is_empty());
        Ok(())
    }

    #[test]
    fn dispute_transaction_ignored_if_transaction_does_not_exist() -> Result<(), Box<dyn Error>> {
        let mut bank = Bank::new();
        let client = ClientId(1);
        bank.handle_dispute(client, Dispute::new(TransactionId(1)))?;

        assert_eq!(bank.account(client).available, Decimal::zero());
        assert_eq!(bank.account(client).held, Decimal::zero());
        assert!(bank.account(client).open_disputes.is_empty());
        Ok(())
    }

    #[test]
    fn dispute_positive_transaction_moves_funds_into_held() -> Result<(), Box<dyn Error>> {
        let mut bank = Bank::new();
        let client = ClientId(1);
        let disputed_amount = Decimal::new(1, 1);
        let transaction_id = TransactionId(1);
        let transaction = Transaction::new(transaction_id, disputed_amount);
        let dispute = Dispute::new(transaction_id);

        bank.transact(client, transaction.clone())?;
        bank.handle_dispute(client, dispute)?;

        assert_eq!(bank.account(client).available, Decimal::zero());
        assert_eq!(bank.account(client).held, disputed_amount);
        assert_eq!(
            *bank
                .account(client)
                .transaction_history
                .get(&transaction_id)
                .unwrap(),
            transaction
        );
        assert!(bank.account(client).open_disputes.contains(&dispute));
        Ok(())
    }

    #[test]
    fn dispute_negative_transaction_moves_funds_into_held() -> Result<(), Box<dyn Error>> {
        let mut bank = Bank::new();
        let client = ClientId(1);
        let disputed_amount = Decimal::new(1, 1);
        bank.account(client).available = disputed_amount;
        let transaction_id = TransactionId(1);
        let transaction = Transaction::new(transaction_id, -disputed_amount);
        let dispute = Dispute::new(transaction_id);

        bank.transact(client, transaction.clone())?;
        assert_eq!(bank.account(client).available, Decimal::zero());
        bank.handle_dispute(client, dispute)?;

        assert_eq!(bank.account(client).available, -disputed_amount);
        assert_eq!(bank.account(client).held, disputed_amount);
        assert_eq!(
            *bank
                .account(client)
                .transaction_history
                .get(&transaction_id)
                .unwrap(),
            transaction
        );
        assert!(bank.account(client).open_disputes.contains(&dispute));
        Ok(())
    }

    #[test]
    fn dispute_transaction_fails_if_causes_overflow_in_held_and_dispute_is_not_recorded(
    ) -> Result<(), Box<dyn Error>> {
        let mut bank = Bank::new();
        let client = ClientId(1);
        let max_value = Decimal::max_value();
        let transaction_id = TransactionId(1);
        let transaction = Transaction::new(transaction_id, max_value);
        let dispute = Dispute::new(transaction_id);
        bank.account(client).held = max_value;

        bank.transact(client, transaction.clone())?;

        assert!(bank.handle_dispute(client, dispute).is_err());
        assert_eq!(bank.account(client).available, max_value);
        assert_eq!(bank.account(client).held, max_value);
        assert_eq!(
            *bank
                .account(client)
                .transaction_history
                .get(&transaction_id)
                .unwrap(),
            transaction
        );
        assert!(bank.account(client).open_disputes.is_empty());
        Ok(())
    }

    #[test]
    fn dispute_transaction_fails_if_causes_overflow_in_available_and_dispute_is_not_recorded(
    ) -> Result<(), Box<dyn Error>> {
        let mut bank = Bank::new();
        let client = ClientId(1);
        let max_value = Decimal::max_value();
        let transaction_id = TransactionId(1);
        let huge_deposit = Transaction::new(transaction_id, max_value);
        let dispute = Dispute::new(transaction_id);

        bank.transact(client, huge_deposit.clone())?;
        bank.account(client).available = -max_value;
        assert!(bank.handle_dispute(client, dispute).is_err());

        assert_eq!(bank.account(client).available, -max_value);
        assert_eq!(bank.account(client).held, Decimal::zero());
        assert_eq!(
            *bank
                .account(client)
                .transaction_history
                .get(&transaction_id)
                .unwrap(),
            huge_deposit
        );
        assert!(bank.account(client).open_disputes.is_empty());
        Ok(())
    }
}
