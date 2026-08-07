use serde_json::Value;

pub fn validate_json_schema_subset(schema: &Value, value: &Value) -> anyhow::Result<()> {
    if schema.is_null() || schema == &Value::Object(Default::default()) {
        return Ok(());
    }

    let Some(schema_object) = schema.as_object() else {
        anyhow::bail!("schema must be an object or null");
    };

    if let Some(type_value) = schema_object.get("type") {
        let Some(type_name) = type_value.as_str() else {
            anyhow::bail!("schema type must be a string");
        };
        let matches = match type_name {
            "object" => value.is_object(),
            "array" => value.is_array(),
            "string" => value.is_string(),
            "number" => value.is_number(),
            "integer" => value.as_i64().is_some() || value.as_u64().is_some(),
            "boolean" => value.is_boolean(),
            "null" => value.is_null(),
            other => anyhow::bail!("unsupported schema type '{other}'"),
        };
        if !matches {
            anyhow::bail!("value does not match schema type '{type_name}'");
        }
    }

    if let Some(required) = schema_object.get("required") {
        let Some(required) = required.as_array() else {
            anyhow::bail!("schema required must be an array");
        };
        let Some(value_object) = value.as_object() else {
            anyhow::bail!("required fields need an object value");
        };
        for field in required {
            let Some(field) = field.as_str() else {
                anyhow::bail!("required field names must be strings");
            };
            if !value_object.contains_key(field) {
                anyhow::bail!("missing required field '{field}'");
            }
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn accepts_empty_schema() {
        validate_json_schema_subset(&json!({}), &json!({"anything": true})).unwrap();
    }

    #[test]
    fn rejects_missing_required_field() {
        let result =
            validate_json_schema_subset(&json!({"type": "object", "required": ["ok"]}), &json!({}));
        assert!(result.is_err());
    }
}
