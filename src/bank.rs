use std::collections::{HashMap, HashSet};

use crate::error::{TransactorError, TransactorError::*};
use rust_decimal::prelude::*;

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

pub struct Account {
    pub client_id: ClientId,
    pub available: Decimal,
    pub held: Decimal,
    pub locked: bool,
    transaction_history: HashMap<TransactionId, Transaction>,
    disputed_transactions: HashSet<TransactionId>,
}

impl Account {
    pub fn new(client_id: ClientId) -> Self {
        Self {
            client_id,
            available: Decimal::zero(),
            held: Decimal::zero(),
            locked: false,
            transaction_history: HashMap::new(),
            disputed_transactions: HashSet::new(),
        }
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
    ) -> Result<(), TransactorError> {
        let account = self.account(client_id);

        if account.locked {
            return Ok(());
        }

        if account
            .transaction_history
            .contains_key(&transaction.transaction_id)
        {
            return Err(TransactionIdReuse);
        }

        let new_balance = account
            .available
            .checked_add(transaction.amount)
            .ok_or_else(|| Overflow)?;
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
    pub fn dispute_transaction(
        &mut self,
        client_id: ClientId,
        dispute: TransactionId,
    ) -> Result<(), TransactorError> {
        let account = self.account(client_id);
        // Only handle disputes that have not been handled and only if the transaction has been enacted.
        if account.disputed_transactions.contains(&dispute)
            || !account.transaction_history.contains_key(&dispute)
        {
            return Ok(());
        }
        let transaction_amount = account.transaction_history[&dispute].amount;
        // no matter if this is a withdrawal or a deposit we need to
        // withhold the absolute value of the funds
        let disputed_amount = transaction_amount.abs();
        Bank::move_funds_from_available_to_held(account, disputed_amount)?;
        account.disputed_transactions.insert(dispute);
        Ok(())
    }

    /// Resolve a previously disputed transaction
    /// If the transaction does not exist, or this transaction was never
    /// previously disputed this will be ignored.
    /// This can fail if moving the disputed funds causes an overflow
    pub fn resolve_disputed_transaction(
        &mut self,
        client_id: ClientId,
        disputed_transaction: TransactionId,
    ) -> Result<(), TransactorError> {
        let account = self.account(client_id);
        // Only handle disputes that have been made already and only if the transaction has been enacted.
        if !account
            .disputed_transactions
            .contains(&disputed_transaction)
            || !account
                .transaction_history
                .contains_key(&disputed_transaction)
        {
            return Ok(());
        }
        let transaction_amount = account.transaction_history[&disputed_transaction].amount;
        // no matter if this is a withdrawal or a deposit we need to
        // move the funds from held into available
        let disputed_amount = -transaction_amount.abs();
        Bank::move_funds_from_available_to_held(account, disputed_amount)?;
        account.disputed_transactions.remove(&disputed_transaction);
        Ok(())
    }

    /// Chargeback a disputed transaction
    /// If the transaction does not exist, or this transaction was never
    /// previously disputed this will be ignored.
    /// This can fail if removing the funds causes overflow.
    pub fn chargeback(
        &mut self,
        client_id: ClientId,
        disputed_transaction: TransactionId,
    ) -> Result<(), TransactorError> {
        let account = self.account(client_id);
        // Only handle disputes that have been made already and only if the transaction has been enacted.
        if !account
            .disputed_transactions
            .contains(&disputed_transaction)
            || !account
                .transaction_history
                .contains_key(&disputed_transaction)
        {
            return Ok(());
        }
        let transaction_amount = account.transaction_history[&disputed_transaction].amount;
        let disputed_amount = transaction_amount.abs();
        account.held = account
            .held
            .checked_sub(disputed_amount)
            .ok_or_else(|| Overflow)?;
        account.locked = true;
        account.disputed_transactions.remove(&disputed_transaction);
        Ok(())
    }

    fn move_funds_from_available_to_held(
        account: &mut Account,
        amount: Decimal,
    ) -> Result<(), TransactorError> {
        let new_available = account.available.checked_sub(amount);
        let new_held = account.held.checked_add(amount);
        return if let (Some(available), Some(held)) = (new_available, new_held) {
            account.available = available;
            account.held = held;
            Ok(())
        } else {
            return Err(Overflow);
        };
    }

    fn account(&mut self, client_id: ClientId) -> &mut Account {
        self.client_accounts
            .entry(client_id)
            .or_insert(Account::new(client_id))
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn do_not_withdraw_into_negative_amount_and_transaction_is_not_recorded(
    ) -> Result<(), TransactorError> {
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
    fn transaction_for_positive_adds_to_total_and_is_recorded() -> Result<(), TransactorError> {
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
    fn transaction_for_negative_lowers_balance_and_is_recorded() -> Result<(), TransactorError> {
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
    fn account_deposit_with_huge_numbers_fails_and_is_not_recorded() -> Result<(), TransactorError>
    {
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
    fn deposit_to_locked_account_is_ignored_and_is_not_recorded() -> Result<(), TransactorError> {
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
    fn dispute_transaction_ignored_if_transaction_does_not_exist() -> Result<(), TransactorError> {
        let mut bank = Bank::new();
        let client = ClientId(1);
        bank.dispute_transaction(client, TransactionId(1))?;

        assert_eq!(bank.account(client).available, Decimal::zero());
        assert_eq!(bank.account(client).held, Decimal::zero());
        assert!(bank.account(client).disputed_transactions.is_empty());
        Ok(())
    }

    #[test]
    fn dispute_positive_transaction_moves_funds_into_held() -> Result<(), TransactorError> {
        let mut bank = Bank::new();
        let client = ClientId(1);
        let disputed_amount = Decimal::new(1, 1);
        let transaction_id = TransactionId(1);
        let transaction = Transaction::new(transaction_id, disputed_amount);

        bank.transact(client, transaction.clone())?;
        bank.dispute_transaction(client, transaction_id)?;

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
        assert!(bank
            .account(client)
            .disputed_transactions
            .contains(&transaction_id));
        Ok(())
    }

    #[test]
    fn dispute_negative_transaction_moves_funds_into_held() -> Result<(), TransactorError> {
        let mut bank = Bank::new();
        let client = ClientId(1);
        let disputed_amount = Decimal::new(1, 1);
        bank.account(client).available = disputed_amount;
        let transaction_id = TransactionId(1);
        let transaction = Transaction::new(transaction_id, -disputed_amount);

        bank.transact(client, transaction.clone())?;
        assert_eq!(bank.account(client).available, Decimal::zero());
        bank.dispute_transaction(client, transaction_id)?;

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
        assert!(bank
            .account(client)
            .disputed_transactions
            .contains(&transaction_id));
        Ok(())
    }

    #[test]
    fn dispute_transaction_fails_if_causes_overflow_in_held_and_dispute_is_not_recorded(
    ) -> Result<(), TransactorError> {
        let mut bank = Bank::new();
        let client = ClientId(1);
        let max_value = Decimal::max_value();
        let transaction_id = TransactionId(1);
        let transaction = Transaction::new(transaction_id, max_value);
        bank.account(client).held = max_value;

        bank.transact(client, transaction.clone())?;

        assert!(bank.dispute_transaction(client, transaction_id).is_err());
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
        assert!(bank.account(client).disputed_transactions.is_empty());
        Ok(())
    }

    #[test]
    fn dispute_transaction_fails_if_causes_overflow_in_available_and_dispute_is_not_recorded(
    ) -> Result<(), TransactorError> {
        let mut bank = Bank::new();
        let client = ClientId(1);
        let max_value = Decimal::max_value();
        let transaction_id = TransactionId(1);
        let huge_deposit = Transaction::new(transaction_id, max_value);

        bank.transact(client, huge_deposit.clone())?;
        bank.account(client).available = -max_value;
        assert!(bank.dispute_transaction(client, transaction_id).is_err());

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
        assert!(bank.account(client).disputed_transactions.is_empty());
        Ok(())
    }

    #[test]
    fn resolve_dispute_fails_if_causes_overflow_in_available_and_dispute_is_not_resolved(
    ) -> Result<(), TransactorError> {
        let mut bank = Bank::new();
        let client = ClientId(1);
        let max_value = Decimal::max_value();
        let transaction_id1 = TransactionId(1);
        let huge_deposit = Transaction::new(transaction_id1, max_value);
        let transaction_id2 = TransactionId(2);
        let huge_deposit2 = Transaction::new(transaction_id2, max_value);

        bank.transact(client, huge_deposit.clone())?;
        bank.dispute_transaction(client, transaction_id1)?;
        bank.transact(client, huge_deposit2.clone())?;

        assert!(bank
            .resolve_disputed_transaction(client, transaction_id1)
            .is_err());
        assert_eq!(bank.account(client).available, max_value);
        assert_eq!(bank.account(client).held, max_value);
        assert_eq!(
            *bank
                .account(client)
                .transaction_history
                .get(&transaction_id1)
                .unwrap(),
            huge_deposit
        );
        assert_eq!(
            *bank
                .account(client)
                .transaction_history
                .get(&transaction_id2)
                .unwrap(),
            huge_deposit2
        );
        assert!(bank
            .account(client)
            .disputed_transactions
            .contains(&transaction_id1));
        Ok(())
    }

    #[test]
    fn resolve_dispute_ignores_if_transaction_does_not_exist() -> Result<(), TransactorError> {
        let mut bank = Bank::new();
        let client = ClientId(1);
        bank.resolve_disputed_transaction(client, TransactionId(1))?;

        assert_eq!(bank.account(client).available, Decimal::zero());
        assert_eq!(bank.account(client).held, Decimal::zero());
        assert!(bank.account(client).disputed_transactions.is_empty());
        Ok(())
    }

    #[test]
    fn resolve_dispute_ignores_if_transaction_is_not_disputed() -> Result<(), TransactorError> {
        let mut bank = Bank::new();
        let client = ClientId(1);
        let amount = Decimal::max_value();
        let transaction_id = TransactionId(1);
        let deposit = Transaction::new(transaction_id, amount);

        bank.transact(client, deposit.clone())?;
        bank.resolve_disputed_transaction(client, transaction_id)?;

        assert_eq!(bank.account(client).available, amount);
        assert_eq!(bank.account(client).held, Decimal::zero());
        assert!(bank.account(client).disputed_transactions.is_empty());
        Ok(())
    }

    #[test]
    fn resolve_dispute_correctly_resolves_disputed_withdrawal() -> Result<(), TransactorError> {
        let mut bank = Bank::new();
        let client = ClientId(1);
        let amount = Decimal::max_value();
        let transaction_id = TransactionId(1);
        let withdrawal = Transaction::new(transaction_id, -amount);

        bank.account(client).available = amount;
        bank.transact(client, withdrawal.clone())?;
        bank.dispute_transaction(client, transaction_id.clone())?;
        bank.resolve_disputed_transaction(client, transaction_id)?;

        assert_eq!(bank.account(client).available, Decimal::zero());
        assert_eq!(bank.account(client).held, Decimal::zero());
        assert_eq!(
            *bank
                .account(client)
                .transaction_history
                .get(&transaction_id)
                .unwrap(),
            withdrawal
        );
        assert!(bank.account(client).disputed_transactions.is_empty());
        Ok(())
    }

    #[test]
    fn resolve_dispute_correctly_resolves_disputed_transaction() -> Result<(), TransactorError> {
        let mut bank = Bank::new();
        let client = ClientId(1);
        let amount = Decimal::max_value();
        let transaction_id = TransactionId(1);
        let deposit = Transaction::new(transaction_id, amount);

        bank.transact(client, deposit.clone())?;
        bank.dispute_transaction(client, transaction_id.clone())?;
        bank.resolve_disputed_transaction(client, transaction_id)?;

        assert_eq!(bank.account(client).available, amount);
        assert_eq!(bank.account(client).held, Decimal::zero());
        assert_eq!(
            *bank
                .account(client)
                .transaction_history
                .get(&transaction_id)
                .unwrap(),
            deposit
        );
        assert!(bank.account(client).disputed_transactions.is_empty());
        Ok(())
    }

    #[test]
    fn chargeback_correctly_pulls_back_disputed_transaction() -> Result<(), TransactorError> {
        let mut bank = Bank::new();
        let client = ClientId(1);
        let amount = Decimal::max_value();
        let transaction_id = TransactionId(1);
        let deposit = Transaction::new(transaction_id, amount);

        bank.transact(client, deposit.clone())?;
        bank.dispute_transaction(client, transaction_id.clone())?;
        bank.chargeback(client, transaction_id)?;

        assert_eq!(bank.account(client).available, Decimal::zero());
        assert_eq!(bank.account(client).held, Decimal::zero());
        assert_eq!(
            *bank
                .account(client)
                .transaction_history
                .get(&transaction_id)
                .unwrap(),
            deposit
        );
        assert!(bank.account(client).disputed_transactions.is_empty());
        assert!(bank.account(client).locked);
        Ok(())
    }

    #[test]
    fn chargeback_correctly_ignored_if_transaction_not_disputed() -> Result<(), TransactorError> {
        let mut bank = Bank::new();
        let client = ClientId(1);
        let amount = Decimal::max_value();
        let transaction_id = TransactionId(1);
        let deposit = Transaction::new(transaction_id, amount);

        bank.transact(client, deposit.clone())?;
        bank.chargeback(client, transaction_id)?;

        assert_eq!(bank.account(client).available, amount);
        assert_eq!(bank.account(client).held, Decimal::zero());
        assert_eq!(
            *bank
                .account(client)
                .transaction_history
                .get(&transaction_id)
                .unwrap(),
            deposit
        );
        assert!(bank.account(client).disputed_transactions.is_empty());
        assert!(!bank.account(client).locked);
        Ok(())
    }
}
