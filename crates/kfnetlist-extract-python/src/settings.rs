use kfnetlist_extract::settings::Setting;
use pyo3::{
    prelude::*,
    types::{PyDict, PyList, PyTuple},
    IntoPyObjectExt,
};
use rlayout_python::interop;
fn read(value: &Bound<'_, PyAny>) -> PyResult<Setting<Py<PyAny>>> {
    if let Ok(values) = value.cast::<PyDict>() {
        return Ok(Setting::Map(
            values
                .iter()
                .map(|(key, value)| Ok((key.str()?.extract()?, read(&value)?)))
                .collect::<PyResult<_>>()?,
        ));
    }
    if let Ok(values) = value.cast::<PyList>() {
        return Ok(Setting::List(
            values.iter().map(|v| read(&v)).collect::<PyResult<_>>()?,
        ));
    }
    if let Ok(values) = value.cast::<PyTuple>() {
        return Ok(Setting::Tuple(
            values.iter().map(|v| read(&v)).collect::<PyResult<_>>()?,
        ));
    }
    if let Some((class, text)) = interop::setting_text(value)? {
        return Ok(Setting::Native { class, text });
    }
    // Opaque Python values pass through by identity, never enter geometry code.
    Ok(Setting::Opaque(value.clone().unbind()))
}
fn write(py: Python<'_>, value: Setting<Py<PyAny>>) -> PyResult<Py<PyAny>> {
    match value {
        Setting::Map(values) => {
            let dict = PyDict::new(py);
            for (k, v) in values {
                dict.set_item(k, write(py, v)?)?;
            }
            Ok(dict.into_any().unbind())
        }
        Setting::List(values) => Ok(PyList::new(
            py,
            values
                .into_iter()
                .map(|v| write(py, v))
                .collect::<PyResult<Vec<_>>>()?,
        )?
        .into_any()
        .unbind()),
        Setting::Tuple(values) => Ok(PyTuple::new(
            py,
            values
                .into_iter()
                .map(|v| write(py, v))
                .collect::<PyResult<Vec<_>>>()?,
        )?
        .into_any()
        .unbind()),
        Setting::Encoded(value) => value.into_py_any(py),
        Setting::Opaque(value) => Ok(value),
        Setting::Native { .. } => {
            unreachable!("encode consumes every native setting before output conversion")
        }
    }
}
#[pyfunction]
pub(crate) fn serialize_setting(py: Python<'_>, setting: &Bound<'_, PyAny>) -> PyResult<Py<PyAny>> {
    write(py, read(setting)?.encode())
}
pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(serialize_setting, module)?)
}
