//! Python metadata conversion; all geometry and adjacency run in the Rust crate.
use crate::{domain_error, model};
use kfnetlist_extract::{
    geometry::OpticalInstance,
    ports::{self, CheckOptions},
    Port, PortTransform,
};
use pyo3::{exceptions::PyAssertionError, prelude::*};
use rlayout_python::interop;

#[derive(Default)]
pub(crate) struct CrossSections<'py> {
    values: Vec<Bound<'py, PyAny>>,
}
impl<'py> CrossSections<'py> {
    fn intern(&mut self, value: &Bound<'py, PyAny>) -> PyResult<usize> {
        for (index, existing) in self.values.iter().enumerate() {
            if existing.eq(value)? {
                return Ok(index);
            }
        }
        self.values.push(value.clone());
        Ok(self.values.len() - 1)
    }
}
fn read_port<'py>(
    base: &Bound<'py, PyAny>,
    xs: &Bound<'py, PyAny>,
    name: String,
    port_type: String,
    sections: &mut CrossSections<'py>,
) -> PyResult<Port> {
    let trans = base.getattr("trans")?;
    let physical = base.getattr("dcplx_trans")?;
    let trans = if trans.is_none() {
        None
    } else {
        Some(interop::trans(&trans)?)
    };
    let physical = if physical.is_none() {
        None
    } else {
        Some(interop::dcplx_trans(&physical)?)
    };
    let transform = match (trans, physical) {
        (Some(a), Some(b)) => PortTransform::Both(a, b),
        (Some(a), None) => PortTransform::Grid(a),
        (None, Some(b)) => PortTransform::Physical(b),
        _ => {
            return Err(PyAssertionError::new_err(
                "port has neither trans nor dcplx_trans",
            ))
        }
    };
    Ok(Port {
        name,
        port_type,
        transform,
        layer: interop::layer_info(&xs.getattr("main_layer")?)?,
        width: xs.getattr("width")?.extract()?,
        dbu: base.getattr("kcl")?.getattr("dbu")?.extract()?,
        cross_section: sections.intern(xs)?,
    })
}
pub(crate) fn wrapper_ports<'py>(
    owner: &Bound<'py, PyAny>,
    sections: &mut CrossSections<'py>,
    types: Option<&[String]>,
) -> PyResult<Vec<Port>> {
    let mut result = Vec::new();
    for port in owner.getattr("ports")?.try_iter()? {
        let port = port?;
        let kind: String = port.getattr("port_type")?.extract()?;
        if types.is_some_and(|types| !types.contains(&kind)) {
            continue;
        }
        result.push(read_port(
            &port.getattr("base")?,
            &port.getattr("cross_section")?.getattr("base")?,
            port.getattr("name")?.extract()?,
            kind,
            sections,
        )?);
    }
    Ok(result)
}
pub(crate) fn optical_inputs<'py>(
    cell: &Bound<'py, PyAny>,
    types: &[String],
) -> PyResult<(Vec<Port>, Vec<OpticalInstance>)> {
    let mut sections = CrossSections::default();
    let ports = wrapper_ports(cell, &mut sections, Some(types))?;
    let mut instances = Vec::new();
    for instance in cell.getattr("insts")?.try_iter()? {
        let instance = instance?;
        let na: i64 = instance.getattr("na")?.extract()?;
        let nb: i64 = instance.getattr("nb")?.extract()?;
        instances.push(OpticalInstance {
            name: instance.getattr("name")?.extract()?,
            na,
            nb,
            ports: wrapper_ports(&instance, &mut sections, Some(types))?,
            array: if na > 1 || nb > 1 {
                Some(interop::instance_array(&instance.getattr("instance")?)?)
            } else {
                None
            },
        });
    }
    Ok((ports, instances))
}
#[pyfunction]
#[pyo3(signature=(p1,p2,*,tolerance=0.1,angle_tolerance=0.01,snapped=false))]
fn check_connection(
    p1: &Bound<'_, PyAny>,
    p2: &Bound<'_, PyAny>,
    tolerance: f64,
    angle_tolerance: f64,
    snapped: bool,
) -> PyResult<u8> {
    let mut sections = CrossSections::default();
    let a = read_port(
        p1,
        &p1.getattr("cross_section")?,
        String::new(),
        p1.getattr("port_type")?.extract()?,
        &mut sections,
    )?;
    let b = read_port(
        p2,
        &p2.getattr("cross_section")?,
        String::new(),
        p2.getattr("port_type")?.extract()?,
        &mut sections,
    )?;
    ports::check_connection(
        &a,
        &b,
        CheckOptions {
            tolerance,
            angle_tolerance,
            snapped,
        },
    )
    .map_err(domain_error)
}
#[pyfunction]
#[pyo3(signature=(cell,port_types=None,*,allow_width_mismatch=false))]
fn get_optical_nets(
    py: Python<'_>,
    cell: &Bound<'_, PyAny>,
    port_types: Option<Vec<String>>,
    allow_width_mismatch: bool,
) -> PyResult<Vec<Py<PyAny>>> {
    let types = port_types.unwrap_or_else(|| vec!["optical".into()]);
    let (ports, instances) = optical_inputs(cell, &types)?;
    kfnetlist_extract::geometry::optical_nets(ports, &instances, &types, allow_width_mismatch)
        .map_err(domain_error)?
        .iter()
        .map(|net| model(py, "Net", net))
        .collect()
}
pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(check_connection, module)?)?;
    module.add_function(wrap_pyfunction!(get_optical_nets, module)?)
}
