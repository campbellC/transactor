## Requirements



## Dependencies
I am using csv and serde for reading and writing from csv files as suggested. For the decimal number handling 
I have gone with rust_decimal because it handles fixed precision calculations according to its documentation - 
I have not worked with this crate before.

## Testing
I have provided two approaches to testing - end-to-end and unit testing. Since this is to be used as a cli tool 
I have provided a bash script to test against specific controlled input files and expected output files. These tests are 
slightly brittle in that I'm using `diff` to validate the outputs of two csv files but it will do for now. The second 
set of tests are more usual cargo testing in the project itself which witness specific functionality of components.

## Efficiency
* All data to be streamed in (out makes no sense because we need the final state before writing the file)
* TODO: Since clients do not interact we can shard based on client id for multi-threading

## Edge cases
* Deposit, Withdrawal, Dispute on Deposit
* Rounding errors on deposits and withdrawals
* Negative deposits
* Negative Withdrawals
* Massive deposits that sum to overflow
* Massive withdrawals
* Multiple resolutions on same transaction
* Multiple resolves on same transaction
* Resolution on transaction after resolves