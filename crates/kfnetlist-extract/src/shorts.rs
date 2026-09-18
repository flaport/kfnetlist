use crate::{parser, Result};
use rlayout::db;
use std::collections::HashSet;
#[derive(Debug)]
pub struct Short {
    pub net_a: String,
    pub net_b: String,
    pub layer: String,
    pub overlap: db::Region,
}
pub fn detect_shorts(
    l2n: &mut db::LayoutToNetlist,
    short_layers: Option<&HashSet<db::LayerInfo>>,
    circuit_name: Option<&str>,
) -> Result<Vec<Short>> {
    let mut layers = parser::discover_layers(l2n)?;
    if let Some(allowed) = short_layers {
        layers.retain(|(info, _)| allowed.contains(info));
    }
    if layers.is_empty() {
        return Ok(vec![]);
    }
    let internal = l2n.internal_layout();
    let name = match circuit_name.filter(|s| !s.is_empty()) {
        Some(s) => s.to_owned(),
        None => internal.cell_name(internal.top_cell_index()?)?,
    };
    let graph = l2n
        .netlist()?
        .ok_or_else(|| crate::Error::Missing("missing extracted netlist".into()))?;
    let Some(circuit) = graph.circuit_by_name(&name)? else {
        return Ok(vec![]);
    };
    let mut result = Vec::new();
    for (info, layer) in layers {
        let display = parser::layer_display_name(&info)?;
        let mut shapes = Vec::new();
        for net in circuit.nets()? {
            let region = l2n.shapes_of_net(&net, &layer, true)?;
            if region.is_empty()? {
                continue;
            }
            let name = net.name()?;
            shapes.push((
                if name.is_empty() {
                    format!("${}", net.cluster_id()?)
                } else {
                    name
                },
                region,
            ));
        }
        for (index, (a, shape)) in shapes.iter().enumerate() {
            for (b, other) in &shapes[index + 1..] {
                let overlap = shape.intersection(other)?;
                if !overlap.is_empty()? {
                    result.push(Short {
                        net_a: a.clone(),
                        net_b: b.clone(),
                        layer: display.clone(),
                        overlap,
                    });
                }
            }
        }
    }
    Ok(result)
}
