use pyo3::prelude::*;

#[cfg(test)]
mod tests {
    use super::*; // bring the module under test into scope

    #[test]
    fn it_works() {
        let result: usize = add(2, 2);
        assert_eq!(result, 4);
    }
}

fn add(a: usize, b: usize) -> usize {
    a + b
}

/// Formats the sum of two numbers as string.
#[pyfunction]
fn sum_as_string(a: usize, b: usize) -> PyResult<String> {
    Ok((add(a, b)).to_string())
}

/// A Python module implemented in Rust.
#[pymodule]
fn cartconf(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(sum_as_string, m)?)?;
    Ok(())
}
