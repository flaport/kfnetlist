//! Native hierarchy assembly with a separate name-conversion seam for bindings.
use crate::{
    db,
    electrical::{self, MarkerCell, PortMapping},
    geometry::{self, OpticalInstance},
    Error, Port, Result,
};
use indexmap::IndexMap;
use kfnetlist_core::{
    self as model, BBox, Net, NetMember, Netlist, NetlistData, NetlistPort, PlacedExtra, Placement,
    PortArrayRef, PortRef,
};
use std::collections::{HashMap, HashSet};

pub struct InstanceInput {
    pub native: db::InstanceId,
    pub name: String,
    pub cell: String,
    pub component: String,
    pub kcl: String,
    pub settings: serde_json::Value,
    pub info: serde_json::Map<String, serde_json::Value>,
    pub named: bool,
    pub purpose: Option<String>,
    pub na: i64,
    pub nb: i64,
}
pub struct CellInput {
    pub id: db::CellId,
    pub name: String,
    pub ports: Vec<String>,
    pub optical_ports: Vec<Port>,
    pub optical_instances: Vec<OpticalInstance>,
    pub instances: Vec<InstanceInput>,
    /// Effective factory/cell metadata supplied by the caller.
    pub equivalents: Option<Vec<Vec<String>>>,
}
#[derive(Default)]
pub enum Flatten {
    #[default]
    No,
    All,
    Cells(Vec<String>),
}
pub struct Options {
    pub port_types: Vec<String>,
    pub mark_port_types: Vec<String>,
    pub connectivity: Vec<Vec<db::LayerInfo>>,
    pub equivalent_ports: Option<IndexMap<String, Vec<Vec<String>>>>,
    pub ignore_unnamed: bool,
    pub exclude_purposes: Vec<String>,
    pub allow_width_mismatch: bool,
    pub include_placement: bool,
    pub flatten: Flatten,
}
impl Default for Options {
    fn default() -> Self {
        Self {
            port_types: vec!["optical".into()],
            mark_port_types: vec!["electrical".into(), "RF".into(), "DC".into()],
            connectivity: vec![],
            equivalent_ports: None,
            ignore_unnamed: false,
            exclude_purposes: vec![],
            allow_width_mismatch: false,
            include_placement: false,
            flatten: Flatten::No,
        }
    }
}
pub struct InstanceMatch {
    pub instance: db::InstanceId,
    pub default_name: String,
}
enum PendingRef {
    Port(String),
    Instance {
        index: usize,
        port: String,
        ia: i64,
        ib: i64,
    },
}
struct PreparedCell {
    name: String,
    netlist: Netlist,
    pending: Vec<Vec<PendingRef>>,
    remove: Vec<String>,
    extras: IndexMap<String, PlacedExtra>,
    cells: HashMap<String, String>,
}
/// A binding may convert each matched native instance through its existing name
/// hook, then pass the resulting strings to finish. Rust never calls Python.
pub struct PreparedExtraction {
    pub matches: Vec<InstanceMatch>,
    match_count: usize,
    cells: Vec<PreparedCell>,
    equivalents: model::EquivalentPorts,
    mapping: model::PortMapping,
    options: Options,
}
pub struct ExtractionOutput {
    pub netlists: IndexMap<String, NetlistData>,
    pub placed: bool,
}
pub fn placement(transform: db::DCplxTrans, bounds: db::DBox) -> Result<Placement> {
    let origin = transform.displacement()?;
    Ok(Placement {
        x: origin.x(),
        y: origin.y(),
        orientation: transform.angle()?,
        mirror: transform.mirror_x(),
        bbox: BBox {
            left: bounds.left(),
            bottom: bounds.bottom(),
            right: bounds.right(),
            top: bounds.top(),
        },
    })
}
pub fn prepare(
    layout: &db::Layout,
    root: db::CellId,
    markers: &[MarkerCell],
    cells: Vec<CellInput>,
    mut options: Options,
) -> Result<PreparedExtraction> {
    let equivalent = options.equivalent_ports.take().unwrap_or_else(|| {
        cells
            .iter()
            .filter_map(|c| c.equivalents.as_ref().map(|e| (c.name.clone(), e.clone())))
            .collect()
    });
    let mut mapping = PortMapping::new();
    for (name, groups) in &equivalent {
        for ports in groups {
            if let Some(first) = ports.first() {
                for port in ports {
                    mapping
                        .entry(name.clone())
                        .or_default()
                        .insert(port.clone(), first.clone());
                }
            }
        }
    }
    let mut l2n = electrical::l2n_elec(
        layout,
        root,
        markers,
        &options.mark_port_types,
        &options.connectivity,
        &mapping,
    )?;
    let graph = l2n
        .netlist()?
        .ok_or_else(|| Error::Missing("missing extracted netlist".into()))?;
    let dbu = layout.dbu()?;
    let mut prepared = Vec::new();
    let mut matches = Vec::new();
    for cell in cells {
        let mut netlist = Netlist::default();
        for inst in &cell.instances {
            netlist.create_inst_with_info(
                inst.name.clone(),
                inst.kcl.clone(),
                inst.component.clone(),
                inst.settings.clone(),
                inst.na,
                inst.nb,
                inst.info.clone(),
            )?;
        }
        for name in cell.ports {
            netlist.create_port(name);
        }
        for net in geometry::optical_nets(
            cell.optical_ports,
            &cell.optical_instances,
            &options.port_types,
            options.allow_width_mismatch,
        )? {
            netlist.add_net(&net)?;
        }
        let mut pending = Vec::new();
        if let Some(circuit) = graph.circuit_by_name(&cell.name)? {
            for net in circuit.nets()? {
                let mut refs = Vec::new();
                for pin in net.pins()? {
                    refs.push(PendingRef::Port(pin.name()?));
                }
                for reference in net.subcircuit_pins()? {
                    let sub = reference.subcircuit;
                    let pin = reference.pin.name()?;
                    let Some(target) = sub.circuit_ref()? else {
                        continue;
                    };
                    let parent = layout
                        .find_cell(&sub.circuit()?.name()?)?
                        .ok_or_else(|| Error::Missing("missing parent circuit".into()))?;
                    let target_name = l2n.internal_layout().cell_name(target.cell_index()?)?;
                    let target = layout
                        .find_cell(&target_name)?
                        .ok_or(Error::Missing(target_name))?;
                    let transform = sub.trans()?.to_itype(dbu)?;
                    let bounds = db::Box::new(-1, -1, 1, 1).transformed_complex(transform)?;
                    let mut cursor = layout
                        .cell(parent)?
                        .recursive_instance_cursor(Some(bounds), Some(0))?;
                    cursor.set_targets(layout, &[target])?;
                    cursor.set_overlapping(true)?;
                    while let Some(element) = cursor.next_with_layout(layout)? {
                        if element.element_trans == transform && !pin.is_empty() {
                            let instance = layout.live_instance_from_snapshot(element.instance)?;
                            let mut name = String::new();
                            for input in &cell.instances {
                                if layout.instances_equal(&input.native, &instance)? {
                                    name = input.name.clone();
                                    break;
                                }
                            }
                            let index = matches.len();
                            matches.push(InstanceMatch {
                                instance,
                                default_name: name,
                            });
                            refs.push(PendingRef::Instance {
                                index,
                                port: pin,
                                ia: element.ia,
                                ib: element.ib,
                            });
                            break;
                        }
                    }
                }
                if !refs.is_empty() {
                    pending.push(refs);
                }
            }
        }
        let mut removed = HashSet::new();
        let mut extras = IndexMap::new();
        let mut instance_cells = HashMap::new();
        for inst in cell.instances {
            if (options.ignore_unnamed && !inst.named)
                || inst
                    .purpose
                    .as_ref()
                    .is_some_and(|p| options.exclude_purposes.contains(p))
            {
                removed.insert(inst.name.clone());
            }
            instance_cells.insert(inst.name.clone(), inst.cell.clone());
            if options.include_placement && !removed.contains(&inst.name) {
                let view = layout.instance(&inst.native)?;
                let bounds = view
                    .bbox(None)?
                    .unwrap_or_else(db::Box::empty)
                    .to_dtype(dbu)?;
                extras.insert(
                    inst.name,
                    PlacedExtra {
                        cell: inst.cell,
                        placement: placement(view.array()?.trans.to_dtype(dbu)?, bounds)?,
                    },
                );
            }
        }
        prepared.push(PreparedCell {
            name: cell.name,
            netlist,
            pending,
            remove: removed.into_iter().collect(),
            extras,
            cells: instance_cells,
        });
    }
    Ok(PreparedExtraction {
        match_count: matches.len(),
        matches,
        cells: prepared,
        equivalents: equivalent.into_iter().collect(),
        mapping: mapping
            .into_iter()
            .map(|(k, v)| (k, v.into_iter().collect()))
            .collect(),
        options,
    })
}
impl PreparedExtraction {
    pub fn take_matches(&mut self) -> Vec<InstanceMatch> {
        std::mem::take(&mut self.matches)
    }
    pub fn finish(self, names: &[String]) -> Result<ExtractionOutput> {
        if names.len() != self.match_count {
            return Err(Error::Invalid(
                "one converted name is required per matched instance".into(),
            ));
        }
        let mut result = IndexMap::new();
        let mut cell_maps = HashMap::new();
        for mut cell in self.cells {
            for pending in cell.pending {
                let mut members = Vec::new();
                for member in pending {
                    members.push(match member {
                        PendingRef::Port(name) => NetMember::Port(cell.netlist.create_port(name)),
                        PendingRef::Instance {
                            index,
                            port,
                            ia,
                            ib,
                        } => {
                            if ia < 0 {
                                NetMember::Ref(PortRef {
                                    instance: names[index].clone(),
                                    port,
                                })
                            } else {
                                NetMember::ArrayRef(PortArrayRef {
                                    instance: names[index].clone(),
                                    port,
                                    ia,
                                    ib,
                                })
                            }
                        }
                    });
                }
                cell.netlist.create_net(members)?;
            }
            cell.netlist.remove_instances(cell.remove)?;
            cell.netlist.sort();
            if self.equivalents.contains_key(&cell.name) {
                // Existing core normalization takes owned maps and returns an
                // independent graph; retain the shared hierarchy maps for peers.
                cell.netlist = cell.netlist.normalize(
                    Some(cell.name.clone()),
                    Some(self.equivalents.clone()),
                    Some(self.mapping.clone()),
                )?;
            }
            cell.netlist.sort();
            let mut data = NetlistData::from(cell.netlist);
            data.extras = cell.extras;
            cell_maps.insert(cell.name.clone(), cell.cells);
            result.insert(cell.name, data);
        }
        let selection = match &self.options.flatten {
            Flatten::No => {
                return Ok(ExtractionOutput {
                    netlists: result,
                    placed: self.options.include_placement,
                })
            }
            Flatten::All => None,
            Flatten::Cells(cells) => Some(cells.clone()),
        };
        let options = model::FlattenOptions::new(selection, None, true, false, false, ".".into());
        // Every base is flattened against the original hierarchy, preserving
        // reference behavior regardless of output iteration order.
        let originals = result
            .iter()
            .map(|(name, data)| (name.clone(), data.clone()))
            .collect::<HashMap<_, _>>();
        let mut flattened = IndexMap::new();
        for (name, data) in result {
            flattened.insert(
                name.clone(),
                model::flatten_netlist(data, &cell_maps[&name], &originals, &cell_maps, &options)?
                    .data,
            );
        }
        Ok(ExtractionOutput {
            netlists: flattened,
            placed: self.options.include_placement,
        })
    }
}
/// Standalone Rust entry point, using the supplied instance metadata names.
pub fn extract(
    layout: &db::Layout,
    root: db::CellId,
    markers: &[MarkerCell],
    cells: Vec<CellInput>,
    options: Options,
) -> Result<ExtractionOutput> {
    let prepared = prepare(layout, root, markers, cells, options)?;
    let names = prepared
        .matches
        .iter()
        .map(|m| m.default_name.clone())
        .collect::<Vec<_>>();
    prepared.finish(&names)
}
