use argh::FromArgs;
use csv::{ReaderBuilder, Trim, Writer};
use rust_decimal::prelude::*;
use serde::{Deserialize, Serialize};

mod bank;
mod error;

use crate::bank::{Bank, ClientId, Transaction, TransactionId};
use crate::error::TransactorError;
use crate::error::TransactorError::*;

#[derive(FromArgs)]
/// A program for enacting a CSV files of transactions over multiple accounts
struct Arguments {
    #[argh(positional)]
    /// A csv file of transactions. Nb: the filename must be UTF-8 encoded
    input_file: String,
}

fn main() {
    let arguments: Arguments = argh::from_env();
    std::process::exit(match enact_transactions(arguments.input_file) {
        Ok(_) => 0,
        Err(e) => {
            eprintln!("Failed to handle given file {}", e);
            1
        }
    })
}

#[derive(Debug, Deserialize)]
struct TransactionRecord {
    r#type: TransactionRecordType,
    client: u16,
    tx: u32,
    amount: Option<Decimal>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "lowercase")]
enum TransactionRecordType {
    DEPOSIT,
    WITHDRAWAL,
    DISPUTE,
    RESOLVE,
    CHARGEBACK,
}

#[derive(Debug, Serialize)]
struct AccountRecord {
    client: u16,
    available: Decimal,
    held: Decimal,
    total: Decimal,
    locked: bool,
}

fn enact_transactions(filename: String) -> Result<(), TransactorError> {
    let mut reader = ReaderBuilder::new().trim(Trim::All).from_path(filename)?;
    let mut bank: Bank = Bank::new();
    for result in reader.deserialize() {
        let record: TransactionRecord = result?;
        match record.r#type {
            TransactionRecordType::DEPOSIT => {
                let amount = record.amount.ok_or_else(missing_data)?;
                if amount < Decimal::zero() {
                    return Err(InvalidData(
                        "Deposit of negative amount attempted".to_string(),
                    ));
                } else {
                    bank.transact(
                        ClientId(record.client),
                        Transaction::new(TransactionId(record.tx), amount),
                    )?;
                }
            }
            TransactionRecordType::WITHDRAWAL => {
                let amount = record.amount.ok_or_else(missing_data)?;
                if amount < Decimal::zero() {
                    return Err(InvalidData(
                        "Withdrawal of a negative amount attempted".to_string(),
                    ));
                } else {
                    bank.transact(
                        ClientId(record.client),
                        Transaction::new(TransactionId(record.tx), -amount),
                    )?;
                }
            }
            TransactionRecordType::DISPUTE => {
                let (client, transaction) = parse_dispute_type_record(record)?;
                bank.dispute_transaction(client, transaction)?;
            }
            TransactionRecordType::RESOLVE => {
                let (client, transaction) = parse_dispute_type_record(record)?;
                bank.resolve_disputed_transaction(client, transaction)?;
            }
            TransactionRecordType::CHARGEBACK => {
                let (client, transaction) = parse_dispute_type_record(record)?;
                bank.chargeback(client, transaction)?;
            }
        }
    }
    let mut writer = Writer::from_writer(std::io::stdout());
    for account in bank.get_accounts() {
        writer.serialize(AccountRecord {
            client: account.client_id.0,
            available: account.available.round_dp(4).normalize(),
            held: account.held.round_dp(4).normalize(),
            total: account
                .available
                .checked_add(account.held)
                .ok_or_else(|| Overflow)?
                .round_dp(4)
                .normalize(),
            locked: account.locked,
        })?;
    }
    Ok(())
}

fn parse_dispute_type_record(
    record: TransactionRecord,
) -> Result<(ClientId, TransactionId), TransactorError> {
    if record.amount.is_some() {
        return Err(InvalidData(
            "Found amount in non-transaction type record".to_string(),
        ));
    } else {
        Ok((ClientId(record.client), TransactionId(record.tx)))
    }
}

fn missing_data() -> TransactorError {
    InvalidData("Missing field in input".to_string())
}
