/// Compare actual bookings with the admission limit, formatting only failures.
/// Values use the same units and resource totals as provisioning.
pub(super) fn shortfall(
    resource: &str,
    committed: f64,
    requested: f64,
    limit: f64,
    unit: &str,
    context: impl FnOnce() -> String,
) -> Option<String> {
    let total = committed + requested;
    (total > limit).then(|| {
        let available = (limit - committed).max(0.0);
        format!(
            "{resource}: requested {requested:.2} {unit}, available {available:.2} {unit} \
             (committed {committed:.2}, after order {total:.2}, limit {limit:.2} {unit}). \
             Shortfall {:.2} {unit}. {}",
            total - limit,
            context(),
        )
    })
}
