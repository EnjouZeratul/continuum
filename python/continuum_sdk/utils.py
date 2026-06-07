"""Utility functions for Continuum SDK.

Provides common utilities used across the SDK.

[STABILITY: STABLE] Core utilities
"""

import uuid


def generate_short_id() -> str:
    """Generate a short unique ID.

    Returns an 8-character hex string derived from UUID4.
    This matches the Rust implementation in sh_layer1::utils::generate_short_id.

    Format: 8 lowercase hex characters (e.g., "a1b2c3d4")

    Returns:
        8-character unique ID string

    Example:
        >>> id = generate_short_id()
        >>> len(id)
        8
        >>> all(c in '0123456789abcdef' for c in id)
        True
    """
    return str(uuid.uuid4())[:8]


def generate_prefixed_id(prefix: str) -> str:
    """Generate a prefixed short ID.

    Creates a unique ID with a descriptive prefix, useful for
    identifying the type or source of an ID.

    This matches the Rust implementation in sh_layer1::utils::generate_prefixed_id.

    Args:
        prefix: The prefix to use (e.g., "task", "call", "tc")

    Returns:
        Prefixed ID in format "{prefix}_{short_id}"

    Example:
        >>> task_id = generate_prefixed_id("task")
        >>> task_id.startswith("task_")
        True
        >>> len(task_id)
        13  # "task_" (5) + short_id (8)

        >>> call_id = generate_prefixed_id("call")
        >>> call_id.startswith("call_")
        True
    """
    return f"{prefix}_{generate_short_id()}"


__all__ = [
    "generate_short_id",
    "generate_prefixed_id",
]
