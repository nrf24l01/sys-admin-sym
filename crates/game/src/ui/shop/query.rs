use super::catalog::{Offer, catalog};
use crate::app::{ShopSection, ShopSort, ShopState};
use crate::localization::{TranslationSnapshot, translation_snapshot};
use cloud_provider_sim::NetworkSim;
use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet};

struct SearchIndex {
    translation: TranslationSnapshot,
    text: BTreeMap<String, String>,
}
thread_local! {
    static SEARCH_INDEX: RefCell<Option<SearchIndex>> = const { RefCell::new(None) };
}

fn search_matches(offer: &Offer, search: &str) -> bool {
    SEARCH_INDEX.with(|index| {
        let mut index = index.borrow_mut();
        if index
            .as_ref()
            .is_none_or(|index| !index.translation.is_current())
        {
            *index = Some(SearchIndex {
                translation: translation_snapshot(),
                text: catalog()
                    .iter()
                    .map(|offer| (offer.id.clone(), offer.search_text()))
                    .collect(),
            });
        }
        let text = &index.as_ref().unwrap().text[&offer.id];
        search
            .to_lowercase()
            .split_whitespace()
            .all(|term| text.contains(term))
    })
}

pub(super) fn matches(
    offer: &Offer,
    state: &ShopState,
    sim: &NetworkSim,
    skip_facet: Option<&str>,
) -> bool {
    (state.all_categories
        || (offer.section.category() == state.category
            && state.section.is_none_or(|s| s == offer.section)))
        && global_matches(offer, state, sim)
        && state
            .rack_units
            .is_none_or(|n| offer.numeric("rack") == u64::from(n))
        && state
            .ports
            .is_none_or(|n| offer.numeric("ports") == u64::from(n))
        && state
            .outlets
            .is_none_or(|n| offer.numeric("outlets") == u64::from(n))
        && state.facets.iter().all(|(key, values)| {
            skip_facet == Some(key.as_str())
                || values.is_empty()
                || offer.values(key).any(|v| values.contains(v))
        })
        && (!state.compatible_only
            || state.target.is_some_and(|target| {
                offer
                    .compatibility(sim, target)
                    .is_some_and(|result| result.is_ok())
            }))
}

pub(super) fn global_matches(offer: &Offer, state: &ShopState, sim: &NetworkSim) -> bool {
    let search = state.search.trim();
    (search.is_empty() || search_matches(offer, search))
        && state.min_price.is_none_or(|p| offer.price() >= p)
        && state.max_price.is_none_or(|p| offer.price() <= p)
        && (!state.affordable_only || offer.price() <= sim.money)
}

pub(super) fn filtered(state: &ShopState, sim: &NetworkSim) -> Vec<&'static Offer> {
    let mut offers: Vec<_> = catalog()
        .iter()
        .filter(|offer| matches(offer, state, sim, None))
        .collect();
    offers.sort_by(|a, b| order(a, b, state.sort));
    offers
}

pub(super) fn order(a: &Offer, b: &Offer, sort: ShopSort) -> std::cmp::Ordering {
    let ordering = match sort {
        ShopSort::Category => {
            (a.section.category(), a.section).cmp(&(b.section.category(), b.section))
        }
        ShopSort::Name => std::cmp::Ordering::Equal,
        ShopSort::PriceAscending => a.price().cmp(&b.price()),
        ShopSort::PriceDescending => b.price().cmp(&a.price()),
        ShopSort::CapacityDescending => b.numeric("capacity").cmp(&a.numeric("capacity")),
        ShopSort::SpeedDescending => b.numeric("speed").cmp(&a.numeric("speed")),
        ShopSort::LengthAscending => a.numeric("length").cmp(&b.numeric("length")),
        ShopSort::CpuSocket => a.values("socket").next().cmp(&b.values("socket").next()),
        ShopSort::MemoryType => a
            .values("memory-type")
            .next()
            .cmp(&b.values("memory-type").next()),
        ShopSort::RamSlotsDescending => b.numeric("ram-slots").cmp(&a.numeric("ram-slots")),
        ShopSort::CpuSocketsDescending => b.numeric("cpu-sockets").cmp(&a.numeric("cpu-sockets")),
        ShopSort::DriveBaysDescending => b.numeric("drive-bays").cmp(&a.numeric("drive-bays")),
        ShopSort::PcieSlotsDescending => b.numeric("pcie-slots").cmp(&a.numeric("pcie-slots")),
    };
    ordering
        .then_with(|| a.name().to_lowercase().cmp(&b.name().to_lowercase()))
        .then(a.id.cmp(&b.id))
}

