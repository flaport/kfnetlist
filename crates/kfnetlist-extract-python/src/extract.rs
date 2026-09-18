//! Read KFactory metadata and invoke its naming hook at the binding boundary.
use crate::{domain_error, model, ports, settings};
use indexmap::IndexMap;
use kfnetlist_extract::{
    db,
    electrical::{self, MarkerCell, MarkerPort, PortMapping},
    extract::{self as domain, CellInput, Flatten, InstanceInput, Options},
};
use pyo3::{
    exceptions::PyValueError,
    prelude::*,
    types::{PyBool, PyDict},
};
use rlayout_python::interop;

fn root_id(layout: &db::Layout, cell: &Bound<'_, PyAny>) -> PyResult<db::CellId> {
    let index = cell.call_method0("cell_index")?.extract()?;
    layout
        .cell_id_at(index)
        .map_err(interop::native_error)?
        .ok_or_else(|| PyValueError::new_err("root cell no longer exists"))
}
fn hierarchy<'py>(
    layout: &db::Layout,
    root: db::CellId,
    kcl: &Bound<'py, PyAny>,
) -> PyResult<Vec<Bound<'py, PyAny>>> {
    std::iter::once(root)
        .chain(
            layout
                .cell(root)
                .and_then(|c| c.called_ids())
                .map_err(interop::native_error)?,
        )
        .map(|id| kcl.get_item(id.index()))
        .collect()
}
fn factory(cell: &Bound<'_, PyAny>) -> PyResult<Option<String>> {
    if cell.call_method0("has_factory_name")?.is_truthy()? {
        Ok(Some(cell.getattr("factory_name")?.extract()?))
    } else {
        Ok(None)
    }
}
fn marker_cell(cell: &Bound<'_, PyAny>) -> PyResult<MarkerCell> {
    let mut ports = Vec::new();
    for port in cell.getattr("ports")?.try_iter()? {
        let port = port?;
        ports.push(MarkerPort {
            name: port.getattr("name")?.extract()?,
            port_type: port.getattr("port_type")?.extract()?,
            layer: interop::layer_info(&port.getattr("layer_info")?)?,
            transform: interop::trans(&port.getattr("trans")?)?,
        });
    }
    Ok(MarkerCell {
        name: cell.getattr("name")?.extract()?,
        factory: factory(cell)?,
        ports,
    })
}
fn connectivity(
    kcl: &Bound<'_, PyAny>,
    value: Option<&Bound<'_, PyAny>>,
) -> PyResult<Vec<Vec<db::LayerInfo>>> {
    let value = match value {
        Some(value) if value.is_truthy()? => value.clone(),
        _ => kcl.getattr("connectivity")?,
    };
    value
        .try_iter()?
        .map(|group| {
            group?
                .try_iter()?
                .map(|info| interop::layer_info(&info?))
                .collect()
        })
        .collect()
}
fn mark_types(value: Option<Vec<String>>) -> Vec<String> {
    value.unwrap_or_else(|| vec!["electrical".into(), "RF".into(), "DC".into()])
}
#[pyfunction]
#[pyo3(signature=(cell,mark_port_types=None,connectivity=None,port_mapping=None))]
fn l2n_elec(
    py: Python<'_>,
    cell: &Bound<'_, PyAny>,
    mark_port_types: Option<Vec<String>>,
    connectivity: Option<&Bound<'_, PyAny>>,
    port_mapping: Option<&Bound<'_, PyAny>>,
) -> PyResult<Py<PyAny>> {
    let kcl = cell.getattr("kcl")?;
    let owner = kcl.getattr("layout")?;
    let layout = interop::layout(&owner)?;
    let root = root_id(&layout, cell)?;
    let cells = hierarchy(&layout, root, &kcl)?
        .iter()
        .map(marker_cell)
        .collect::<PyResult<Vec<_>>>()?;
    let mapping: PortMapping = port_mapping
        .map(pythonize::depythonize)
        .transpose()?
        .unwrap_or_default();
    let result = electrical::l2n_elec(
        &layout,
        root,
        &cells,
        &mark_types(mark_port_types),
        &self::connectivity(&kcl, connectivity)?,
        &mapping,
    )
    .map_err(domain_error)?;
    interop::wrap_extraction(py, result)
}
fn equivalents(cell: &Bound<'_, PyAny>) -> PyResult<Option<Vec<Vec<String>>>> {
    let own = cell.getattr("lvs_equivalent_ports")?;
    if let Some(factory) = factory(cell)? {
        // Factory registries and proxy metadata are KFactory-owned Python data.
        let mut source = cell.clone();
        while source.call_method0("is_library_cell")?.is_truthy()? {
            source = source.getattr("library_cell")?;
        }
        let registry = if cell.getattr("virtual")?.is_truthy()? {
            "virtual_factories"
        } else {
            "factories"
        };
        source
            .getattr("kcl")?
            .getattr(registry)?
            .get_item(factory)?
            .getattr("lvs_equivalent_ports")?
            .extract()
    } else if own.is_truthy()? {
        own.extract()
    } else {
        Ok(None)
    }
}
fn instance_input(py: Python<'_>, value: &Bound<'_, PyAny>) -> PyResult<InstanceInput> {
    let cell = value.getattr("cell")?;
    let name: String = value.getattr("name")?.extract()?;
    let placed_cell: String = cell.getattr("name")?.extract()?;
    let component = factory(&cell)?.unwrap_or_else(|| placed_cell.clone());
    let kcl = if cell.call_method0("is_library_cell")?.is_truthy()? {
        cell.call_method0("library")?
            .call_method0("name")?
            .extract()?
    } else {
        cell.getattr("kcl")?.getattr("name")?.extract()?
    };
    let settings =
        settings::serialize_setting(py, &cell.getattr("settings")?.call_method0("model_dump")?)?;
    let named = value.call_method0("is_named")?.is_truthy()?;
    let info = if named {
        match value.getattr("info") {
            Ok(info) if !info.is_none() => {
                pythonize::depythonize(&info.call_method0("model_dump")?)?
            }
            Ok(_) => Default::default(),
            Err(error) if error.is_instance_of::<pyo3::exceptions::PyAttributeError>(py) => {
                Default::default()
            }
            Err(error) => return Err(error),
        }
    } else {
        Default::default()
    };
    Ok(InstanceInput {
        native: interop::owned_instance(&value.getattr("instance")?)?,
        name,
        cell: placed_cell,
        component,
        kcl,
        settings: pythonize::depythonize(settings.bind(py))?,
        info,
        named,
        purpose: value.getattr("purpose")?.extract()?,
        na: value.getattr("na")?.extract()?,
        nb: value.getattr("nb")?.extract()?,
    })
}
#[pyfunction]
fn _create_inst_entry(
    py: Python<'_>,
    nl: &Bound<'_, PyAny>,
    inst: &Bound<'_, PyAny>,
) -> PyResult<()> {
    let cell = inst.getattr("cell")?;
    let component = factory(&cell)?.unwrap_or(cell.getattr("name")?.extract()?);
    let kcl = if cell.call_method0("is_library_cell")?.is_truthy()? {
        cell.call_method0("library")?.call_method0("name")?
    } else {
        cell.getattr("kcl")?.getattr("name")?
    };
    let settings =
        settings::serialize_setting(py, &cell.getattr("settings")?.call_method0("model_dump")?)?;
    let kwargs = PyDict::new(py);
    if inst.call_method0("is_named")?.is_truthy()? {
        match inst.getattr("info") {
            Ok(info) if !info.is_none() => {
                kwargs.set_item("info", info.call_method0("model_dump")?)?
            }
            Ok(_) => {}
            Err(error) if error.is_instance_of::<pyo3::exceptions::PyAttributeError>(py) => {}
            Err(error) => return Err(error),
        }
    }
    // Model values belong to the separate engine-free extension. This private
    // compatibility adapter dispatches its existing native mutation boundary.
    nl.call_method(
        "create_inst",
        (
            inst.getattr("name")?,
            kcl,
            component,
            settings,
            inst.getattr("na")?,
            inst.getattr("nb")?,
        ),
        Some(&kwargs),
    )?;
    Ok(())
}

