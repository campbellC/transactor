## Dependencies
I am using csv and serde for reading and writing from csv files as suggested. For the decimal number handling 
I have gone with rust_decimal because it handles fixed precision calculations according to its documentation - 
I have not worked with this crate before. For errors I've used thiserror - even though this crate is more appropriate 
for a library I imagine this code would be probably librarified at some point so it felt a fair choice.

## Testing
I have provided two approaches to testing - end-to-end and unit testing. Since this is to be used as a cli tool 
I have provided a bash script to test against specific controlled input files and expected output files. These tests are 
slightly brittle in that I'm using `diff` to validate the outputs of two csv files but it will do for now. The second 
set of tests are more usual cargo testing in the project itself which witness specific functionality of components.

## Efficiency
* All data to be streamed in (out makes no sense because we need the final state before writing the file)
* TODO: Look into reusing memory for records that are read in (less sure there's scope here)
* TODO: Since clients do not interact we can shard based on client id for multi-threading

## Edge cases
* Deposit, Withdrawal, Dispute on Deposit - handled
* Rounding errors on deposits and withdrawals - handled by rust_decimal
* Negative deposits - handled
* Negative Withdrawals - handled
* Massive deposits that sum to overflow - handled
* Massive withdrawals - handled
* Multiple resolutions on same transaction -handled
* Multiple resolves on same transaction - handled
* Multiply disputed transaction - handled  
* Disputes and resolutions/chargebacks after account lock - not handled - I have run out of time for this
