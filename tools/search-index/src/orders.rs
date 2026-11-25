//! The orderings of the Nachlass Map (By date, By name, By format), read from its data.json
//! (written by tools/map/map.py): each remark's position in each ordering, as a number to
//! sort by — the document's place in the ordering, then the remark's place in the document.

use serde::Deserialize;
use std::collections::HashMap;
use std::fs;
use std::path::Path;

#[derive(Deserialize)]
struct Data {
    /// ordering -> [(group label, [documents])]
    orders: HashMap<String, Vec<(String, Vec<String>)>>,
    /// document -> the heading ids of its remarks, in order
    anchors: HashMap<String, Vec<String>>,
}

/// Remarks per document are far fewer than this, so document and remark never overlap.
const SPAN: u32 = 100_000;

pub struct Orders {
    /// ordering -> document -> its place
    rank: HashMap<String, HashMap<String, u32>>,
    /// document -> heading id -> the remark's place
    position: HashMap<String, HashMap<String, u32>>,
}

impl Orders {
    pub fn load(path: &Path) -> Result<Orders, String> {
        let text = fs::read_to_string(path).map_err(|e| format!("reading {}: {e}", path.display()))?;
        let data: Data = serde_json::from_str(&text).map_err(|e| format!("parsing {}: {e}", path.display()))?;
        let rank = data
            .orders
            .into_iter()
            .map(|(name, groups)| {
                let docs = groups.into_iter().flat_map(|(_, docs)| docs);
                (name, docs.enumerate().map(|(i, d)| (d, i as u32)).collect())
            })
            .collect();
        let position = data
            .anchors
            .into_iter()
            .map(|(doc, ids)| (doc, ids.into_iter().enumerate().map(|(i, id)| (id, i as u32)).collect()))
            .collect();
        Ok(Orders { rank, position })
    }

    /// A remark's position in an ordering (None for a document or remark the map doesn't have).
    pub fn of(&self, ordering: &str, doc: &str, fragment: &str) -> Option<u32> {
        let rank = self.rank.get(ordering)?.get(doc)?;
        let position = self.position.get(doc)?.get(fragment)?;
        Some(rank * SPAN + position.min(&(SPAN - 1)))
    }
}
