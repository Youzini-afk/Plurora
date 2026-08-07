use std::fs;
use std::path::Path;

use plurora_runtime::KernelMethod;
use serde_json::Value;

use super::defs::normalize_schema;
use super::methods::method_schema;

pub(crate) fn filename(name: &str) -> String {
    name.replace('/', "__")
}

pub(crate) fn write_json(path: impl AsRef<Path>, value: &Value) -> anyhow::Result<()> {
    let mut value = value.clone();
    normalize_schema(&mut value);
    let bytes = serde_json::to_vec_pretty(&value)?;
    fs::write(path, [bytes, b"\n".to_vec()].concat())?;
    Ok(())
}

pub(crate) fn write_method(
    out: &Path,
    method: KernelMethod,
    params: Value,
    result: Value,
) -> anyhow::Result<()> {
    write_json(
        out.join("methods")
            .join(format!("{}.schema.json", method.id())),
        &method_schema(method, params, result),
    )
}
