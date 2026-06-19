//! # Python Tool Adapter
//!
//! Adapts Python callables to Rust Tool trait for registration.
//! Supports both synchronous and asynchronous Python functions.

use async_trait::async_trait;
use pyo3::exceptions::PyTypeError;
use pyo3::prelude::*;
use pyo3::types::{PyDict, PyList, PyString};
use pyo3::IntoPyObjectExt;
use serde_json::{json, Value as JsonValue};
use sh_layer2::{Layer2Result, Tool, ToolResult};
use std::sync::Arc;

/// Error type for Python tool operations
#[derive(Debug)]
pub enum PythonToolError {
    /// Python callable raised an exception
    PythonException(String),
    /// Type conversion failed
    TypeConversion(String),
    /// Argument validation failed
    ValidationError(String),
    /// GIL acquisition failed
    #[allow(dead_code)]
    GilError(String),
    /// Invalid callable
    InvalidCallable(String),
    /// PyO3 error
    PyO3Error(String),
}

impl std::fmt::Display for PythonToolError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::PythonException(msg) => write!(f, "Python exception: {}", msg),
            Self::TypeConversion(msg) => write!(f, "Type conversion error: {}", msg),
            Self::ValidationError(msg) => write!(f, "Validation error: {}", msg),
            Self::GilError(msg) => write!(f, "GIL error: {}", msg),
            Self::InvalidCallable(msg) => write!(f, "Invalid callable: {}", msg),
            Self::PyO3Error(msg) => write!(f, "PyO3 error: {}", msg),
        }
    }
}

impl std::error::Error for PythonToolError {}

impl From<pyo3::PyErr> for PythonToolError {
    fn from(err: pyo3::PyErr) -> Self {
        PythonToolError::PyO3Error(err.to_string())
    }
}

/// Check if a Python object is a coroutine (async function result)
fn is_coroutine(py: Python<'_>, obj: &Bound<'_, PyAny>) -> bool {
    let inspect = py.import("inspect").ok();
    if let Some(inspect) = inspect {
        if let Ok(is_coro) = inspect.call_method1("iscoroutine", (obj,)) {
            if let Ok(true) = is_coro.extract::<bool>() {
                return true;
            }
        }
    }
    false
}

/// Python callable wrapper that implements Tool trait
///
/// 将 Python callable 封装为 Rust Tool trait 实现，支持同步和异步函数。
///
/// Example:
///     >>> from sh_python import register_tool
///     >>> @register_tool("echo", "Echo input text")
///     ... def echo(text: str) -> str:
///     ...     return text
///     >>>
///     >>> @register_tool("async_echo", "Async echo")
///     ... async def async_echo(text: str) -> str:
///     ...     return text
pub struct PythonToolAdapter {
    /// Tool name
    name: String,
    /// Tool description
    description: String,
    /// Parameter schema (JSON Schema)
    parameters: JsonValue,
    /// Required parameters
    required: Vec<String>,
    /// Python callable wrapped in Arc for thread safety
    callable: Arc<Py<PyAny>>,
    /// Whether the callable is async
    is_async: bool,
}

impl PythonToolAdapter {
    /// Create a new Python tool adapter
    pub fn new(
        name: String,
        description: String,
        parameters: JsonValue,
        required: Vec<String>,
        callable: Py<PyAny>,
        is_async: bool,
    ) -> Self {
        Self {
            name,
            description,
            parameters,
            required,
            callable: Arc::new(callable),
            is_async,
        }
    }

