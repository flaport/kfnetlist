//! Ordered native circuit traversal and the public L2N wire representation.
use crate::{Error, Result};
use indexmap::IndexMap;
use rlayout::db;
use serde::Serialize;
use std::collections::HashSet;

/// An insertion-ordered JSON tree. Model settings retain their existing map
/// semantics; this tree preserves the parser's independent public key ordering.
#[derive(Debug, Serialize)]
#[serde(untagged)]
pub enum Json {
    Object(IndexMap<String, Json>),
    Array(Vec<Json>),
    Scalar(serde_json::Value),
}
impl Json {
    pub fn object(entries: impl IntoIterator<Item = (String, Json)>) -> Self {
        Self::Object(entries.into_iter().collect())
    }
    pub fn value(value: impl Into<serde_json::Value>) -> Self {
        Self::Scalar(value.into())
    }
}
fn object<const N: usize>(entries: [(&str, Json); N]) -> Json {
    Json::object(entries.into_iter().map(|(k, v)| (k.into(), v)))
}
fn strings(values: Vec<String>) -> Json {
    Json::Array(values.into_iter().map(Json::value).collect())
}
pub fn layer_display_name(info: &db::LayerInfo) -> Result<String> {
    Ok(if info.name.is_empty() {
        info.to_native_string(false)?
    } else {
        info.name.clone()
    })
}
pub type LayerRegions = Vec<(db::LayerInfo, db::Region)>;
pub type LayerViews<'a> = [(db::LayerInfo, &'a db::Region)];
pub fn discover_layers(l2n: &mut db::LayoutToNetlist) -> Result<LayerRegions> {
    let mut result: IndexMap<db::LayerInfo, db::Region> = IndexMap::new();
    for (index, info) in l2n.internal_layout().layers()? {
        match l2n.layer_by_index(index) {
            Ok(Some(region)) => {
                result.insert(info, region);
            }
            Ok(None) => {}
            Err(error)
                if !error.is_invalid_input()
                    && !error.is_cancelled()
                    && !error.is_out_of_memory() => {}
            Err(error) => return Err(error.into()),
        }
    }
    Ok(result.into_iter().collect())
}
pub fn net_shapes(
    net: &db::Net,
    l2n: &db::LayoutToNetlist,
    layers: &LayerViews<'_>,
) -> Result<LayerRegions> {
    let mut result = Vec::new();
    for (info, region) in layers {
        let shapes = l2n.shapes_of_net(net, region, true)?;
        if !shapes.is_empty()? {
            result.push((info.clone(), shapes));
        }
    }
    Ok(result)
}
#[derive(Default, Debug)]
pub struct Filters {
    pub include_layers: Option<HashSet<db::LayerInfo>>,
    pub exclude_layers: Option<HashSet<db::LayerInfo>>,
    pub include_instances: Option<HashSet<String>>,
    pub exclude_instances: Option<HashSet<String>>,
}
pub fn serialize_net(
    net: &db::Net,
    l2n: Option<&db::LayoutToNetlist>,
    layers: &LayerViews<'_>,
    subcircuits: Option<&HashSet<u64>>,
    filters: &Filters,
) -> Result<Option<Json>> {
    let shapes = match l2n {
        Some(l2n) if !layers.is_empty() => {
            let shapes = net_shapes(net, l2n, layers)?;
            if filters
                .include_layers
                .as_ref()
                .is_some_and(|allowed| !shapes.iter().any(|(info, _)| allowed.contains(info)))
            {
                return Ok(None);
            }
            if filters.exclude_layers.as_ref().is_some_and(|excluded| {
                !shapes.is_empty() && shapes.iter().all(|(info, _)| excluded.contains(info))
            }) {
                return Ok(None);
            }
            shapes
        }
        _ => vec![],
    };
    let pins = net
        .pins()?
        .into_iter()
        .map(|p| p.name())
        .collect::<std::result::Result<Vec<_>, _>>()?
        .into_iter()
        .filter(|n| !n.is_empty())
        .collect::<Vec<_>>();
    let mut child_pins = Vec::new();
    for reference in net.subcircuit_pins()? {
        let sub = reference.subcircuit;
        let id = sub.id()?;
        if subcircuits.is_some_and(|included| !included.contains(&id)) {
            continue;
        }
        let pin = reference.pin.name()?;
        if pin.is_empty() {
            continue;
        }
        let name = sub.name()?;
        child_pins.push(object([
            (
                "subcircuit",
                Json::value(if name.is_empty() {
                    format!("${id}")
                } else {
                    name
                }),
            ),
            ("pin", Json::value(pin)),
        ]));
    }
    if pins.is_empty() && child_pins.is_empty() {
        return Ok(None);
    }
    let name = net.name()?;
    let mut result = IndexMap::new();
    result.insert(
        "name".into(),
        if name.is_empty() {
            Json::Scalar(serde_json::Value::Null)
        } else {
            Json::value(name)
        },
    );
    if !pins.is_empty() {
        result.insert("pins".into(), strings(pins));
    }
    if !child_pins.is_empty() {
        result.insert("subcircuit_pins".into(), Json::Array(child_pins));
    }
    if !shapes.is_empty() {
        let mut named = shapes
            .into_iter()
            .map(|(info, region)| Ok((layer_display_name(&info)?, region)))
            .collect::<Result<Vec<_>>>()?;
        named.sort_by(|a, b| a.0.cmp(&b.0));
        let mut polygons = IndexMap::new();
        let mut holes = IndexMap::new();
        for (name, region) in named {
            let mut hulls = Vec::new();
            let mut interiors = Vec::new();
            for polygon in region.polygons()? {
                let polygon = polygon?;
                hulls.push(points(polygon.hull_points()?));
                for i in 0..polygon.holes()? {
                    interiors.push(points(polygon.hole_points(i)?));
                }
            }
            polygons.insert(name.clone(), Json::Array(hulls));
            if !interiors.is_empty() {
                holes.insert(name, Json::Array(interiors));
            }
        }
        result.insert("layer_to_polygons".into(), Json::Object(polygons));
        if !holes.is_empty() {
            result.insert("layer_to_holes".into(), Json::Object(holes));
        }
    }
    Ok(Some(Json::Object(result)))
}
fn points(points: Vec<db::Point>) -> Json {
    Json::Array(
        points
            .into_iter()
            .map(|p| Json::Array(vec![Json::value(p.x), Json::value(p.y)]))
            .collect(),
    )
}
pub fn serialize_circuit(
    circuit: &db::Circuit,
    l2n: Option<&db::LayoutToNetlist>,
    layers: &LayerViews<'_>,
    filters: &Filters,
) -> Result<Json> {
    let mut pins = circuit
        .pins()?
        .into_iter()
        .map(|p| p.name())
        .collect::<std::result::Result<Vec<_>, _>>()?
        .into_iter()
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>();
    pins.sort();
    let mut ids = HashSet::new();
    let mut subs = Vec::new();
    for sub in circuit.subcircuits()? {
        let target = sub
            .circuit_ref()?
            .ok_or_else(|| Error::Missing("missing circuit reference".into()))?
            .name()?;
        if filters
            .include_instances
            .as_ref()
            .is_some_and(|set| !set.contains(&target))
            || filters
                .exclude_instances
                .as_ref()
                .is_some_and(|set| set.contains(&target))
        {
            continue;
        }
        let id = sub.id()?;
        ids.insert(id);
        let name = sub.name()?;
        subs.push(object([
            (
                "name",
                Json::value(if name.is_empty() {
                    format!("${id}")
                } else {
                    name
                }),
            ),
            ("circuit_ref", Json::value(target)),
            ("transform", Json::value(sub.trans()?.to_native_string()?)),
        ]));
    }
    let ids = if filters.include_instances.is_some() || filters.exclude_instances.is_some() {
        Some(&ids)
    } else {
        None
    };
    let mut nets = Vec::new();
    for net in circuit.nets()? {
        if let Some(net) = serialize_net(&net, l2n, layers, ids, filters)? {
            nets.push(net);
        }
    }
    let mut result = IndexMap::new();
    if !pins.is_empty() {
        result.insert("pins".into(), strings(pins));
    }
    if !subs.is_empty() {
        result.insert("subcircuits".into(), Json::Array(subs));
    }
    if !nets.is_empty() {
        result.insert("nets".into(), Json::Array(nets));
    }
    Ok(Json::Object(result))
}
pub fn parse_l2n(l2n: &mut db::LayoutToNetlist, flatten: bool, filters: &Filters) -> Result<Json> {
    let internal = l2n.internal_layout();
    let top = internal.cell_name(internal.top_cell_index()?)?;
    let dbu = internal.dbu()?;
    let layers = discover_layers(l2n)?;
    let mut reported = layers
        .iter()
        .filter(|(info, _)| {
            filters
                .include_layers
                .as_ref()
                .is_none_or(|s| s.contains(info))
                && filters
                    .exclude_layers
                    .as_ref()
                    .is_none_or(|s| !s.contains(info))
        })
        .map(|(info, _)| Ok((layer_display_name(info)?, info)))
        .collect::<Result<Vec<_>>>()?;
    reported.sort_by(|a, b| a.0.cmp(&b.0));
    let reported = Json::Array(
        reported
            .into_iter()
            .map(|(name, info)| {
                object([
                    ("name", Json::value(name)),
                    ("layer", Json::value(info.layer)),
                    ("datatype", Json::value(info.datatype)),
                ])
            })
            .collect(),
    );
    let views = layers
        .iter()
        .map(|(i, r)| (i.clone(), r))
        .collect::<Vec<_>>();
    let mut circuits = IndexMap::new();
    let netlist = l2n
        .netlist()?
        .ok_or_else(|| Error::Missing("missing extracted netlist".into()))?;
    if flatten {
        // Flattening explicitly works on a detached graph, preserving live L2N.
        let mut flat = netlist.try_clone()?;
        for circuit in flat.circuits(db::CircuitOrder::BottomUp)? {
            if circuit.name()? != top {
                flat.flatten_circuit(&circuit)?;
            }
        }
        let circuit = flat
            .circuit_by_name(&top)?
            .ok_or_else(|| Error::Missing(top.clone()))?;
        circuits.insert(
            top.clone(),
            serialize_circuit(&circuit, None, &[], &Filters::default())?,
        );
    } else {
        for circuit in netlist.circuits(db::CircuitOrder::TopDown)? {
            circuits.insert(
                circuit.name()?,
                serialize_circuit(&circuit, Some(l2n), &views, filters)?,
            );
        }
    }
    Ok(object([
        ("top_circuit", Json::value(top)),
        ("dbu", Json::value(dbu)),
        ("layers", reported),
        ("circuits", Json::Object(circuits)),
    ]))
}
