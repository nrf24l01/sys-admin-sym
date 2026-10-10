use cloud_provider_sim::*;

fn borrow(sim: &mut NetworkSim, amount: i64) {
    sim.execute(Command::Bank(BankCommand::Borrow {
        amount,
        term_id: "standard".into(),
    }))
    .unwrap();
}

fn repay(sim: &mut NetworkSim, amount: Option<i64>) -> Result<Vec<SimEvent>, SimError> {
    let loan_id = sim.bank().active().unwrap().id;
    sim.execute(Command::Bank(BankCommand::Repay { loan_id, amount }))
}

#[test]
fn borrowing_credits_cash_and_records_the_disclosed_fixed_cost() {
    let mut sim = NetworkSim::new();
    let original_cash = sim.money;
    borrow(&mut sim, 5000);
    let loan = sim.bank().active().unwrap();
    assert_eq!(sim.money, original_cash + 5000);
    assert_eq!(loan.quote.principal, 5000);
    assert_eq!(loan.quote.interest, 500);
    assert_eq!(loan.remaining(), 5500);
    assert_eq!(loan.next_payment(), Some((300_000, 459)));
    assert_eq!(sim.bank().overdue(), 0);
}

#[test]
fn automatic_payments_start_at_the_deadline_and_round_to_the_exact_total() {
    let mut sim = NetworkSim::new();
    borrow(&mut sim, 5000);
    let cash = sim.money;
    sim.advance_time(299_999);
    assert_eq!(sim.money, cash);
    sim.advance_time(1);
    assert_eq!(sim.money, cash - 459);
    assert_eq!(sim.bank().active().unwrap().repaid, 459);
    sim.advance_time(300_000);
    assert_eq!(sim.bank().active().unwrap().repaid, 917);
    sim.advance_time(3_000_000);
    assert!(sim.bank().active().is_none());
    assert_eq!(sim.money, cash - 5500);
    assert_eq!(sim.bank().history()[0].repaid, 5500);
    sim.advance_time(10_000_000);
    assert_eq!(sim.money, cash - 5500);
}

#[test]
fn insufficient_cash_creates_arrears_and_later_cash_pays_them_without_overdraft() {
    let mut sim = NetworkSim::new();
    borrow(&mut sim, 5000);
    sim.money = 100;
    sim.advance_time(600_000);
    assert_eq!(sim.money, 0);
    assert_eq!(sim.bank().active().unwrap().repaid, 100);
    assert_eq!(sim.bank().overdue(), 817);
    sim.money = 200;
    sim.advance_time(0);
    assert_eq!(sim.money, 0);
    assert_eq!(sim.bank().overdue(), 617);
    sim.money = 1000;
    sim.advance_time(0);
    assert_eq!(sim.money, 383);
    assert_eq!(sim.bank().overdue(), 0);
    assert_eq!(
        sim.bank().active().unwrap().next_payment(),
        Some((900_000, 458))
    );
    sim.money = 0;
    sim.advance_time(u64::MAX);
    assert_eq!(sim.bank().active().unwrap().remaining(), 4583);
    assert_eq!(sim.bank().overdue(), 4583);
}

#[test]
fn early_payment_covers_oldest_installments_and_full_repayment_unlocks_new_credit() {
    let mut sim = NetworkSim::new();
    borrow(&mut sim, 5000);
    repay(&mut sim, Some(1000)).unwrap();
    assert_eq!(
        sim.bank().active().unwrap().next_payment(),
        Some((900_000, 375))
    );
    let cash = sim.money;
    sim.advance_time(600_000);
    assert_eq!(sim.money, cash);
    repay(&mut sim, None).unwrap();
    assert!(sim.bank().active().is_none());
    assert_eq!(sim.bank().history()[0].repaid, 5500);
    assert_eq!(sim.bank().history()[0].closed_at_ms, Some(600_000));
    borrow(&mut sim, 500);
    assert_eq!(sim.bank().active().unwrap().id, 2);
    assert_eq!(sim.bank().active().unwrap().opened_at_ms, 600_000);
}

#[test]
fn invalid_and_duplicate_loans_leave_cash_and_bank_unchanged() {
    let mut sim = NetworkSim::new();
    for (amount, term_id) in [
        (-1, "standard"),
        (0, "standard"),
        (499, "standard"),
        (501, "standard"),
        (25_100, "standard"),
        (1000, "missing"),
    ] {
        let original = sim.bank().clone();
        let cash = sim.money;
        assert!(
            sim.execute(Command::Bank(BankCommand::Borrow {
                amount,
                term_id: term_id.into()
            }))
            .is_err()
        );
        assert_eq!(sim.bank(), &original);
        assert_eq!(sim.money, cash);
    }
    borrow(&mut sim, 500);
    let original = sim.bank().clone();
    let cash = sim.money;
    assert_eq!(
        sim.execute(Command::Bank(BankCommand::Borrow {
            amount: 500,
            term_id: "standard".into(),
        })),
        Err(SimError::Bank(BankError::ActiveLoan))
    );
    assert_eq!(sim.bank(), &original);
    assert_eq!(sim.money, cash);
}

