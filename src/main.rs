use argh::FromArgs;
use csv::{ReaderBuilder, Trim, Writer};
use rust_decimal::prelude::*;
use serde::{Serialize, Deserialize};
use std::error::Error;

mod bank;

use simple_error::SimpleError;
use crate::bank::{Bank, Transaction};

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
        Ok(_) => {
                0
        }
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

#[derive(Debug,Deserialize)]
#[serde(rename_all = "lowercase")]
enum TransactionRecordType {
    DEPOSIT,
    WITHDRAWAL,
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
    let mut reader = ReaderBuilder::new()
        .trim(Trim::All)
        .from_path(filename)?;
    let mut bank: Bank = Bank::new();
    for result in reader.deserialize() {
        let transaction: TransactionRecord = result?;
        match transaction.r#type {
            TransactionRecordType::DEPOSIT  => {
                let amount = transaction.amount.ok_or_else(missing_data)?;
                if amount < Decimal::zero() {
                    return Err(invalid_data());
                } else {
                    bank.transact(transaction.client, Transaction::new(transaction.tx, amount))?;
                }
            },
            TransactionRecordType::WITHDRAWAL => {
                    let amount = transaction.amount.ok_or_else(missing_data)?;
                    if amount < Decimal::zero() {
                        return Err(invalid_data());
                    } else {
                        bank.transact(transaction.client, Transaction::new(transaction.tx, -amount))?;
                    }
            },
        }
    }
    let mut writer = Writer::from_writer(std::io::stdout());
    for account in bank.get_accounts() {
        writer.serialize(AccountRecord {
            client: account.client_id,
            available: account.available,
            held: Decimal::zero(),
            total: account.available + Decimal::zero(),
            locked: false,
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
