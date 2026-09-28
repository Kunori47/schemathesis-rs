use rand::RngExt;
use serde_json::{json, Value};

#[derive(Debug, Clone)]
pub struct ValueGenerator {
    seed: Option<u64>,
}

impl ValueGenerator {
    pub fn new() -> Self {
        Self { seed: None }
    }

    pub fn with_seed(seed: u64) -> Self {
        Self { seed: Some(seed) }
    }

    pub fn seed(&self) -> Option<u64> {
        self.seed
    }

    pub fn generate_from_schema(&self, schema: &Value) -> Value {
        self.generate_internal(schema, 0)
    }

    fn generate_internal(&self, schema: &Value, depth: usize) -> Value {
        if depth > 4 {
            return Value::Null;
        }

        // Handle enum
        if let Some(enum_vals) = schema.get("enum").and_then(Value::as_array) {
            if !enum_vals.is_empty() {
                let mut rng = rand::rng();
                let idx = rng.random_range(0..enum_vals.len());
                return enum_vals[idx].clone();
            }
        }

        let schema_type = schema.get("type").and_then(Value::as_str).unwrap_or("string");

        match schema_type {
            "integer" => {
                let min = schema.get("minimum").and_then(Value::as_i64).unwrap_or(0);
                let max = schema.get("maximum").and_then(Value::as_i64).unwrap_or(1000);
                let mut rng = rand::rng();
                let val = if min <= max {
                    rng.random_range(min..=max)
                } else {
                    min
                };
                json!(val)
            }
            "number" => {
                let min = schema.get("minimum").and_then(Value::as_f64).unwrap_or(0.0);
                let max = schema.get("maximum").and_then(Value::as_f64).unwrap_or(1000.0);
                let mut rng = rand::rng();
                let val = if min <= max {
                    rng.random_range(min..=max)
                } else {
                    min
                };
                json!(val)
            }
            "boolean" => {
                let mut rng = rand::rng();
                json!(rng.random_bool(0.5))
            }
            "string" => {
                let format = schema.get("format").and_then(Value::as_str);
                match format {
                    Some("uuid") => json!("a1b2c3d4-e5f6-7a8b-9c0d-1e2f3a4b5c6d"),
                    Some("email") => json!("test.user@example.com"),
                    Some("date") => json!("2026-09-28"),
                    Some("date-time") => json!("2026-09-28T12:00:00Z"),
                    _ => {
                        let min_len = schema.get("minLength").and_then(Value::as_u64).unwrap_or(1) as usize;
                        let max_len = schema.get("maxLength").and_then(Value::as_u64).unwrap_or(16) as usize;
                        let mut rng = rand::rng();
                        let target_len = if min_len <= max_len {
                            rng.random_range(min_len..=max_len)
                        } else {
                            min_len
                        };
                        let chars: Vec<char> = "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789".chars().collect();
                        let mut s = String::with_capacity(target_len);
                        for _ in 0..target_len {
                            let idx = rng.random_range(0..chars.len());
                            s.push(chars[idx]);
                        }
                        json!(s)
                    }
                }
            }
            "array" => {
                let items_schema = schema.get("items").unwrap_or(&Value::Null);
                let min_items = schema.get("minItems").and_then(Value::as_u64).unwrap_or(1) as usize;
                let max_items = schema.get("maxItems").and_then(Value::as_u64).unwrap_or(3) as usize;
                let mut rng = rand::rng();
                let count = if min_items <= max_items {
                    rng.random_range(min_items..=max_items)
                } else {
                    min_items
                };

                let mut arr = Vec::with_capacity(count);
                for _ in 0..count {
                    arr.push(self.generate_internal(items_schema, depth + 1));
                }
                Value::Array(arr)
            }
            "object" => {
                let mut map = serde_json::Map::new();
                if let Some(props) = schema.get("properties").and_then(Value::as_object) {
                    for (k, prop_schema) in props {
                        map.insert(k.clone(), self.generate_internal(prop_schema, depth + 1));
                    }
                }
                Value::Object(map)
            }
            _ => json!("sample_value"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_integer_within_bounds() {
        let generator = ValueGenerator::new();
        let schema = json!({
            "type": "integer",
            "minimum": 10,
            "maximum": 20
        });

        for _ in 0..20 {
            let val = generator.generate_from_schema(&schema);
            let num = val.as_i64().expect("should be integer");
            assert!(num >= 10 && num <= 20, "Generated {} out of bounds", num);
        }
    }

    #[test]
    fn test_generate_string_with_format() {
        let generator = ValueGenerator::new();
        let schema = json!({
            "type": "string",
            "format": "uuid"
        });

        let val = generator.generate_from_schema(&schema);
        assert_eq!(val.as_str().unwrap(), "a1b2c3d4-e5f6-7a8b-9c0d-1e2f3a4b5c6d");
    }

    #[test]
    fn test_generate_enum_value() {
        let generator = ValueGenerator::new();
        let schema = json!({
            "type": "string",
            "enum": ["active", "inactive", "pending"]
        });

        for _ in 0..10 {
            let val = generator.generate_from_schema(&schema);
            let s = val.as_str().unwrap();
            assert!(["active", "inactive", "pending"].contains(&s));
        }
    }

    #[test]
    fn test_generate_object_with_nested_properties() {
        let generator = ValueGenerator::new();
        let schema = json!({
            "type": "object",
            "properties": {
                "id": { "type": "integer", "minimum": 1, "maximum": 5 },
                "name": { "type": "string", "minLength": 3, "maxLength": 8 }
            }
        });

        let val = generator.generate_from_schema(&schema);
        assert!(val.is_object());
        let obj = val.as_object().unwrap();
        assert!(obj.contains_key("id"));
        assert!(obj.contains_key("name"));
    }
}