#[test]
fn invalid_unaffordable_and_stale_repayments_do_not_spend_money() {
    let mut sim = NetworkSim::new();
    borrow(&mut sim, 500);
    for amount in [0, -10, 551] {
        let original = sim.bank().clone();
        let cash = sim.money;
        assert_eq!(
            repay(&mut sim, Some(amount)),
            Err(SimError::Bank(BankError::InvalidRepayment))
        );
        assert_eq!(sim.bank(), &original);
        assert_eq!(sim.money, cash);
    }
    sim.money = 50;
    let original = sim.bank().clone();
    assert!(matches!(
        repay(&mut sim, Some(100)),
        Err(SimError::InsufficientFunds { .. })
    ));
    assert_eq!(sim.money, 50);
    assert_eq!(sim.bank(), &original);
    sim.money = 1000;
    repay(&mut sim, None).unwrap();
    borrow(&mut sim, 500);
    let original = sim.bank().clone();
    let cash = sim.money;
    assert_eq!(
        sim.execute(Command::Bank(BankCommand::Repay {
            loan_id: 1,
            amount: None
        })),
        Err(SimError::Bank(BankError::NoActiveLoan))
    );
    assert_eq!(sim.bank(), &original);
    assert_eq!(sim.money, cash);
}

#[test]
fn saving_preserves_deadlines_arrears_and_contract_terms_without_restart_interest() {
    let mut sim = NetworkSim::new();
    borrow(&mut sim, 5000);
    sim.money = 10;
    sim.advance_time(425_000);
    let saved = ron::to_string(&sim).unwrap();
    let mut loaded: NetworkSim = ron::from_str(&saved).unwrap();
    loaded.rebuild_indexes();
    assert_eq!(loaded.bank(), sim.bank());
    assert_eq!(loaded.money, sim.money);
    loaded.money = 500;
    sim.money = 500;
    loaded.advance_time(175_000);
    sim.advance_time(175_000);
    assert_eq!(loaded.bank(), sim.bank());
    assert_eq!(loaded.money, sim.money);
    assert_eq!(loaded.bank().active().unwrap().quote.interest, 500);
}

#[test]
fn legacy_saves_start_with_no_debt_and_keep_their_cash() {
    let sim = NetworkSim::new();
    let mut saved = serde_json::to_value(&sim).unwrap();
    saved.as_object_mut().unwrap().remove("bank");
    let mut loaded: NetworkSim = serde_json::from_value(saved).unwrap();
    loaded.rebuild_indexes();
    assert!(loaded.bank().active().is_none());
    assert!(loaded.bank().history().is_empty());
    assert_eq!(loaded.money, sim.money);
}

#[test]
fn one_large_time_step_matches_many_installment_steps() {
    let mut one = NetworkSim::new();
    borrow(&mut one, 2000);
    one.money = 1800;
    let mut many = one.clone();
    one.advance_time(2_700_000);
    for _ in 0..9 {
        many.advance_time(300_000);
    }
    assert_eq!(one.money, many.money);
    assert_eq!(one.bank(), many.bank());
}

#[test]
fn fully_paid_loan_history_records_the_actual_deadline_in_any_time_step_size() {
    let mut one = NetworkSim::new();
    borrow(&mut one, 2000);
    let mut many = one.clone();
    one.advance_time(4_500_000);
    for _ in 0..15 {
        many.advance_time(300_000);
    }
    assert!(one.bank().active().is_none());
    assert_eq!(one.money, many.money);
    assert_eq!(one.bank(), many.bank());
    assert_eq!(one.bank().history()[0].closed_at_ms, Some(3_600_000));
}

#[test]
fn overdue_debt_takes_priority_over_proceeds_from_selling_equipment() {
    let mut sim = NetworkSim::new();
    let SimEvent::DeviceAdded(device) = sim
        .execute(Command::BuyDevice {
            kind: DeviceTemplate::Server,
        })
        .unwrap()[0]
    else {
        panic!()
    };
    borrow(&mut sim, 5000);
    sim.money = 0;
    sim.advance_time(3_600_000);
    let debt = sim.bank().overdue();
    sim.execute(Command::SellDevice { device }).unwrap();
    assert_eq!(sim.money, 0);
    assert!(sim.bank().overdue() < debt);
}

#[test]
fn bank_terms_are_configurable_and_invalid_catalogs_are_rejected() {
    let catalog: BankCatalog = serde_json::from_str(
        r#"{
        "minimum_loan":100,"maximum_loan":1000,"amount_step":100,
        "payment_interval_minutes":2,
        "terms":[{"id":"test","installments":3,"interest_bps":333}]
    }"#,
    )
    .unwrap();
    assert!(catalog.validate());
    let quote = catalog.quote(100, "test").unwrap();
    assert_eq!(quote.interest, 4);
    assert_eq!(quote.first_payment(), 35);
    assert_eq!(quote.duration_ms(), 360_000);
    let mut invalid = catalog.clone();
    invalid.amount_step = 0;
    assert!(!invalid.validate());
    invalid = catalog.clone();
    invalid.terms[0].installments = 0;
    assert!(!invalid.validate());
    invalid = catalog.clone();
    invalid.terms.push(invalid.terms[0].clone());
    assert!(!invalid.validate());
}

#[test]
fn overflowing_cash_balance_rejects_the_loan_atomically() {
    let mut sim = NetworkSim::new();
    sim.money = i64::MAX;
    assert_eq!(
        sim.execute(Command::Bank(BankCommand::Borrow {
            amount: 500,
            term_id: "standard".into(),
        })),
        Err(SimError::Bank(BankError::Capacity))
    );
    assert_eq!(sim.money, i64::MAX);
    assert!(sim.bank().active().is_none());
}