    /// Create from a Python callable
    ///
    /// This inspects the callable to determine if it's async and extracts
    /// parameter schema information.
    pub fn from_callable(
        py: Python<'_>,
        name: String,
        description: String,
        callable: Py<PyAny>,
        parameters: Option<Bound<'_, PyDict>>,
    ) -> PyResult<Self> {
        // Validate that callable is actually callable
        let bound = callable.bind(py);
        if !bound.is_callable() {
            return Err(PyTypeError::new_err(format!(
                "Object '{}' is not callable",
                bound.repr()?.extract::<String>()?
            )));
        }

        // Determine if async by checking if it's a coroutine function
        let is_async = Python::attach(|py| {
            let inspect = py.import("inspect").ok();
            if let Some(inspect) = inspect {
                if let Ok(is_coro_fn) =
                    inspect.call_method1("iscoroutinefunction", (callable.bind(py),))
                {
                    return is_coro_fn.extract::<bool>().unwrap_or(false);
                }
            }
            false
        });

        // Extract parameters schema
        let (parameters, required) = if let Some(schema_dict) = parameters {
            let params = python_dict_to_json(&schema_dict)?;
            let required = params
                .get("required")
                .and_then(|r| r.as_array())
                .map(|arr| {
                    arr.iter()
                        .filter_map(|v| v.as_str().map(|s| s.to_string()))
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default();
            (params, required)
        } else {
            (json!({"type": "object", "properties": {}}), vec![])
        };

        Ok(Self::new(
            name,
            description,
            parameters,
            required,
            callable,
            is_async,
        ))
    }

    /// Execute a synchronous Python callable
    fn execute_sync(&self, py: Python<'_>, args: JsonValue) -> Result<String, PythonToolError> {
        let py_args = json_to_python_dict(py, &args)?;

        let result = self
            .callable
            .call1(py, (py_args,))
            .map_err(|e| PythonToolError::PythonException(e.to_string()))?;

        // Convert result to string
        let bound = result.bind(py);
        if let Ok(dict) = bound.cast::<PyDict>() {
            python_dict_to_json_string(dict)
        } else if let Ok(s) = bound.extract::<String>() {
            Ok(s)
        } else if let Ok(py_str) = bound.cast::<PyString>() {
            py_str
                .extract::<String>()
                .map_err(|e| PythonToolError::TypeConversion(e.to_string()))
        } else {
            // Fallback: use repr for complex objects
            bound
                .repr()
                .and_then(|r| r.extract::<String>())
                .map_err(|e| PythonToolError::TypeConversion(e.to_string()))
        }
    }

    /// Execute an async Python callable
    async fn execute_async(&self, args: JsonValue) -> Result<String, PythonToolError> {
        let callable = Arc::clone(&self.callable);

        // Get the Python event loop and convert coroutine to future
        let coroutine_result = Python::attach(|py| {
            let py_args = json_to_python_dict(py, &args)?;

            // Call the async function to get a coroutine
            let coroutine = callable
                .call1(py, (py_args,))
                .map_err(|e| PythonToolError::PythonException(e.to_string()))?;

            let bound = coroutine.bind(py);

            // Check if it's actually a coroutine
            if !is_coroutine(py, bound) {
                return Err(PythonToolError::InvalidCallable(
                    "Async callable did not return a coroutine".to_string(),
                ));
            }

            // Convert coroutine to Rust future
            let fut = pyo3_async_runtimes::tokio::into_future(bound.clone())
                .map_err(|e| PythonToolError::PythonException(e.to_string()))?;

            Ok::<_, PythonToolError>(fut)
        })?;

        // Execute the future
        let result = coroutine_result.await;

        // Convert result to string
        Python::attach(|py| match result {
            Ok(py_result) => {
                let bound = py_result.bind(py);
                if let Ok(dict) = bound.cast::<PyDict>() {
                    python_dict_to_json_string(dict)
                } else if let Ok(s) = bound.extract::<String>() {
                    Ok(s)
                } else {
                    bound
                        .repr()
                        .and_then(|r| r.extract::<String>())
                        .map_err(|e| PythonToolError::TypeConversion(e.to_string()))
                }
            }
            Err(e) => Err(PythonToolError::PythonException(e.to_string())),
        })
    }
}

#[async_trait]
impl Tool for PythonToolAdapter {
    fn name(&self) -> &str {
        &self.name
    }

    fn description(&self) -> &str {
        &self.description
    }

    fn parameters(&self) -> JsonValue {
        self.parameters.clone()
    }

    async fn execute(&self, args: &str) -> Layer2Result<ToolResult> {
        let args_json: JsonValue = serde_json::from_str(args).unwrap_or(JsonValue::Null);

        // Validate required parameters first
        if let Err(e) = self.validate_args_internal(&args_json) {
            return Err(anyhow::anyhow!("Validation error: {}", e));
        }

        let name = self.name.clone();
        let tool_call_id = sh_layer1::generate_short_id();

        // Execute based on whether it's async
        let result = if self.is_async {
            self.execute_async(args_json).await
        } else {
            // For sync execution, we need to acquire GIL
            Python::attach(|py| self.execute_sync(py, args_json))
        };

        match result {
            Ok(content) => Ok(ToolResult {
                name,
                tool_call_id,
                content,
                is_error: false,
            }),
            Err(e) => Ok(ToolResult {
                name,
                tool_call_id,
                content: e.to_string(),
                is_error: true,
            }),
        }
    }

    fn validate_args(&self, args: &JsonValue) -> Layer2Result<bool> {
        Ok(self.validate_args_internal(args)?)
    }
}

impl PythonToolAdapter {
    /// Internal validation logic
    fn validate_args_internal(&self, args: &JsonValue) -> Result<bool, PythonToolError> {
        // Check if args is an object
        let props = args.as_object().ok_or_else(|| {
            PythonToolError::ValidationError("Arguments must be a JSON object".to_string())
        })?;

        // Check required parameters
        for req in &self.required {
            if !props.contains_key(req) {
                return Err(PythonToolError::ValidationError(format!(
                    "Missing required parameter: {}",
                    req
                )));
            }
        }

        // Type checking for parameters with defined types
        if let Some(properties) = self.parameters.get("properties") {
            if let Some(properties_obj) = properties.as_object() {
                for (key, value) in props {
                    if let Some(param_schema) = properties_obj.get(key) {
                        if let Err(e) = validate_type(key, value, param_schema) {
                            return Err(PythonToolError::ValidationError(e));
                        }
                    }
                }
            }
        }

        Ok(true)
    }
}

/// Validate JSON value against a parameter schema
fn validate_type(key: &str, value: &JsonValue, schema: &JsonValue) -> Result<(), String> {
    if let Some(type_str) = schema.get("type").and_then(|t| t.as_str()) {
        let valid = match type_str {
            "string" => value.is_string(),
            "number" => value.is_number(),
            "integer" => value.is_i64() || value.is_u64(),
            "boolean" => value.is_boolean(),
            "array" => value.is_array(),
            "object" => value.is_object(),
            "null" => value.is_null(),
            _ => true,
        };

        if !valid {
            return Err(format!(
                "Parameter '{}' has wrong type. Expected '{}'",
                key, type_str
            ));
        }
    }

    // Check enum values if specified
    if let Some(enum_vals) = schema.get("enum").and_then(|e| e.as_array()) {
        if !enum_vals.contains(value) {
            return Err(format!(
                "Parameter '{}' value is not one of the allowed enum values",
                key
            ));
        }
    }

    Ok(())
}

/// Convert JSON value to Python dict
fn json_to_python_dict<'py>(py: Python<'py>, json: &JsonValue) -> PyResult<Bound<'py, PyDict>> {
    let dict = PyDict::new(py);

