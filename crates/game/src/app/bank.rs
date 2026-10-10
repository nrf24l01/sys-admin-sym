pub struct BankWindowState {
    pub open: bool,
    pub amount: i64,
    pub term_id: String,
    pub repayment: i64,
}

impl Default for BankWindowState {
    fn default() -> Self {
        Self {
            open: false,
            amount: 5000,
            term_id: "standard".into(),
            repayment: 100,
        }
    }
}
