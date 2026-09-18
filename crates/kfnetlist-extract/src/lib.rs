//! Layout-dependent netlist operations. This crate never initializes Python.
use indexmap::IndexMap;
pub use rlayout::db;
mod error;
pub mod geometry;
pub mod ports;
pub use error::{Error, Result};

/// Port geometry preserves whether the caller supplied grid or physical units.
#[derive(Clone, Copy, Debug)]
pub enum PortTransform {
    Grid(db::Trans),
    Physical(db::DCplxTrans),
    Both(db::Trans, db::DCplxTrans),
}

/// Owned boundary metadata; no language objects or callbacks enter extraction.
#[derive(Clone, Debug)]
pub struct Port {
    pub name: String,
    pub port_type: String,
    pub transform: PortTransform,
    pub layer: db::LayerInfo,
    pub width: i64,
    pub dbu: f64,
    /// Equality key interned from cross-section metadata by the caller.
    pub cross_section: usize,
}

/// Metadata supplied separately from borrowed native layout storage.
#[derive(Debug, Default)]
pub struct CellMetadata {
    pub ports: Vec<Port>,
    pub settings: IndexMap<String, serde_json::Value>,
    pub info: IndexMap<String, serde_json::Value>,
    pub placement: Option<kfnetlist_core::Placement>,
}

/// Extract connected geometry from the actual source layout.
///
/// Resolve layer IDs before calling: native extraction installs a source read
/// lease. The returned context retains source storage, including after the Rust
/// layout wrapper is dropped. No layout duplication is performed here.
pub fn connected_geometry(
    layout: &db::Layout,
    root: db::CellId,
    connectivity: &[Vec<db::LayerId>],
) -> std::result::Result<db::LayoutToNetlist, rlayout::Error> {
    let mut extraction = db::LayoutToNetlist::from_cell(layout, root)?;
    let mut layers = IndexMap::new();
    for &id in connectivity.iter().flatten() {
        if let indexmap::map::Entry::Vacant(entry) = layers.entry(id) {
            let region = extraction.make_layer(id, &layout.layer_info(id)?.name)?;
            extraction.connect(&region)?;
            entry.insert(region);
        }
    }
    for group in connectivity {
        for pair in group.windows(2) {
            extraction.connect_layers(&layers[&pair[0]], &layers[&pair[1]])?;
        }
    }
    extraction.extract_netlist()?;
    extraction.check_extraction_errors()?;
    Ok(extraction)
}

pub mod electrical;
pub mod extract;
pub mod parser;
pub mod settings;
pub mod shorts;
