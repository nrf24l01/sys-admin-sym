use crate::app::{BankWindowState, UiAction};
use crate::localization::{tr, tr_args};
use bevy::prelude::MessageWriter;
use bevy_egui::egui;
use cloud_provider_sim::*;

#[cfg(test)]
mod tests;

pub(super) fn show(
    viewport: &mut egui::Ui,
    sim: &NetworkSim,
    state: &mut BankWindowState,
    actions: &mut MessageWriter<UiAction>,
) {
    if !state.open {
        return;
    }
    let mut open = true;
    egui::Window::new(tr("bank.title"))
        .id(egui::Id::new("bank-window"))
        .open(&mut open)
        .collapsible(false)
        .default_size(egui::vec2(540.0, 600.0))
        .vscroll(true)
        .show(viewport, |ui| {
            ui.heading(tr_args("bank.cash", &[sim.money.to_string()]));
            ui.weak(tr("bank.clock-help"));
            ui.separator();
            if let Some(loan) = sim.bank().active() {
                active_loan(ui, sim, loan, state, actions);
            } else {
                loan_offer(ui, state, actions);
            }
            if let Some(loan) = sim.bank().history().last() {
                ui.separator();
                ui.weak(tr_args(
                    "bank.last-paid",
                    &[loan.id.to_string(), loan.repaid.to_string()],
                ));
            }
        });
    state.open = open;
}

fn loan_offer(
    ui: &mut egui::Ui,
    state: &mut BankWindowState,
    actions: &mut MessageWriter<UiAction>,
) {
    let catalog = bank_catalog();
    if !catalog.terms.iter().any(|term| term.id == state.term_id) {
        state.term_id = catalog.terms[0].id.clone();
    }
    state.amount = state
        .amount
        .clamp(catalog.minimum_loan, catalog.maximum_loan);
    ui.heading(tr("bank.new-loan"));
    ui.label(tr_args(
        "bank.amount-range",
        &[
            catalog.minimum_loan.to_string(),
            catalog.maximum_loan.to_string(),
            catalog.amount_step.to_string(),
        ],
    ));
    ui.horizontal(|ui| {
        ui.label(tr("bank.amount"));
        ui.add(
            egui::DragValue::new(&mut state.amount)
                .range(catalog.minimum_loan..=catalog.maximum_loan)
                .speed(catalog.amount_step as f64)
                .prefix("$"),
        );
    });
    state.amount -= state.amount % catalog.amount_step;
    ui.horizontal(|ui| {
        ui.label(tr("bank.term"));
        egui::ComboBox::from_id_salt("bank-term")
            .selected_text(term_label(
                catalog
                    .terms
                    .iter()
                    .find(|term| term.id == state.term_id)
                    .unwrap(),
            ))
            .show_ui(ui, |ui| {
                for term in &catalog.terms {
                    ui.selectable_value(&mut state.term_id, term.id.clone(), term_label(term));
                }
            });
    });
    let quote = catalog
        .quote(state.amount, &state.term_id)
        .expect("valid loan draft");
    ui.separator();
    egui::Grid::new("bank-quote")
        .spacing([32.0, 8.0])
        .show(ui, |ui| {
            row(ui, "bank.you-receive", price(quote.principal));
            row(ui, "bank.fixed-interest", price(quote.interest));
            row(ui, "bank.total-repayment", price(quote.total()));
            row(
                ui,
                "bank.first-installment",
                tr_args(
                    "bank.installment-value",
                    &[
                        quote.first_payment().to_string(),
                        (quote.interval_ms / 60_000).to_string(),
                    ],
                ),
            );
            row(ui, "bank.payment-count", quote.installments.to_string());
        });
    ui.weak(tr("bank.terms-help"));
    ui.add_space(8.0);
    if ui
        .button(tr_args("bank.borrow", &[quote.principal.to_string()]))
        .clicked()
    {
        actions.write(UiAction::NetworkCommand(Command::Bank(
            BankCommand::Borrow {
                amount: quote.principal,
                term_id: state.term_id.clone(),
            },
        )));
    }
}

