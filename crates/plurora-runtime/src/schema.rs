use serde_json::Value;

pub fn validate_json_schema_subset(schema: &Value, value: &Value) -> anyhow::Result<()> {
    if schema.is_null() || schema == &Value::Object(Default::default()) {
        return Ok(());
    }

    let Some(schema_object) = schema.as_object() else {
        anyhow::bail!("schema must be an object or null");
    };

    if let Some(expected) = schema_object.get("const") {
        if value != expected {
            anyhow::bail!("value does not match schema const");
        }
    }

    if let Some(one_of) = schema_object.get("oneOf") {
        let Some(branches) = one_of.as_array() else {
            anyhow::bail!("schema oneOf must be an array");
        };
        if branches.is_empty() {
            anyhow::bail!("schema oneOf must contain at least one branch");
        }
        let matches = branches
            .iter()
            .filter(|branch| validate_json_schema_subset(branch, value).is_ok())
            .count();
        if matches != 1 {
            anyhow::bail!("value must match exactly one schema oneOf branch; matched {matches}");
        }
    }

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

    if let Some(properties) = schema_object.get("properties") {
        let Some(properties) = properties.as_object() else {
            anyhow::bail!("schema properties must be an object");
        };
        if let Some(value_object) = value.as_object() {
            for (field, property_schema) in properties {
                if let Some(property_value) = value_object.get(field) {
                    validate_json_schema_subset(property_schema, property_value)
                        .map_err(|error| anyhow::anyhow!("field '{field}' is invalid: {error}"))?;
                }
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

    #[test]
    fn validates_one_of_properties_and_const_discriminators() {
        let schema = json!({
            "oneOf": [
                {
                    "type": "object",
                    "required": ["kind", "bundle_id"],
                    "properties": {"kind": {"type": "string", "const": "work_bundle"}}
                },
                {
                    "type": "object",
                    "required": ["kind", "reason"],
                    "properties": {"kind": {"type": "string", "const": "sharing_lab_rejected"}}
                }
            ]
        });

        validate_json_schema_subset(
            &schema,
            &json!({"kind": "work_bundle", "bundle_id": "sha256:abc"}),
        )
        .unwrap();
        validate_json_schema_subset(
            &schema,
            &json!({"kind": "sharing_lab_rejected", "reason": "unsafe"}),
        )
        .unwrap();
        assert!(validate_json_schema_subset(
            &schema,
            &json!({"kind": "work_bundle_import", "bundle_id": "sha256:abc"}),
        )
        .is_err());
        assert!(validate_json_schema_subset(&schema, &json!({"kind": "work_bundle"})).is_err());
    }

    #[test]
    fn one_of_rejects_ambiguous_matches() {
        let schema = json!({"oneOf": [{"type": "object"}, {"type": "object"}]});
        assert!(validate_json_schema_subset(&schema, &json!({})).is_err());
    }
}
