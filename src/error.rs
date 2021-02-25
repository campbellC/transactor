use thiserror::Error;

#[derive(Error, Debug)]
pub enum TransactorError {
    #[error("Overflow handling transaction")]
    Overflow,
    #[error("Invalid data: {0}")]
    InvalidData(String),
    #[error("Two transactions attempted with the same id")]
    TransactionIdReuse,
    #[error("CSV parsing error")]
    CsvError(#[from] csv::Error),
}
