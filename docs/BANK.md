# Bank loans

Open **Bank** in the top bar to borrow money for equipment. The quote shows
cash received, fixed interest, total repayment, term and payment schedule before
the **Borrow** action. The game allows one active loan at a time.

Default offers accept $500–$25,000 in $100 increments:

| Term in game time | Payments | Fixed interest |
| --- | --- | --- |
| 30 minutes | 6 | 5% |
| 60 minutes | 12 | 10% |
| 120 minutes | 24 | 18% |

All payments are five minutes apart. Interest is a fixed fee charged once when
borrowing, rather than an annual rate or compounding interest. Repaying early
does not remove that fee. For example, a $5,000, 60-minute loan adds $5,000 cash
and creates $5,500 debt. Its first payment is $459. Cumulative integer rounding
makes the following payments differ by at most $1, and the total is exactly $5,500.

At a deadline, the bank automatically collects the amount due from cash. If cash
runs out, the balance stops at zero and the unpaid amount remains overdue. It is
shown in red in the bank and top bar. Later cash, including equipment sale
proceeds, pays overdue amounts first. Missing a payment does not cancel debt,
create an overdraft, or charge extra late fees. There is no equipment seizure.
New credit remains unavailable until the current loan is fully repaid.

The active-loan view shows remaining debt, paid amount, the next unpaid
installment and its countdown. **Repay amount** makes a partial early payment;
**Repay all** settles the current balance. Early payments cover the oldest
installments first, so paying ahead moves the next unpaid deadline. Payment
buttons are disabled when the cash balance is insufficient. The bank records
the last eight completed loans.

This feature does not add income contracts or passive revenue. Repayment uses
the game's existing cash balance and equipment sale proceeds.

## Configuration

`assets/equipment/bank.json` owns the offers:

- `minimum_loan`, `maximum_loan`, `amount_step`: allowed principal amounts.
- `payment_interval_minutes`: interval in game minutes.
- `terms[].id`: stable offer ID used by borrowing commands.
- `terms[].installments`: number of scheduled payments.
- `terms[].interest_bps`: fixed interest in basis points; `1000` means 10%.

The catalog loads once, using installed assets or embedded defaults. Restart to
apply catalog changes. Bounds, increments, term IDs, payment counts and interest
are validated. Existing contracts keep their original interest and schedule
when the catalog changes.

## Domain and saves

`Command::Bank(BankCommand::Borrow { .. })` validates the quote and credits cash
atomically. `BankCommand::Repay` names the loan ID, preventing a stale button
from paying a later loan. Invalid or unaffordable commands preserve cash and debt.
The UI dispatches commands to the authoritative simulation worker.

Loan time uses `NetworkSim::advance_time`, with a separate saved bank clock;
the transient packet clock can reset without resetting deadlines. Closing the
game pauses loan time. Saving and loading preserve cash, paid amounts, overdue
debt and deadlines. Loading an earlier save restores its economy consistently,
just like equipment purchases. Old saves default to an empty bank account and
keep their existing cash.

Regression tests cover loan issuance, exact rounding, automatic/early payments,
shortages, sale proceeds, blocked additional loans, stale payment IDs, overflow,
configuration, time-step equivalence and save compatibility.