#[pyfunction]
fn _placement_for(py: Python<'_>, inst: &Bound<'_, PyAny>) -> PyResult<Py<PyAny>> {
    let transform = interop::dcplx_trans(&inst.getattr("dcplx_trans")?)?;
    let native = inst.getattr("instance")?;
    // The historical helper also accepts structural test/user stand-ins. Native
    // layout instances always use the checked Rust bridge.
    let bounds = if native.get_type().module()?.to_str()?.starts_with("rlayout") {
        interop::instance_bounds(&native)?
    } else {
        let bounds = native.call_method0("dbbox")?;
        db::DBox::new(
            bounds.getattr("left")?.extract()?,
            bounds.getattr("bottom")?.extract()?,
            bounds.getattr("right")?.extract()?,
            bounds.getattr("top")?.extract()?,
        )
        .map_err(interop::native_error)?
    };
    model(
        py,
        "Placement",
        &domain::placement(transform, bounds).map_err(domain_error)?,
    )
}
#[pyfunction]
#[pyo3(signature=(cell,*,wrap_kdb_instance,port_types=None,mark_port_types=None,connectivity=None,equivalent_ports=None,ignore_unnamed=false,exclude_purposes=None,allow_width_mismatch=false,include_placement=false,flatten=None))]
fn extract(
    py: Python<'_>,
    cell: &Bound<'_, PyAny>,
    wrap_kdb_instance: &Bound<'_, PyAny>,
    port_types: Option<Vec<String>>,
    mark_port_types: Option<Vec<String>>,
    connectivity: Option<&Bound<'_, PyAny>>,
    equivalent_ports: Option<&Bound<'_, PyAny>>,
    ignore_unnamed: bool,
    exclude_purposes: Option<Vec<String>>,
    allow_width_mismatch: bool,
    include_placement: bool,
    flatten: Option<&Bound<'_, PyAny>>,
) -> PyResult<Py<PyAny>> {
    let kcl = cell.getattr("kcl")?;
    let owner = kcl.getattr("layout")?;
    let layout = interop::layout(&owner)?;
    let root = root_id(&layout, cell)?;
    let hierarchy = hierarchy(&layout, root, &kcl)?;
    let port_types = port_types.unwrap_or_else(|| vec!["optical".into()]);
    let mut cells = Vec::new();
    let mut markers = Vec::new();
    for cell in hierarchy {
        markers.push(marker_cell(&cell)?);
        let (optical_ports, optical_instances) = ports::optical_inputs(&cell, &port_types)?;
        cells.push(CellInput {
            id: root_id(&layout, &cell)?,
            name: cell.getattr("name")?.extract()?,
            ports: cell
                .getattr("ports")?
                .try_iter()?
                .map(|p| p?.getattr("name")?.extract())
                .collect::<PyResult<_>>()?,
            optical_ports,
            optical_instances,
            instances: cell
                .getattr("insts")?
                .try_iter()?
                .map(|v| instance_input(py, &v?))
                .collect::<PyResult<_>>()?,
            equivalents: if equivalent_ports.is_none() {
                equivalents(&cell)?
            } else {
                None
            },
        });
    }
    let flatten = match flatten {
        Some(v) if v.is_truthy()? => {
            if v.is_instance_of::<PyBool>() {
                Flatten::All
            } else {
                Flatten::Cells(v.extract()?)
            }
        }
        _ => Flatten::No,
    };
    let options = Options {
        port_types,
        mark_port_types: mark_types(mark_port_types),
        connectivity: self::connectivity(&kcl, connectivity)?,
        equivalent_ports: equivalent_ports
            .map(pythonize::depythonize::<IndexMap<String, Vec<Vec<String>>>>)
            .transpose()?,
        ignore_unnamed,
        exclude_purposes: exclude_purposes.unwrap_or_default(),
        allow_width_mismatch,
        include_placement,
        flatten,
    };
    let mut prepared =
        domain::prepare(&layout, root, &markers, cells, options).map_err(domain_error)?;
    let mut names = Vec::new();
    for matched in prepared.take_matches() {
        let instance = interop::wrap_instance(&owner, matched.instance)?;
        names.push(
            wrap_kdb_instance
                .call1((instance,))?
                .getattr("name")?
                .extract()?,
        );
    }
    let output = prepared.finish(&names).map_err(domain_error)?;
    let result = PyDict::new(py);
    for (name, data) in output.netlists {
        let netlist = kfnetlist_core::Netlist {
            instances: data.instances,
            nets: data.nets,
            ports: data.ports,
        };
        let value = if output.placed {
            model(
                py,
                "PlacedNetlist",
                &kfnetlist_core::PlacedNetlist::new(netlist, data.extras),
            )?
        } else {
            model(py, "Netlist", &netlist)?
        };
        result.set_item(name, value)?;
    }
    Ok(result.into_any().unbind())
}
pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(l2n_elec, module)?)?;
    module.add_function(wrap_pyfunction!(_create_inst_entry, module)?)?;
    module.add_function(wrap_pyfunction!(_placement_for, module)?)?;
    module.add_function(wrap_pyfunction!(extract, module)?)
}
