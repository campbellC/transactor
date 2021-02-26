## Assumptions

Since this program creates accounts from scratch and does not persist them between calls, I have assumed that any errors
that happen whatsoever should cause program exit with status 1. In a real setting this would probably not be ideal
because the accounts may be in an incorrect state (for example, a disputed transaction caused an overflow but it was
genuinely disputed) but given the constraints I think it makes sense. This applies to:

* A deposit with a negative amount/a withdrawal with a positive amount
* A dispute/resolve/chargeback with an amount given
* Reuse of a transaction id for a given client id (see edge cases - I do not handle reuse of transaction id across
  different clients)
* Overflow during calculation
* CSV parsing errors/IO errors

## Dependencies

I am using csv and serde for reading and writing from csv files as suggested. For the decimal number handling I have
gone with rust_decimal because it handles fixed precision calculations according to its documentation - I have not
worked with this crate before. For errors I've used thiserror - even though this crate is more appropriate for a library
I imagine this code would be probably librarified at some point so it felt a fair choice.

## Testing

I have provided two approaches to testing - end-to-end and unit testing. Since this is to be used as a cli tool I have
provided a bash script to test against specific controlled input files and expected output files. These tests are
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
* Reuse of transaction ID across different client ids - not handled - I'm not sure it makes sense to fail in this case
  but it probably would be better handled by another system. I decided not to add this because it would need to be an
  explicit check that added either memory or time complexity 