pub(super) fn selected_variant<'a>(variants: &[&'a Offer], state: &ShopState) -> &'a Offer {
    state
        .variants
        .get(&variants[0].family)
        .and_then(|id| variants.iter().find(|offer| &offer.id == id))
        .copied()
        .unwrap_or(variants[0])
}

pub(super) fn grouped(offers: &[&'static Offer]) -> Vec<Vec<&'static Offer>> {
    let mut groups: Vec<Vec<&Offer>> = Vec::new();
    let mut indexes: BTreeMap<&str, usize> = BTreeMap::new();
    for &offer in offers {
        if let Some(&index) = indexes.get(offer.family.as_str()) {
            groups[index].push(offer);
        } else {
            indexes.insert(offer.family.as_str(), groups.len());
            groups.push(vec![offer]);
        }
    }
    groups
}

/// Explicit per-product filters prevent unrelated specifications leaking into browsing.
pub(super) fn facet_keys(state: &ShopState) -> &'static [&'static str] {
    if state.all_categories {
        return &[];
    }
    match state.section {
        None => &[],
        Some(ShopSection::Routers) => &["speed", "ports", "rack"],
        Some(ShopSection::Switches) => &["speed", "ports", "cages", "cage", "rack"],
        Some(ShopSection::Servers) => &[
            "memory-type",
            "socket",
            "ram-slots",
            "configuration",
            "cpu-sockets",
            "pcie-slots",
            "pcie-generation",
            "pcie-width",
            "drive-bays",
            "drive-interface",
            "rack",
            "ports",
            "psu-watts",
        ],
        Some(ShopSection::Cpu) => &["socket", "cores", "frequency", "watts"],
        Some(ShopSection::Ram) => &["capacity", "memory-type"],
        Some(ShopSection::PciCards) => &[
            "speed",
            "ports",
            "cages",
            "cage",
            "pcie-generation",
            "pcie-width",
        ],
        Some(ShopSection::Storage) => &["type", "capacity", "interface", "read"],
        Some(ShopSection::Transceivers) => &[
            "cage", "speed", "medium", "fiber", "strands", "reach", "dom",
        ],
        Some(ShopSection::FiberCables) => &["fiber", "strands", "length", "medium"],
        Some(ShopSection::DirectAttach) => &["type", "cage", "speed", "length"],
        Some(ShopSection::CopperSupplies) => &["type"],
        Some(ShopSection::PatchPanels) => &["type", "ports", "lc-pairs", "rack"],
        Some(ShopSection::CableManagers) => &["rack"],
        Some(ShopSection::Ups) => &["watts", "va", "battery", "outlets", "outlet-type", "rack"],
        Some(ShopSection::Pdu) => &["outlets", "outlet-type", "watts", "rack"],
        Some(ShopSection::PublicIp) => &["prefix", "addresses"],
    }
}

pub(super) fn facet_values(key: &str, state: &ShopState, sim: &NetworkSim) -> Vec<(String, usize)> {
    // Keep the section's choices stable as other filters narrow the results.
    // Unavailable values remain visible with zero counts. Single-value
    // specifications are useful filters too, especially for small catalogs.
    let mut counts: BTreeMap<String, BTreeSet<&str>> = catalog()
        .iter()
        .filter(|offer| !state.all_categories && state.section == Some(offer.section))
        .flat_map(|offer| offer.values(key))
        .map(|value| (value.to_owned(), BTreeSet::new()))
        .collect();
    for offer in catalog()
        .iter()
        .filter(|o| matches(o, state, sim, Some(key)))
    {
        for value in offer.values(key) {
            counts
                .entry(value.into())
                .or_default()
                .insert(&offer.family);
        }
    }
    if let Some(selected) = state.facets.get(key) {
        for value in selected {
            counts.entry(value.clone()).or_default();
        }
    }
    let mut values: Vec<_> = counts
        .into_iter()
        .map(|(value, families)| (value, families.len()))
        .collect();
    values.sort_by(
        |(a, _), (b, _)| match (a.parse::<u64>(), b.parse::<u64>()) {
            (Ok(a), Ok(b)) => a.cmp(&b),
            _ => a.cmp(b),
        },
    );
    values
}
