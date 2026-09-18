//! Python conversion at the boundary of the pure-Rust flattening engine.

use std::collections::HashMap;

use indexmap::IndexMap;
use pyo3::exceptions::PyTypeError;
use pyo3::prelude::*;
use pyo3::types::PyDict;

use crate::netlist::Netlist;
use crate::placement::PlacedNetlist;

/// Read a `{cell name: Netlist | PlacedNetlist}` mapping into core values.
pub(crate) fn read_netlists(
    obj: &Bound<'_, PyAny>,
) -> PyResult<HashMap<String, kfnetlist_core::NetlistData>> {
    let dict = obj.cast::<PyDict>().map_err(|_| {
        PyTypeError::new_err("netlists must be a dict of {cell name: Netlist | PlacedNetlist}")
    })?;
    let mut out = HashMap::with_capacity(dict.len());
    for (key, value) in dict.iter() {
        let name: String = key
            .extract()
            .map_err(|_| PyTypeError::new_err("netlists keys must be cell names (str)"))?;
        out.insert(name, read_netlist(&value)?);
    }
    Ok(out)
}

fn read_netlist(obj: &Bound<'_, PyAny>) -> PyResult<kfnetlist_core::NetlistData> {
    // PlacedNetlist first: it is a Python subclass of Netlist.
    if let Ok(placed) = obj.cast::<PlacedNetlist>() {
        let child = placed.borrow();
        let base: &Netlist = child.as_ref();
        return Ok(kfnetlist_core::NetlistData {
            instances: base.instances.clone(),
            nets: base.nets.clone(),
            ports: base.ports.clone(),
            extras: child.extras.clone(),
        });
    }
    let plain = obj
        .cast::<Netlist>()
        .map_err(|_| PyTypeError::new_err("netlists values must be Netlist or PlacedNetlist"))?
        .borrow();
    Ok(kfnetlist_core::NetlistData {
        instances: plain.instances.clone(),
        nets: plain.nets.clone(),
        ports: plain.ports.clone(),
        extras: IndexMap::new(),
    })
}

pub(crate) fn emit_warnings(py: Python<'_>, warnings: Vec<String>) -> PyResult<()> {
    for warning in warnings {
        crate::warn(py, &warning)?;
    }
    Ok(())
}

#[pyfunction]
#[pyo3(signature=(netlists,cells=None,*,exclude=None,instance_cell_maps=None,recursive=true,allow_unconnected_ports=false,warn_skipped=false,separator=".".to_owned()))]
pub(crate) fn flatten_netlists(
    py: Python<'_>,
    netlists: &Bound<'_, PyAny>,
    cells: Option<Vec<String>>,
    exclude: Option<Vec<String>>,
    instance_cell_maps: Option<HashMap<String, HashMap<String, String>>>,
    recursive: bool,
    allow_unconnected_ports: bool,
    warn_skipped: bool,
    separator: String,
) -> PyResult<Py<PyAny>> {
    let dict = py
        .import("builtins")?
        .getattr("dict")?
        .call1((netlists,))?
        .cast_into::<PyDict>()?;
    let mut placed = HashMap::new();
    let mut values = IndexMap::new();
    for (key, value) in dict.iter() {
        let name: String = key.extract()?;
        placed.insert(name.clone(), value.is_instance_of::<PlacedNetlist>());
        values.insert(name, read_netlist(&value)?);
    }
    let options = kfnetlist_core::FlattenOptions::new(
        cells,
        exclude,
        recursive,
        allow_unconnected_ports,
        warn_skipped,
        separator,
    );
    let (output, warnings) = kfnetlist_core::flatten::flatten_netlists(
        values,
        &instance_cell_maps.unwrap_or_default(),
        &options,
    )
    .map_err(crate::core_error)?;
    emit_warnings(py, warnings)?;
    let result = PyDict::new(py);
    for (name, data) in output {
        let plain = Netlist(kfnetlist_core::Netlist {
            instances: data.instances,
            nets: data.nets,
            ports: data.ports,
        });
        let value = if placed[&name] {
            Py::new(py, PlacedNetlist::init_from(plain, data.extras))?.into_any()
        } else {
            Py::new(py, plain)?.into_any()
        };
        result.set_item(name, value)?;
    }
    Ok(result.into_any().unbind())
}