    if let Some(obj) = json.as_object() {
        for (key, value) in obj {
            let py_value = json_value_to_python(py, value)?;
            dict.set_item(key, py_value)?;
        }
    }

    Ok(dict)
}

/// Convert a single JSON value to Python object
/// Convert JSON value to Python object
/// Note: Uses to_object which is deprecated in PyO3 0.23, but IntoPyObject migration
/// requires more complex changes. This will be updated in a future refactoring.
#[allow(deprecated)]
fn json_value_to_python<'py>(py: Python<'py>, value: &JsonValue) -> PyResult<Bound<'py, PyAny>> {
    match value {
        JsonValue::Null => Ok(py.None().into_bound(py)),
        JsonValue::Bool(b) => Ok(b.into_py_any(py)?.into_bound(py)),
        JsonValue::Number(n) => {
            if let Some(i) = n.as_i64() {
                Ok(i.into_py_any(py)?.into_bound(py))
            } else if let Some(f) = n.as_f64() {
                Ok(f.into_py_any(py)?.into_bound(py))
            } else {
                Ok(n.to_string().into_py_any(py)?.into_bound(py))
            }
        }
        JsonValue::String(s) => Ok(s.into_py_any(py)?.into_bound(py)),
        JsonValue::Array(arr) => {
            let list = PyList::empty(py);
            for item in arr {
                list.append(json_value_to_python(py, item)?)?;
            }
            Ok(list.into_any())
        }
        JsonValue::Object(obj) => {
            let dict = PyDict::new(py);
            for (k, v) in obj {
                dict.set_item(k, json_value_to_python(py, v)?)?;
            }
            Ok(dict.into_any())
        }
    }
}

/// Convert Python dict to JSON value
fn python_dict_to_json(dict: &Bound<'_, PyDict>) -> PyResult<JsonValue> {
    let mut map = serde_json::Map::new();

    for (key, value) in dict.iter() {
        let key_str: String = key.extract()?;
        let json_value = python_value_to_json(&value)?;
        map.insert(key_str, json_value);
    }

    Ok(JsonValue::Object(map))
}

