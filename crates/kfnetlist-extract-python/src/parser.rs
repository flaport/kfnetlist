//! Translate Python containers and native wrapper guards around Rust traversal.
use crate::{domain_error, wire};
use kfnetlist_extract::{
    db,
    parser::{self, Filters},
};
use pyo3::{prelude::*, types::PyDict};
use rlayout_python::interop;
use std::collections::HashSet;

fn layers(value: Option<&Bound<'_, PyAny>>) -> PyResult<Option<HashSet<db::LayerInfo>>> {
    value
        .filter(|v| !v.is_none())
        .map(|v| {
            v.try_iter()?
                .map(|item| interop::layer_info(&item?))
                .collect()
        })
        .transpose()
}
fn filters(
    include_layers: Option<&Bound<'_, PyAny>>,
    exclude_layers: Option<&Bound<'_, PyAny>>,
    include_instances: Option<HashSet<String>>,
    exclude_instances: Option<HashSet<String>>,
) -> PyResult<Filters> {
    Ok(Filters {
        include_layers: layers(include_layers)?,
        exclude_layers: layers(exclude_layers)?,
        include_instances,
        exclude_instances,
    })
}
fn wrap_regions(py: Python<'_>, regions: parser::LayerRegions) -> PyResult<Py<PyAny>> {
    let dict = PyDict::new(py);
    for (info, region) in regions {
        dict.set_item(
            interop::wrap_layer_info(py, info)?,
            interop::wrap_region(py, region)?,
        )?;
    }
    Ok(dict.into_any().unbind())
}
fn borrow_regions<'py>(
    value: &Bound<'py, PyAny>,
) -> PyResult<Vec<(db::LayerInfo, interop::RegionRef<'py>)>> {
    value
        .cast::<PyDict>()?
        .iter()
        .map(|(info, region)| Ok((interop::layer_info(&info)?, interop::region(&region)?)))
        .collect()
}
#[pyfunction]
fn _layer_display_name(info: &Bound<'_, PyAny>) -> PyResult<String> {
    parser::layer_display_name(&interop::layer_info(info)?).map_err(domain_error)
}
#[pyfunction]
fn _discover_layer_regions(py: Python<'_>, l2n: &Bound<'_, PyAny>) -> PyResult<Py<PyAny>> {
    wrap_regions(
        py,
        parser::discover_layers(&mut interop::extraction(l2n)?).map_err(domain_error)?,
    )
}
#[pyfunction]
fn _net_shapes_by_layer(
    py: Python<'_>,
    net: &Bound<'_, PyAny>,
    l2n: &Bound<'_, PyAny>,
    layer_regions: &Bound<'_, PyAny>,
) -> PyResult<Py<PyAny>> {
    let layers = borrow_regions(layer_regions)?;
    let views = layers
        .iter()
        .map(|(i, r)| (i.clone(), &**r))
        .collect::<Vec<_>>();
    wrap_regions(
        py,
        parser::net_shapes(&interop::net(net)?, &interop::extraction(l2n)?, &views)
            .map_err(domain_error)?,
    )
}
#[pyfunction]
#[pyo3(signature=(net,l2n,layer_regions,subc_id_filter,include_layers,exclude_layers))]
fn _serialize_net(
    py: Python<'_>,
    net: &Bound<'_, PyAny>,
    l2n: Option<&Bound<'_, PyAny>>,
    layer_regions: &Bound<'_, PyAny>,
    subc_id_filter: Option<HashSet<u64>>,
    include_layers: Option<&Bound<'_, PyAny>>,
    exclude_layers: Option<&Bound<'_, PyAny>>,
) -> PyResult<Py<PyAny>> {
    let layers = borrow_regions(layer_regions)?;
    let views = layers
        .iter()
        .map(|(i, r)| (i.clone(), &**r))
        .collect::<Vec<_>>();
    let extraction = l2n.map(interop::extraction).transpose()?;
    let result = parser::serialize_net(
        &interop::net(net)?,
        extraction.as_deref(),
        &views,
        subc_id_filter.as_ref(),
        &filters(include_layers, exclude_layers, None, None)?,
    )
    .map_err(domain_error)?;
    wire(py, &result)
}
#[pyfunction]
#[pyo3(signature=(circuit,l2n,layer_regions,include_instances,exclude_instances,include_layers,exclude_layers))]
fn _serialize_circuit(
    py: Python<'_>,
    circuit: &Bound<'_, PyAny>,
    l2n: Option<&Bound<'_, PyAny>>,
    layer_regions: &Bound<'_, PyAny>,
    include_instances: Option<HashSet<String>>,
    exclude_instances: Option<HashSet<String>>,
    include_layers: Option<&Bound<'_, PyAny>>,
    exclude_layers: Option<&Bound<'_, PyAny>>,
) -> PyResult<Py<PyAny>> {
    let layers = borrow_regions(layer_regions)?;
    let views = layers
        .iter()
        .map(|(i, r)| (i.clone(), &**r))
        .collect::<Vec<_>>();
    let extraction = l2n.map(interop::extraction).transpose()?;
    wire(
        py,
        &parser::serialize_circuit(
            &interop::circuit(circuit)?,
            extraction.as_deref(),
            &views,
            &filters(
                include_layers,
                exclude_layers,
                include_instances,
                exclude_instances,
            )?,
        )
        .map_err(domain_error)?,
    )
}
#[pyfunction]
#[pyo3(signature=(l2n,*,flatten=false,include_layers=None,exclude_layers=None,include_instances=None,exclude_instances=None))]
fn parse_l2n(
    py: Python<'_>,
    l2n: &Bound<'_, PyAny>,
    flatten: bool,
    include_layers: Option<&Bound<'_, PyAny>>,
    exclude_layers: Option<&Bound<'_, PyAny>>,
    include_instances: Option<HashSet<String>>,
    exclude_instances: Option<HashSet<String>>,
) -> PyResult<Py<PyAny>> {
    wire(
        py,
        &parser::parse_l2n(
            &mut interop::extraction(l2n)?,
            flatten,
            &filters(
                include_layers,
                exclude_layers,
                include_instances,
                exclude_instances,
            )?,
        )
        .map_err(domain_error)?,
    )
}
#[pyfunction]
#[pyo3(signature=(l2n,*,flatten=false,include_layers=None,exclude_layers=None,include_instances=None,exclude_instances=None,indent=Some(2)))]
fn l2n_to_json(
    py: Python<'_>,
    l2n: &Bound<'_, PyAny>,
    flatten: bool,
    include_layers: Option<&Bound<'_, PyAny>>,
    exclude_layers: Option<&Bound<'_, PyAny>>,
    include_instances: Option<HashSet<String>>,
    exclude_instances: Option<HashSet<String>>,
    indent: Option<usize>,
) -> PyResult<String> {
    let value = parse_l2n(
        py,
        l2n,
        flatten,
        include_layers,
        exclude_layers,
        include_instances,
        exclude_instances,
    )?;
    // Standard-library JSON is a boundary encoding, preserving Python's exact
    // Unicode/float/indent spelling; the Rust tree is independently serializable.
    let options = PyDict::new(py);
    options.set_item("indent", indent)?;
    py.import("json")?
        .call_method("dumps", (value,), Some(&options))?
        .extract()
}
#[pyfunction]
#[pyo3(signature=(l2n,*,short_layers=None,circuit_name=None))]
fn detect_shorts(
    py: Python<'_>,
    l2n: &Bound<'_, PyAny>,
    short_layers: Option<&Bound<'_, PyAny>>,
    circuit_name: Option<&str>,
) -> PyResult<Vec<Py<PyAny>>> {
    let allowed = layers(short_layers)?;
    let results = kfnetlist_extract::shorts::detect_shorts(
        &mut interop::extraction(l2n)?,
        allowed.as_ref(),
        circuit_name,
    )
    .map_err(domain_error)?;
    let class = py
        .import("kfnetlist.extract._shorts")?
        .getattr("ShortResult")?;
    results
        .into_iter()
        .map(|v| {
            Ok(class
                .call1((
                    v.net_a,
                    v.net_b,
                    v.layer,
                    interop::wrap_region(py, v.overlap)?,
                ))?
                .unbind())
        })
        .collect()
}
pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    let fields = [
        ("net_a", "str"),
        ("net_b", "str"),
        ("layer", "str"),
        ("overlap", "rlayout.db.Region"),
    ];
    let class = module
        .py()
        .import("dataclasses")?
        .getattr("make_dataclass")?
        .call1(("ShortResult", fields))?;
    class.setattr("__module__", "kfnetlist.extract._shorts")?;
    module.add("ShortResult", class)?;
    module.add_function(wrap_pyfunction!(_layer_display_name, module)?)?;
    module.add_function(wrap_pyfunction!(_discover_layer_regions, module)?)?;
    module.add_function(wrap_pyfunction!(_net_shapes_by_layer, module)?)?;
    module.add_function(wrap_pyfunction!(_serialize_net, module)?)?;
    module.add_function(wrap_pyfunction!(_serialize_circuit, module)?)?;
    module.add_function(wrap_pyfunction!(parse_l2n, module)?)?;
    module.add_function(wrap_pyfunction!(l2n_to_json, module)?)?;
    module.add_function(wrap_pyfunction!(detect_shorts, module)?)
}
