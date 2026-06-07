# Continuum API Migration Guide

This guide helps you migrate from the legacy API to the unified API.

## Overview

Continuum now provides a **Unified API** that automatically selects the best implementation:
- **Rust binding** (`sh_python`) - High performance
- **Pure Python** - Compatibility fallback

## Quick Migration

### Before (Legacy)

```python
from continuum_sdk.agent import Agent
from continuum_sdk.tools.builtin import BuiltinTools

agent = Agent()
tools = BuiltinTools()
```

### After (Unified)

```python
from continuum_sdk import Agent, BuiltinTools

# Automatically uses best implementation
agent = Agent()
tools = BuiltinTools()
```

## Key Changes

### 1. Import Path

| Legacy | Unified |
|--------|---------|
| `from continuum_sdk.agent import Agent` | `from continuum_sdk import Agent` |
| `from continuum_sdk.tools.builtin import BuiltinTools` | `from continuum_sdk import BuiltinTools` |

### 2. Implementation Selection

```python
# Check current implementation
from continuum_sdk import HAS_RUST_BINDING, get_implementation_preference

print(f"Rust available: {HAS_RUST_BINDING}")
print(f"Using: {get_implementation_preference()}")

# Force specific implementation
agent = Agent(impl="python")  # Force Python
agent = Agent(impl="rust")    # Force Rust (if available)
```

### 3. Environment Variable Override

```bash
# Force Python implementation
export CONTINUUM_IMPL=python

# Force Rust implementation (requires binding)
export CONTINUUM_IMPL=rust
```

## API Compatibility

Both implementations provide the same API:

### Agent

| Method | Description |
|--------|-------------|
| `run(task)` | Execute task synchronously |
| `arun(task)` | Execute task asynchronously |
| `register_tool(name, func, ...)` | Register custom tool |
| `create_session()` | Create new session |

### Session

| Method | Description |
|--------|-------------|
| `add_message(role, content)` | Add message |
| `get_messages()` | Get all messages |
| `save()` | Save session |

### BuiltinTools

| Method | Description |
|--------|-------------|
| `read_file(path, ...)` | Read file |
| `write_file(path, content)` | Write file |
| `edit_file(path, old, new)` | Edit file |
| `grep(pattern, ...)` | Search content |
| `glob(pattern, ...)` | Find files |
| `bash(command, ...)` | Execute command |

## Implementation Details

### Rust Implementation (`rust_impl.py`)

- Wraps `sh_python` bindings
- Higher performance for I/O operations
- Lower memory footprint
- Recommended for production

### Python Implementation (`python_impl.py`)

- Pure Python fallback
- No native dependencies
- Works in restricted environments
- Easier debugging

## Deprecation Timeline

| Version | Status |
|---------|--------|
| v1.0 | Unified API introduced |
| v1.1 | Legacy imports deprecated |
| v2.0 | Legacy imports removed |

## FAQ

**Q: Will my existing code break?**
A: No. Legacy imports still work. They're just deprecated.

**Q: How do I know which implementation is used?**
A: Check `agent.implementation` or `HAS_RUST_BINDING`.

**Q: Can I mix implementations?**
A: Yes. You can use `impl="python"` or `impl="rust"` per instance.

**Q: What if Rust binding fails?**
A: Automatically falls back to Python implementation.

## Need Help?

- GitHub Issues: https://github.com/continuum/continuum/issues
- Documentation: https://continuum.readthedocs.io