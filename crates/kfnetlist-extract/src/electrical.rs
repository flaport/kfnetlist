//! Port-marker extraction. The source copy is algorithm-required, not a bridge.
use crate::{connected_geometry, Error, Result};
use indexmap::IndexMap;
use rlayout::db;
use std::collections::HashMap;

#[derive(Debug)]
pub struct MarkerPort {
    pub name: String,
    pub port_type: String,
    pub layer: db::LayerInfo,
    pub transform: db::Trans,
}
#[derive(Debug)]
pub struct MarkerCell {
    pub name: String,
    pub factory: Option<String>,
    pub ports: Vec<MarkerPort>,
}
pub type PortMapping = IndexMap<String, IndexMap<String, String>>;
pub fn l2n_elec(
    source: &db::Layout,
    root: db::CellId,
    cells: &[MarkerCell],
    mark_types: &[String],
    connectivity: &[Vec<db::LayerInfo>],
    mappings: &PortMapping,
) -> Result<db::LayoutToNetlist> {
    let root_name = source.cell(root)?.name()?.to_owned();
    let mut layout = source.try_clone()?;
    for cell in cells {
        let id = layout
            .find_cell(&cell.name)?
            .ok_or_else(|| Error::Missing(cell.name.clone()))?;
        layout.cell_mut(id)?.set_locked(false)?;
        let mapping = mappings
            .get(&cell.name)
            .or_else(|| cell.factory.as_ref().and_then(|name| mappings.get(name)));
        let canonical = |name: &str| {
            mapping
                .and_then(|map| map.get(name))
                .map_or_else(|| name.to_owned(), Clone::clone)
        };
        let mut preferred = HashMap::new();
        for port in &cell.ports {
            if mark_types.contains(&port.port_type) {
                preferred.entry(canonical(&port.name)).or_insert(&port.name);
            }
        }
        for port in &cell.ports {
            let canonical = canonical(&port.name);
            if preferred.get(&canonical).copied() != Some(&port.name) {
                continue;
            }
            let layer = layout.layer(&port.layer)?;
            layout
                .cell_mut(id)?
                .insert_text(layer, &db::Text::new(&canonical, port.transform)?)?;
        }
    }
    // All layer resolution precedes the extraction context's source read lease.
    let resolved = connectivity
        .iter()
        .map(|group| {
            group
                .iter()
                .map(|info| layout.layer(info))
                .collect::<std::result::Result<Vec<_>, _>>()
        })
        .collect::<std::result::Result<Vec<_>, _>>()?;
    if resolved.iter().any(Vec::is_empty) {
        return Err(Error::EmptyConnectivity);
    }
    let root = layout
        .find_cell(&root_name)?
        .ok_or(Error::Missing(root_name))?;
    Ok(connected_geometry(&layout, root, &resolved)?)
}