fn active_loan(
    ui: &mut egui::Ui,
    sim: &NetworkSim,
    loan: &BankLoan,
    state: &mut BankWindowState,
    actions: &mut MessageWriter<UiAction>,
) {
    ui.heading(tr_args("bank.loan-number", &[loan.id.to_string()]));
    let overdue = sim.bank().overdue();
    if overdue > 0 {
        ui.colored_label(
            egui::Color32::LIGHT_RED,
            tr_args("bank.overdue", &[overdue.to_string()]),
        );
        ui.label(tr("bank.overdue-help"));
    } else {
        ui.colored_label(egui::Color32::LIGHT_GREEN, tr("bank.on-schedule"));
    }
    egui::Grid::new("bank-active")
        .spacing([32.0, 8.0])
        .show(ui, |ui| {
            row(ui, "bank.borrowed", price(loan.quote.principal));
            row(ui, "bank.fixed-interest", price(loan.quote.interest));
            row(ui, "bank.total-repayment", price(loan.quote.total()));
            row(ui, "bank.repaid", price(loan.repaid));
            row(ui, "bank.remaining", price(loan.remaining()));
            if let Some((deadline, amount)) = loan.next_payment() {
                row(ui, "bank.next-payment", price(amount));
                row(
                    ui,
                    "bank.due-in",
                    duration(deadline.saturating_sub(sim.bank().elapsed_ms())),
                );
            }
            row(
                ui,
                "bank.final-deadline",
                duration(
                    (loan.opened_at_ms + loan.quote.duration_ms())
                        .saturating_sub(sim.bank().elapsed_ms()),
                ),
            );
        });
    ui.add(
        egui::ProgressBar::new(loan.repaid as f32 / loan.quote.total() as f32).show_percentage(),
    );
    ui.weak(tr("bank.autopay-help"));
    ui.separator();
    ui.heading(tr("bank.early-repayment"));
    state.repayment = state.repayment.clamp(1, loan.remaining());
    ui.horizontal(|ui| {
        ui.add(
            egui::DragValue::new(&mut state.repayment)
                .range(1..=loan.remaining())
                .speed(10)
                .prefix("$"),
        );
        if ui
            .add_enabled(
                sim.money >= state.repayment,
                egui::Button::new(tr("bank.repay")),
            )
            .clicked()
        {
            actions.write(UiAction::NetworkCommand(Command::Bank(
                BankCommand::Repay {
                    loan_id: loan.id,
                    amount: Some(state.repayment),
                },
            )));
        }
    });
    if ui
        .add_enabled(
            sim.money >= loan.remaining(),
            egui::Button::new(tr_args("bank.repay-all", &[loan.remaining().to_string()])),
        )
        .clicked()
    {
        actions.write(UiAction::NetworkCommand(Command::Bank(
            BankCommand::Repay {
                loan_id: loan.id,
                amount: None,
            },
        )));
    }
    ui.weak(tr("bank.one-loan-help"));
}

pub(super) fn summary(sim: &NetworkSim) -> String {
    if sim.bank().overdue() > 0 {
        tr_args("bank.overdue-short", &[sim.bank().overdue().to_string()])
    } else if let Some(loan) = sim.bank().active() {
        tr_args("bank.balance-short", &[loan.remaining().to_string()])
    } else {
        tr("bank.title")
    }
}

fn term_label(term: &LoanTerm) -> String {
    tr_args(
        "bank.term-option",
        &[
            (term.installments * bank_catalog().payment_interval_minutes).to_string(),
            format!("{:.2}", f64::from(term.interest_bps) / 100.0),
        ],
    )
}

fn duration(ms: u64) -> String {
    let seconds = ms.div_ceil(1000);
    tr_args(
        "bank.time",
        &[(seconds / 60).to_string(), format!("{:02}", seconds % 60)],
    )
}

fn price(amount: i64) -> String {
    tr_args("format.price", &[amount.to_string()])
}
fn row(ui: &mut egui::Ui, label: &str, value: String) {
    ui.label(tr(label));
    ui.strong(value);
    ui.end_row();
}
