use argh::FromArgs;
use csv::{Reader, ReaderBuilder, Trim, Writer};
use rust_decimal::prelude::*;
use serde::{Serialize, Deserialize};
use std::error::Error;
use std::fs::File;

mod bank;

use bank::{Bank, TransactionType};

#[derive(FromArgs)]
/// A program for enacting a CSV files of transactions over multiple accounts
struct Arguments {
    #[argh(positional)]
    /// A csv file of transactions. Nb: the filename must be UTF-8 encoded
    input_file: String,
}

fn main() {
    let arguments: Arguments = argh::from_env();
    let reader = ReaderBuilder::new()
        .trim(Trim::All)
        .from_path(arguments.input_file);
    std::process::exit(match reader {
        Ok(f) => {
            if let Err(e) = enact_transactions(f) {
                eprintln!("Failed to enact transactions {}", e);
                1
            } else {
                0
            }
        }
        Err(e) => {
            eprintln!("Failed to open given file {}", e);
            1
        }
    })
}

#[derive(Debug, Deserialize)]
struct TransactionRecord {
    r#type: TransactionType,
    client: u16,
    tx: u32,
    amount: Decimal,
}

#[derive(Debug, Serialize)]
struct AccountRecord {
    client: u16,
    available: Decimal,
    held: Decimal,
    total: Decimal,
    locked: bool,
}

fn enact_transactions(mut reader: Reader<File>) -> Result<(), Box<dyn Error>> {
    let mut bank: Bank = Bank::new();
    for result in reader.deserialize() {
        let record: TransactionRecord = result?;
        bank.transact(record.r#type, record.client, record.tx, record.amount)?;
    }
    let mut writer = Writer::from_writer(std::io::stdout());
    for account in bank.get_accounts() {
        writer.serialize(AccountRecord {
            client: account.client_id,
            available: account.balance,
            held: Decimal::zero(),
            total: account.balance + Decimal::zero(),
            locked: false,
        })?;
    }
    Ok(())
}
