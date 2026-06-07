# Contributing to Continuum SDK

Thank you for your interest in contributing to Continuum SDK! This document provides guidelines and instructions for contributing.

## Table of Contents

- [Code of Conduct](#code-of-conduct)
- [Getting Started](#getting-started)
- [Development Setup](#development-setup)
- [How to Contribute](#how-to-contribute)
- [Coding Standards](#coding-standards)
- [Commit Guidelines](#commit-guidelines)
- [Pull Request Process](#pull-request-process)

---

## Code of Conduct

This project follows the [Contributor Covenant Code of Conduct](https://www.contributor-covenant.org/version/2/1/code_of-conduct/). By participating, you are expected to uphold this code. Please report unacceptable behavior to the maintainers.

---

## Getting Started

### Prerequisites

- Python 3.10 or higher
- pip or uv package manager
- Git

### Development Setup

1. **Fork and Clone**

```bash
git clone https://github.com/your-username/continuum-sdk.git
cd continuum-sdk/python
```

2. **Create Virtual Environment**

```bash
python -m venv .venv
source .venv/bin/activate  # Linux/macOS
.venv\Scripts\activate     # Windows
```

3. **Install Development Dependencies**

```bash
pip install -e ".[dev]"
```

4. **Install Pre-commit Hooks**

```bash
pre-commit install
```

---

## How to Contribute

### Reporting Bugs

Before submitting a bug report, please:

1. Check existing issues to avoid duplicates
2. Use the bug report template
3. Include:
   - Python version
   - OS and version
   - Steps to reproduce
   - Expected vs actual behavior
   - Logs or error messages

### Suggesting Features

Feature suggestions are welcome! Please:

1. Check existing issues/discussions
2. Use the feature request template
3. Describe:
   - The problem it solves
   - Proposed solution
   - Alternative solutions considered
   - Potential impact

### Submitting Code

1. Create a feature branch from `main`
2. Make your changes
3. Write/update tests
4. Update documentation
5. Submit a pull request

---

## Coding Standards

### Python Style

We follow PEP 8 with some modifications:

```python
# Line length: 100 characters
# Use double quotes for strings
# Use type hints for all public APIs

def process_data(items: list[str]) -> dict[str, int]:
    """Process a list of items and return counts.
    
    Args:
        items: List of item names to process.
        
    Returns:
        Dictionary mapping item names to counts.
    """
    result: dict[str, int] = {}
    for item in items:
        result[item] = result.get(item, 0) + 1
    return result
```

### Code Organization

```
continuum_sdk/
├── __init__.py          # Public API exports
├── api.py               # Unified API
├── agent/               # Agent intelligence
│   ├── __init__.py
│   ├── checkpoint.py
│   ├── history.py
│   └── ...
├── config/              # Configuration
├── llm/                 # LLM clients
├── memory/              # Memory system
├── security/            # Security module
└── render/              # Rendering
```

### Documentation

- Use docstrings for all public functions and classes
- Follow Google docstring style
- Include examples in docstrings

```python
def calculate_sum(numbers: list[int]) -> int:
    """Calculate the sum of a list of numbers.
    
    Args:
        numbers: List of integers to sum.
        
    Returns:
        The sum of all numbers.
        
    Raises:
        ValueError: If the list is empty.
        
    Example:
        >>> calculate_sum([1, 2, 3])
        6
    """
    if not numbers:
        raise ValueError("List cannot be empty")
    return sum(numbers)
```

### Type Hints

```python
from typing import Optional, Union, TypeVar, Generic

T = TypeVar("T")

class Container(Generic[T]):
    def __init__(self, value: Optional[T] = None) -> None:
        self._value = value
    
    def get(self) -> T:
        if self._value is None:
            raise ValueError("No value set")
        return self._value
```

---

## Commit Guidelines

### Commit Message Format

```
<type>(<scope>): <subject>

<body>

<footer>
```

### Types

| Type | Description |
|------|-------------|
| `feat` | New feature |
| `fix` | Bug fix |
| `docs` | Documentation only |
| `style` | Code style (formatting, etc.) |
| `refactor` | Code refactoring |
| `test` | Adding/updating tests |
| `chore` | Maintenance tasks |

### Examples

```
feat(agent): add self-correction capability

Implement automatic error detection and retry mechanism
for agent operations. Includes configurable retry count
and custom error handlers.

Closes #123
```

```
fix(memory): resolve SQLite connection leak

Connection was not properly closed when using FTS5 search.
Added context manager for automatic cleanup.

Fixes #456
```

---

## Pull Request Process

### Before Submitting

1. **Run Tests**

```bash
pytest tests/ -v --cov=continuum_sdk
```

2. **Run Linters**

```bash
ruff check continuum_sdk/
mypy continuum_sdk/
```

3. **Format Code**

```bash
ruff format continuum_sdk/
```

4. **Update Documentation**

- API changes: Update `docs/API_REFERENCE.md`
- New features: Update `docs/CHANGELOG.md`
- Configuration: Update `README.md`

### PR Checklist

- [ ] Code follows project style guidelines
- [ ] Tests pass locally
- [ ] New tests added for new functionality
- [ ] Documentation updated
- [ ] Changelog entry added
- [ ] Commit messages follow guidelines

### Review Process

1. At least one approval required
2. All CI checks must pass
3. No merge conflicts
4. Squash and merge to main

---

## Testing

### Running Tests

```bash
# All tests
pytest tests/

# Specific test file
pytest tests/test_agent.py

# With coverage
pytest tests/ --cov=continuum_sdk --cov-report=html
```

### Writing Tests

```python
import pytest
from continuum_sdk import Agent, Config

class TestAgent:
    @pytest.fixture
    def agent(self):
        config = Config(provider="anthropic", api_key="test-key")
        return Agent(config=config)
    
    def test_agent_creation(self, agent):
        assert agent is not None
        assert agent.config.provider == "anthropic"
    
    @pytest.mark.asyncio
    async def test_agent_run(self, agent, mocker):
        mocker.patch.object(agent._client, "chat", return_value="Hello!")
        result = await agent.arun("Hi")
        assert result == "Hello!"
```

---

## License

By contributing to Continuum SDK, you agree that your contributions will be licensed under the MIT License.

---

## Questions?

- Open an issue for bug reports or feature requests
- Start a discussion for questions or ideas
- Join our community chat (coming soon)

Thank you for contributing!
