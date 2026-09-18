//! Ordered optical adjacency, including array elements and unconnected ports.
use crate::{ports::*, Error, Port, Result};
use indexmap::IndexMap;
use kfnetlist_core::{Net, NetMember, NetlistPort, PortArrayRef, PortRef};
use rlayout::db;
use std::collections::HashSet;

#[derive(Debug)]
pub struct OpticalInstance {
    pub name: String,
    pub na: i64,
    pub nb: i64,
    pub ports: Vec<Port>,
    /// Required only for arrays with more than one element.
    pub array: Option<db::CellInstArray>,
}
struct Resolved {
    port: Port,
    reference: NetMember,
    identity: Option<(String, String, i64, i64)>,
}
type Buckets = IndexMap<(i64, i64), IndexMap<(i32, i32), Vec<Resolved>>>;
fn insert(buckets: &mut Buckets, item: Resolved) -> Result<()> {
    let point = item.port.grid_transform()?.displacement;
    buckets
        .entry((i64::from(point.x), i64::from(point.y)))
        .or_default()
        .entry((item.port.layer.layer, item.port.layer.datatype))
        .or_default()
        .push(item);
    Ok(())
}
fn at(buckets: &Buckets, point: (i64, i64), layer: (i32, i32)) -> &[Resolved] {
    buckets
        .get(&point)
        .and_then(|layers| layers.get(&layer))
        .map_or(&[], Vec::as_slice)
}
fn reference(inst: &OpticalInstance, port: &Port, ia: i64, ib: i64) -> NetMember {
    if inst.na > 0 && inst.nb > 0 {
        NetMember::ArrayRef(PortArrayRef {
            instance: inst.name.clone(),
            port: port.name.clone(),
            ia,
            ib,
        })
    } else {
        NetMember::Ref(PortRef {
            instance: inst.name.clone(),
            port: port.name.clone(),
        })
    }
}
/// Input snapshots are owned metadata and value geometry; layout operations stay
/// in Rust. The insertion-ordered buckets preserve the public list ordering.
pub fn optical_nets(
    ports: Vec<Port>,
    instances: &[OpticalInstance],
    port_types: &[String],
    allow_width_mismatch: bool,
) -> Result<Vec<Net>> {
    let mut cell = Buckets::new();
    let mut children = Buckets::new();
    let mut names = HashSet::new();
    for port in ports {
        if !port_types.contains(&port.port_type) {
            continue;
        }
        if names.contains(&port.name) {
            return Err(Error::Invalid(format!(
                "Netlist extraction is not possible with colliding port names. Duplicate name: {}",
                port.name
            )));
        }
        if !port.name.is_empty() {
            names.insert(port.name.clone());
        }
        let reference = NetMember::Port(NetlistPort {
            name: port.name.clone(),
        });
        insert(
            &mut cell,
            Resolved {
                port,
                reference,
                identity: None,
            },
        )?;
    }
    for inst in instances {
        let elements: Vec<(i64, i64, Option<db::Trans>)> = if inst.na > 1 || inst.nb > 1 {
            let array = inst
                .array
                .ok_or_else(|| Error::Invalid("missing native instance array".into()))?;
            let transforms = array
                .transforms()
                .collect::<std::result::Result<Vec<_>, _>>()?;
            let mut elements = Vec::new();
            for ia in 0..inst.na {
                for ib in 0..inst.nb {
                    elements.push((
                        ia,
                        ib,
                        Some(transforms[(ib * inst.na + ia) as usize].to_orthogonal()?),
                    ));
                }
            }
            elements
        } else {
            vec![(0, 0, None)]
        };
        for (ia, ib, transform) in elements {
            for port in &inst.ports {
                if !port_types.contains(&port.port_type) {
                    continue;
                }
                // Each retained array member must own its metadata and transform.
                let port = match transform {
                    Some(t) => port.transformed(t)?,
                    None => port.clone(),
                };
                let reference = reference(inst, &port, ia, ib);
                let identity = Some((inst.name.clone(), port.name.clone(), ia, ib));
                insert(
                    &mut children,
                    Resolved {
                        port,
                        reference,
                        identity,
                    },
                )?;
            }
        }
    }
    let base = POSITION | LAYER | PORT_TYPE | if allow_width_mismatch { 0 } else { WIDTH };
    let mut nets = Vec::new();
    let mut connected_cell = HashSet::new();
    let mut connected_inst = HashSet::new();
    let mut connect = |a: &Resolved, b: &Resolved, snapped: bool, orientation: u8| -> Result<()> {
        let mask = base | orientation;
        if check_connection(
            &a.port,
            &b.port,
            CheckOptions {
                snapped,
                ..Default::default()
            },
        )? & mask
            == mask
        {
            // A net owns references independently of temporary geometry buckets.
            nets.push(Net::from_members(vec![
                a.reference.clone(),
                b.reference.clone(),
            ]));
            for item in [a, b] {
                if let Some(id) = &item.identity {
                    connected_inst.insert(id.clone());
                } else {
                    connected_cell.insert(item.port.name.clone());
                }
            }
        }
        Ok(())
    };
    for (&(x, y), layers) in &cell {
        for (&layer, ports) in layers {
            let extra = at(&cell, (x + 1, y), layer)
                .iter()
                .chain(at(&cell, (x, y + 1), layer));
            let child_buckets = &children;
            let near: Vec<_> = (-1..=1)
                .flat_map(|dx| {
                    (-1..=1).flat_map(move |dy| at(child_buckets, (x + dx, y + dy), layer))
                })
                .collect();
            for (n, port) in ports.iter().enumerate() {
                for other in ports[n + 1..].iter().chain(extra.clone()) {
                    connect(port, other, false, OPPOSITE)?;
                }
                for other in &near {
                    connect(port, other, true, SAME)?;
                }
            }
        }
    }
    for (&(x, y), layers) in &children {
        for (&layer, ports) in layers {
            let extra =
                at(&children, (x + 1, y), layer)
                    .iter()
                    .chain(at(&children, (x, y + 1), layer));
            for (n, port) in ports.iter().enumerate() {
                for other in ports[n + 1..].iter().chain(extra.clone()) {
                    connect(port, other, false, OPPOSITE)?;
                }
            }
        }
    }
    for item in cell
        .into_values()
        .chain(children.into_values())
        .flat_map(IndexMap::into_values)
        .flatten()
    {
        let connected = match &item.identity {
            Some(id) => connected_inst.contains(id),
            None => connected_cell.contains(&item.port.name),
        };
        if !connected {
            nets.push(Net::from_members(vec![item.reference]));
        }
    }
    Ok(nets)
}
