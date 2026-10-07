use super::catalog::{Offer, catalog};
use crate::app::{ShopSort, ShopState};
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

pub(super) fn facet_keys(state: &ShopState) -> Vec<&'static str> {
    let keys: BTreeSet<_> = catalog()
        .iter()
        .filter(|offer| {
            state.all_categories
                || (offer.section.category() == state.category
                    && state.section.is_none_or(|s| s == offer.section))
        })
        .flat_map(|offer| offer.attributes.iter().map(|a| a.key))
        .filter(|key| {
            !matches!(
                *key,
                "frequency"
                    | "write"
                    | "write-iops"
                    | "power"
                    | "tx"
                    | "rx"
                    | "battery"
                    | "addresses"
                    | "prefix"
                    | "lanes"
            )
        })
        .collect();
    let priority = [
        "type",
        "configuration",
        "rack",
        "ports",
        "cages",
        "lc-pairs",
        "cage",
        "speed",
        "capacity",
        "interface",
        "socket",
        "cores",
        "memory-type",
        "pcie-generation",
        "pcie-width",
        "fiber",
        "strands",
        "length",
        "reach",
        "medium",
        "dom",
        "watts",
        "va",
        "outlets",
        "outlet-type",
        "read",
        "read-iops",
    ];
    let mut keys: Vec<_> = keys.into_iter().collect();
    keys.sort_by_key(|key| {
        priority
            .iter()
            .position(|candidate| candidate == key)
            .unwrap_or(usize::MAX)
    });
    keys
}

pub(super) fn facet_values(key: &str, state: &ShopState, sim: &NetworkSim) -> Vec<(String, usize)> {
    let mut counts: BTreeMap<String, BTreeSet<&str>> = BTreeMap::new();
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
