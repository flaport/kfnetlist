//! Binding registration for the single RLayout engine image.
use pyo3::prelude::*;
use rlayout_python::interop;

/// Borrow a real live layout, convert layer identities, and wrap the Rust result.
#[pyfunction]
fn connected_geometry(
    py: Python<'_>,
    layout: &Bound<'_, PyAny>,
    cell: &Bound<'_, PyAny>,
    connectivity: Vec<Vec<Bound<'_, PyAny>>>,
) -> PyResult<Py<PyAny>> {
    let layout = interop::layout(layout)?;
    let root = layout.cell_id(cell)?;
    let connectivity = connectivity
        .iter()
        .map(|group| {
            group
                .iter()
                .map(|layer| layout.layer_id(layer))
                .collect::<PyResult<Vec<_>>>()
        })
        .collect::<PyResult<Vec<_>>>()?;
    let result = kfnetlist_extract::connected_geometry(&layout, root, &connectivity)
        .map_err(interop::native_error)?;
    interop::wrap_extraction(py, result)
}

/// Register in the host extension; never load a second copy of the engine.
pub fn register(parent: &Bound<'_, PyModule>) -> PyResult<()> {
    let module = PyModule::new(parent.py(), "_kfnetlist_extract")?;
    module.add_function(wrap_pyfunction!(connected_geometry, &module)?)?;
    parent.add_submodule(&module)
}
