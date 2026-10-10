//! Saved loan contracts and repayment schedules use game time and integer dollars.
use crate::{NetworkSim, SimError, SimEvent};
use serde::{Deserialize, Serialize};
use std::sync::OnceLock;

#[derive(Debug, Clone, Deserialize)]
pub struct BankCatalog {
    pub minimum_loan: i64,
    pub maximum_loan: i64,
    pub amount_step: i64,
    pub payment_interval_minutes: u32,
    pub terms: Vec<LoanTerm>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct LoanTerm {
    pub id: String,
    pub installments: u32,
    /// Fixed, one-time interest, in hundredths of a percent.
    pub interest_bps: u32,
}

impl BankCatalog {
    pub fn validate(&self) -> bool {
        self.minimum_loan > 0
            && self.maximum_loan >= self.minimum_loan
            && self.maximum_loan <= 1_000_000_000
            && self.amount_step > 0
            && self.minimum_loan % self.amount_step == 0
            && self.maximum_loan % self.amount_step == 0
            && (1..=1440).contains(&self.payment_interval_minutes)
            && !self.terms.is_empty()
            && self.terms.len() <= 16
            && self.terms.iter().enumerate().all(|(index, term)| {
                !term.id.is_empty()
                    && (1..=1000).contains(&term.installments)
                    && i64::from(term.installments) <= self.minimum_loan
                    && term.interest_bps <= 10_000
                    && self.terms[..index].iter().all(|other| other.id != term.id)
            })
    }

