use std::path::PathBuf;

use pyo3::exceptions::PyOSError;
use pyo3::prelude::*;

fn diff_sources(old: &str, new: &str) -> String {
    let filter = |b: &typdiff::Block| !matches!(b, typdiff::Block::Parbreak);
    let old_blocks: Vec<_> = typdiff::parse::parse(old)
        .into_iter()
        .filter(filter)
        .collect();
    let new_blocks: Vec<_> = typdiff::parse::parse(new)
        .into_iter()
        .filter(filter)
        .collect();
    let diff_results = typdiff::diff::diff(&old_blocks, &new_blocks);
    typdiff::render::render(&diff_results)
}

/// Diff two Typst documents given as source strings, returning diff markup.
#[pyfunction]
fn diff(old: &str, new: &str) -> String {
    diff_sources(old, new)
}

/// Diff two Typst documents given as file paths, returning diff markup.
#[pyfunction]
fn diff_files(old_path: PathBuf, new_path: PathBuf) -> PyResult<String> {
    let old = std::fs::read_to_string(&old_path).map_err(|e| PyOSError::new_err(e.to_string()))?;
    let new = std::fs::read_to_string(&new_path).map_err(|e| PyOSError::new_err(e.to_string()))?;
    Ok(diff_sources(&old, &new))
}

#[pymodule]
fn _typdiff(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(diff, m)?)?;
    m.add_function(wrap_pyfunction!(diff_files, m)?)?;
    Ok(())
}