/// Convert Python value to JSON
fn python_value_to_json(value: &Bound<'_, PyAny>) -> PyResult<JsonValue> {
    if value.is_none() {
        return Ok(JsonValue::Null);
    }

    if let Ok(b) = value.extract::<bool>() {
        return Ok(JsonValue::Bool(b));
    }

    if let Ok(i) = value.extract::<i64>() {
        return Ok(JsonValue::Number(i.into()));
    }

    if let Ok(f) = value.extract::<f64>() {
        if let Some(n) = serde_json::Number::from_f64(f) {
            return Ok(JsonValue::Number(n));
        }
    }

    if let Ok(s) = value.extract::<String>() {
        return Ok(JsonValue::String(s));
    }

    if let Ok(list) = value.cast::<PyList>() {
        let mut arr = Vec::new();
        for item in list.iter() {
            arr.push(python_value_to_json(&item)?);
        }
        return Ok(JsonValue::Array(arr));
    }

    if let Ok(dict) = value.cast::<PyDict>() {
        return python_dict_to_json(dict);
    }

    // Fallback: convert to string via repr
    Ok(JsonValue::String(value.repr()?.extract::<String>()?))
}

/// Convert Python dict to JSON string
fn python_dict_to_json_string(dict: &Bound<'_, PyDict>) -> Result<String, PythonToolError> {
    let json =
        python_dict_to_json(dict).map_err(|e| PythonToolError::TypeConversion(e.to_string()))?;
    serde_json::to_string(&json).map_err(|e| PythonToolError::TypeConversion(e.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Once;

    static INIT: Once = Once::new();

    fn ensure_python_init() {
        INIT.call_once(|| {
            pyo3::prepare_freethreaded_python();
        });
    }

    fn with_gil<F, R>(f: F) -> R
    where
        F: FnOnce(Python<'_>) -> R,
    {
        ensure_python_init();
        Python::attach(f)
    }

    #[test]
    fn test_adapter_creation_sync() {
        with_gil(|py| {
            let callable = py.eval(c"lambda x: x", None, None).unwrap();
            let adapter = PythonToolAdapter::from_callable(
                py,
                "test_sync".to_string(),
                "Test sync tool".to_string(),
                callable.into(),
                None,
            )
            .unwrap();

            assert_eq!(adapter.name(), "test_sync");
            assert_eq!(adapter.description(), "Test sync tool");
            assert!(!adapter.is_async);
        });
    }

    #[test]
    fn test_adapter_creation_async() {
        with_gil(|py| {
            let _code = r#"
async def async_tool(x):
    return x * 2
"#;
            py.run(
                c"
async def async_tool(x):
    return x * 2
",
                None,
                None,
            )
            .unwrap();
            let callable = py.eval(c"async_tool", None, None).unwrap();
            let adapter = PythonToolAdapter::from_callable(
                py,
                "test_async".to_string(),
                "Test async tool".to_string(),
                callable.into(),
                None,
            )
            .unwrap();

            assert_eq!(adapter.name(), "test_async");
            // Async detection works
            assert!(adapter.is_async);
        });
    }

    #[test]
    fn test_sync_execution() {
        with_gil(|py| {
            py.run(
                c"
def sync_tool(args):
    return f\"Got: {args['value']}\"
",
                None,
                None,
            )
            .unwrap();
            let callable = py.eval(c"sync_tool", None, None).unwrap();
            let adapter = PythonToolAdapter::from_callable(
                py,
                "sync_exec_test".to_string(),
                "Test sync execution".to_string(),
                callable.into(),
                None,
            )
            .unwrap();

            let result = adapter.execute_sync(py, json!({"value": 42}));

            assert!(result.is_ok());
            assert_eq!(result.unwrap(), "Got: 42");
        });
    }

    #[test]
    fn test_validation_success() {
        with_gil(|py| {
            let callable = py.eval(c"lambda x: x", None, None).unwrap();
            let params = json!({
                "type": "object",
                "properties": {
                    "name": {"type": "string"},
                    "count": {"type": "integer"}
                },
                "required": ["name"]
            });

            let adapter = PythonToolAdapter::new(
                "validate_test".to_string(),
                "Test validation".to_string(),
                params,
                vec!["name".to_string()],
                callable.into(),
                false,
            );

            // Valid args
            let result = adapter.validate_args(&json!({"name": "test", "count": 5}));
            assert!(result.is_ok());

            // Missing required
            let result = adapter.validate_args(&json!({"count": 5}));
            assert!(result.is_err());
        });
    }

    #[test]
    fn test_type_validation() {
        with_gil(|py| {
            let callable = py.eval(c"lambda x: x", None, None).unwrap();
            let params = json!({
                "type": "object",
                "properties": {
                    "num": {"type": "integer"},
                    "text": {"type": "string"},
                    "flag": {"type": "boolean"}
                }
            });

            let adapter = PythonToolAdapter::new(
                "type_check".to_string(),
                "Type check tool".to_string(),
                params,
                vec![],
                callable.into(),
                false,
            );

            // Wrong type for integer
            let result = adapter.validate_args(&json!({"num": "not_a_number"}));
            assert!(result.is_err());

            // Correct types
            let result = adapter.validate_args(&json!({"num": 42, "text": "hello", "flag": true}));
            assert!(result.is_ok());
        });
    }

    #[test]
    fn test_json_python_conversion() {
        with_gil(|py| {
            let json = json!({
                "string": "hello",
                "number": 42,
                "float": 1.2345,
                "bool": true,
                "null": null,
                "array": [1, 2, 3],
                "nested": {"key": "value"}
            });

            let py_dict = json_to_python_dict(py, &json).unwrap();
            let converted = python_dict_to_json(&py_dict).unwrap();

            assert_eq!(json, converted);
        });
    }

    #[test]
    fn test_python_json_roundtrip() {
        with_gil(|py| {
            py.run(
                c"
test_dict = {
    'name': 'test',
    'count': 10,
    'active': True,
    'items': [1, 2, 3],
    'config': {'debug': False}
}
",
                None,
                None,
            )
            .unwrap();
            let py_dict = py.eval(c"test_dict", None, None).unwrap();
            let dict = py_dict.cast::<PyDict>().unwrap();

            let json = python_dict_to_json(dict).unwrap();
            let py_dict2 = json_to_python_dict(py, &json).unwrap();
            let json2 = python_dict_to_json(&py_dict2).unwrap();

            assert_eq!(json, json2);
        });
    }

    #[test]
    fn test_error_propagation() {
        with_gil(|py| {
            py.run(
                c"
def error_tool(args):
    raise ValueError('Test error message')
",
                None,
                None,
            )
            .unwrap();
            let callable = py.eval(c"error_tool", None, None).unwrap();
            let adapter = PythonToolAdapter::from_callable(
                py,
                "error_test".to_string(),
                "Error tool".to_string(),
                callable.into(),
                None,
            )
            .unwrap();

            let result = adapter.execute_sync(py, json!({}));

            assert!(result.is_err());
            let err_msg = result.unwrap_err().to_string();
            assert!(err_msg.contains("Test error message"));
        });
    }

    #[test]
    fn test_non_callable_error() {
        with_gil(|py| {
            let not_callable = py.eval(c"42", None, None).unwrap();
            let result = PythonToolAdapter::from_callable(
                py,
                "bad_tool".to_string(),
                "Not callable".to_string(),
                not_callable.into(),
                None,
            );

            assert!(result.is_err());
        });
    }

    #[test]
    fn test_complex_return_types() {
        with_gil(|py| {
            py.run(
                c"
def complex_tool(args):
    return {'result': 'success', 'data': [1, 2, 3], 'meta': {'count': 3}}
",
                None,
                None,
            )
            .unwrap();
            let callable = py.eval(c"complex_tool", None, None).unwrap();
            let adapter = PythonToolAdapter::from_callable(
                py,
                "complex".to_string(),
                "Complex return".to_string(),
                callable.into(),
                None,
            )
            .unwrap();

            let result = adapter.execute_sync(py, json!({})).unwrap();
            let parsed: JsonValue = serde_json::from_str(&result).unwrap();

            assert_eq!(parsed["result"], "success");
            assert_eq!(parsed["data"].as_array().unwrap().len(), 3);
        });
    }

    #[test]
    fn test_enum_validation() {
        with_gil(|py| {
            let callable = py.eval(c"lambda x: x", None, None).unwrap();
            let params = json!({
                "type": "object",
                "properties": {
                    "status": {
                        "type": "string",
                        "enum": ["active", "inactive", "pending"]
                    }
                }
            });

            let adapter = PythonToolAdapter::new(
                "enum_check".to_string(),
                "Enum check tool".to_string(),
                params,
                vec![],
                callable.into(),
                false,
            );

            // Valid enum value
            let result = adapter.validate_args(&json!({"status": "active"}));
            assert!(result.is_ok());

            // Invalid enum value
            let result = adapter.validate_args(&json!({"status": "unknown"}));
            assert!(result.is_err());
        });
    }
}