    pub fn quote(&self, amount: i64, term_id: &str) -> Result<LoanQuote, BankError> {
        if !self.validate()
            || amount < self.minimum_loan
            || amount > self.maximum_loan
            || amount % self.amount_step != 0
        {
            return Err(BankError::InvalidAmount);
        }
        let term = self
            .terms
            .iter()
            .find(|term| term.id == term_id)
            .ok_or(BankError::UnknownTerm)?;
        let interest = (i128::from(amount) * i128::from(term.interest_bps) + 9999) / 10_000;
        Ok(LoanQuote {
            principal: amount,
            interest: interest as i64,
            term_id: term.id.clone(),
            installments: term.installments,
            interval_ms: u64::from(self.payment_interval_minutes) * 60_000,
        })
    }
}

pub fn bank_catalog() -> &'static BankCatalog {
    static CATALOG: OnceLock<BankCatalog> = OnceLock::new();
    CATALOG.get_or_init(|| {
        let catalog: BankCatalog = crate::equipment_config::equipment_catalog(
            "bank.json",
            include_str!("../../../assets/equipment/bank.json"),
        );
        assert!(catalog.validate(), "invalid bank configuration");
        catalog
    })
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum BankCommand {
    Borrow {
        amount: i64,
        term_id: String,
    },
    /// None settles the current balance; Some makes an exact partial payment.
    Repay {
        loan_id: u64,
        amount: Option<i64>,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum BankError {
    #[error("loan amount is outside the configured range or increment")]
    InvalidAmount,
    #[error("unknown loan term")]
    UnknownTerm,
    #[error("repay your current loan before borrowing again")]
    ActiveLoan,
    #[error("this loan is no longer active")]
    NoActiveLoan,
    #[error("repayment must be positive and no greater than the remaining balance")]
    InvalidRepayment,
    #[error("bank account capacity exceeded")]
    Capacity,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LoanQuote {
    pub principal: i64,
    pub interest: i64,
    pub term_id: String,
    pub installments: u32,
    pub interval_ms: u64,
}

impl LoanQuote {
    pub fn total(&self) -> i64 {
        self.principal + self.interest
    }
    pub fn duration_ms(&self) -> u64 {
        self.interval_ms * u64::from(self.installments)
    }
    pub fn first_payment(&self) -> i64 {
        self.cumulative_payment(1)
    }
    fn cumulative_payment(&self, index: u32) -> i64 {
        let n = i128::from(self.installments);
        ((i128::from(self.total()) * i128::from(index) + n - 1) / n) as i64
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BankLoan {
    pub id: u64,
    pub quote: LoanQuote,
    pub opened_at_ms: u64,
    pub repaid: i64,
    pub closed_at_ms: Option<u64>,
}

impl BankLoan {
    pub fn remaining(&self) -> i64 {
        self.quote.total() - self.repaid
    }

    pub fn overdue(&self, now_ms: u64) -> i64 {
        let index = (now_ms.saturating_sub(self.opened_at_ms) / self.quote.interval_ms)
            .min(u64::from(self.quote.installments)) as u32;
        (self.quote.cumulative_payment(index) - self.repaid).max(0)
    }

    /// Early payments cover the oldest installments first and move the next deadline.
    pub fn next_payment(&self) -> Option<(u64, i64)> {
        if self.remaining() == 0 {
            return None;
        }
        let index = (i128::from(self.repaid) * i128::from(self.quote.installments)
            / i128::from(self.quote.total())
            + 1) as u32;
        Some((
            self.opened_at_ms + u64::from(index) * self.quote.interval_ms,
            self.quote.cumulative_payment(index) - self.repaid,
        ))
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BankState {
    elapsed_ms: u64,
    next_loan_id: u64,
    active: Option<BankLoan>,
    history: Vec<BankLoan>,
}

impl Default for BankState {
    fn default() -> Self {
        Self {
            elapsed_ms: 0,
            next_loan_id: 1,
            active: None,
            history: Vec::new(),
        }
    }
}

impl BankState {
    pub fn elapsed_ms(&self) -> u64 {
        self.elapsed_ms
    }
    pub fn active(&self) -> Option<&BankLoan> {
        self.active.as_ref()
    }
    pub fn history(&self) -> &[BankLoan] {
        &self.history
    }
    pub fn overdue(&self) -> i64 {
        self.active
            .as_ref()
            .map_or(0, |loan| loan.overdue(self.elapsed_ms))
    }

    pub(crate) fn advance(&mut self, ms: u64, money: &mut i64) {
        let available_since = self.elapsed_ms;
        self.elapsed_ms = self.elapsed_ms.saturating_add(ms);
        self.collect_due_since(money, available_since);
    }

    pub(crate) fn collect_due(&mut self, money: &mut i64) {
        self.collect_due_since(money, self.elapsed_ms);
    }

    fn collect_due_since(&mut self, money: &mut i64, available_since: u64) {
        let payment = self.overdue().min((*money).max(0));
        let mut paid_at = self.elapsed_ms;
        if let Some(loan) = &mut self.active {
            loan.repaid += payment;
            *money -= payment;
            // Cash existed throughout this time step. Record the actual final
            // deadline, or the time late funds became available, rather than
            // the end of a potentially much longer step.
            paid_at = available_since.max(loan.opened_at_ms + loan.quote.duration_ms());
        }
        self.finish_paid_loan(paid_at);
    }

    fn finish_paid_loan(&mut self, paid_at: u64) {
        if self
            .active
            .as_ref()
            .is_some_and(|loan| loan.remaining() == 0)
        {
            let mut loan = self.active.take().unwrap();
            loan.closed_at_ms = Some(paid_at);
            self.history.push(loan);
            if self.history.len() > 8 {
                self.history.remove(0);
            }
        }
    }
}

impl NetworkSim {
    pub fn bank(&self) -> &BankState {
        &self.bank
    }

    pub(crate) fn configure_bank(
        &mut self,
        command: BankCommand,
    ) -> Result<Vec<SimEvent>, SimError> {
        match command {
            BankCommand::Borrow { amount, term_id } => {
                if self.bank.active.is_some() {
                    return Err(BankError::ActiveLoan.into());
                }
                let quote = bank_catalog().quote(amount, &term_id)?;
                let money = self.money.checked_add(amount).ok_or(BankError::Capacity)?;
                let next_id = self
                    .bank
                    .next_loan_id
                    .checked_add(1)
                    .ok_or(BankError::Capacity)?;
                self.bank
                    .elapsed_ms
                    .checked_add(quote.duration_ms())
                    .ok_or(BankError::Capacity)?;
                self.bank.active = Some(BankLoan {
                    id: self.bank.next_loan_id,
                    quote,
                    opened_at_ms: self.bank.elapsed_ms,
                    repaid: 0,
                    closed_at_ms: None,
                });
                self.bank.next_loan_id = next_id;
                self.money = money;
            }
            BankCommand::Repay { loan_id, amount } => {
                let loan = self
                    .bank
                    .active
                    .as_mut()
                    .filter(|loan| loan.id == loan_id)
                    .ok_or(BankError::NoActiveLoan)?;
                let payment = amount.unwrap_or_else(|| loan.remaining());
                if payment <= 0 || payment > loan.remaining() {
                    return Err(BankError::InvalidRepayment.into());
                }
                if self.money < payment {
                    return Err(SimError::InsufficientFunds {
                        needed: payment,
                        available: self.money,
                    });
                }
                loan.repaid += payment;
                self.money -= payment;
                self.bank.finish_paid_loan(self.bank.elapsed_ms);
            }
        }
        Ok(vec![SimEvent::BankChanged])
    }
}
