use argh::FromArgs;
use csv::{ReaderBuilder, Trim, Writer};
use rust_decimal::prelude::*;
use serde::{Deserialize, Serialize};
use std::error::Error;

mod bank;

use crate::bank::{Bank, ClientId, Dispute, Transaction, TransactionId};
use simple_error::SimpleError;

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
}

#[derive(Debug, Serialize)]
struct AccountRecord {
    client: u16,
    available: Decimal,
    held: Decimal,
    total: Decimal,
    locked: bool,
}

fn enact_transactions(filename: String) -> Result<(), Box<dyn Error>> {
    let mut reader = ReaderBuilder::new().trim(Trim::All).from_path(filename)?;
    let mut bank: Bank = Bank::new();
    for result in reader.deserialize() {
        let transaction: TransactionRecord = result?;
        match transaction.r#type {
            TransactionRecordType::DEPOSIT => {
                let amount = transaction.amount.ok_or_else(missing_data)?;
                if amount < Decimal::zero() {
                    return Err(invalid_data());
                } else {
                    bank.transact(
                        ClientId(transaction.client),
                        Transaction::new(TransactionId(transaction.tx), amount),
                    )?;
                }
            }
            TransactionRecordType::WITHDRAWAL => {
                let amount = transaction.amount.ok_or_else(missing_data)?;
                if amount < Decimal::zero() {
                    return Err(invalid_data());
                } else {
                    bank.transact(
                        ClientId(transaction.client),
                        Transaction::new(TransactionId(transaction.tx), -amount),
                    )?;
                }
            }
            TransactionRecordType::DISPUTE => {
                if transaction.amount.is_some() {
                    return Err(invalid_data());
                }
                bank.handle_dispute(
                    ClientId(transaction.client),
                    Dispute::new(TransactionId(transaction.tx)),
                )?;
            }
        }
    }
    let mut writer = Writer::from_writer(std::io::stdout());
    for account in bank.get_accounts() {
        writer.serialize(AccountRecord {
            client: account.client_id.0,
            available: account.available,
            held: account.held,
            total: account.available.checked_add(account.held).ok_or_else(|| {
                SimpleError::new("Overflow calculating total funds associated to account")
            })?,
            locked: account.locked,
        })?;
    }
    Ok(())
}

fn missing_data() -> SimpleError {
    SimpleError::new("Missing data in record")
}

fn invalid_data() -> Box<dyn Error> {
    Box::new(SimpleError::new("Invalid data passed into program"))
}
