"""
Tests for utility functions module.

Tests all utility functions including ID generation.
"""

import re

from continuum_sdk.utils import generate_prefixed_id, generate_short_id


class TestGenerateShortId:
    """Tests for generate_short_id function."""

    def test_returns_string(self):
        """Test that generate_short_id returns a string."""
        result = generate_short_id()
        assert isinstance(result, str)

    def test_length_is_8(self):
        """Test that the returned ID is exactly 8 characters."""
        result = generate_short_id()
        assert len(result) == 8

    def test_contains_only_hex_characters(self):
        """Test that the ID contains only lowercase hex characters."""
        result = generate_short_id()
        # Should only contain characters 0-9 and a-f
        assert all(c in '0123456789abcdef' for c in result)

    def test_is_lowercase(self):
        """Test that the ID is lowercase."""
        result = generate_short_id()
        assert result == result.lower()

    def test_uniqueness(self):
        """Test that multiple calls generate different IDs."""
        ids = [generate_short_id() for _ in range(100)]
        # All IDs should be unique (extremely unlikely to have collisions with UUID4)
        assert len(set(ids)) == 100

    def test_format_matches_rust_implementation(self):
        """Test that the format matches the Rust implementation.

        The Rust implementation returns 8 lowercase hex characters.
        """
        result = generate_short_id()
        # Should match pattern: 8 lowercase hex characters
        pattern = re.compile(r'^[0-9a-f]{8}$')
        assert pattern.match(result) is not None

    def test_derived_from_uuid4(self):
        """Test that the ID is derived from UUID4 format.

        UUID4 format: xxxxxxxx-xxxx-4xxx-yxxx-xxxxxxxxxxxx
        Our implementation takes the first 8 characters.
        """
        result = generate_short_id()
        # The result should be the first 8 characters of a UUID4
        # We can't verify it's actually from UUID4, but we can verify
        # the format is consistent with taking the first 8 hex chars
        assert len(result) == 8
        assert all(c in '0123456789abcdef' for c in result)


class TestGeneratePrefixedId:
    """Tests for generate_prefixed_id function."""

    def test_returns_string(self):
        """Test that generate_prefixed_id returns a string."""
        result = generate_prefixed_id("task")
        assert isinstance(result, str)

    def test_prefix_is_prepended(self):
        """Test that the prefix is correctly prepended."""
        result = generate_prefixed_id("task")
        assert result.startswith("task_")

    def test_format_is_correct(self):
        """Test that the format is {prefix}_{short_id}."""
        prefix = "task"
        result = generate_prefixed_id(prefix)
        # Split by underscore
        parts = result.split("_")
        assert len(parts) == 2
        assert parts[0] == prefix
        assert len(parts[1]) == 8

    def test_total_length(self):
        """Test the total length of the prefixed ID."""
        # "task_" is 5 characters + 8 for short_id = 13
        result = generate_prefixed_id("task")
        assert len(result) == 13

        # "call_" is 5 characters + 8 for short_id = 13
        result = generate_prefixed_id("call")
        assert len(result) == 13

        # "tc_" is 3 characters + 8 for short_id = 11
        result = generate_prefixed_id("tc")
        assert len(result) == 11

    def test_short_id_is_valid_hex(self):
        """Test that the ID part after prefix is valid hex."""
        result = generate_prefixed_id("task")
        short_id = result.split("_")[1]
        assert all(c in '0123456789abcdef' for c in short_id)

    def test_uniqueness(self):
        """Test that multiple calls generate different IDs."""
        prefix = "task"
        ids = [generate_prefixed_id(prefix) for _ in range(100)]
        # All IDs should be unique
        assert len(set(ids)) == 100

    def test_different_prefixes(self):
        """Test with different prefix values."""
        prefixes = ["task", "call", "tc", "job", "process", "request"]

        for prefix in prefixes:
            result = generate_prefixed_id(prefix)
            assert result.startswith(f"{prefix}_")
            parts = result.split("_")
            assert len(parts) == 2
            assert parts[0] == prefix

    def test_empty_prefix(self):
        """Test behavior with empty prefix."""
        result = generate_prefixed_id("")
        # Should produce "_{short_id}"
        assert result.startswith("_")
        assert len(result) == 9  # 1 for underscore + 8 for short_id

    def test_underscore_in_prefix(self):
        """Test behavior when prefix contains underscore."""
        result = generate_prefixed_id("my_task")
        # Should produce "my_task_{short_id}"
        # When splitting, we'll get more than 2 parts
        assert result.startswith("my_task_")
        # The last 8 characters should be the short_id
        short_id = result[-8:]
        assert all(c in '0123456789abcdef' for c in short_id)

    def test_format_matches_rust_implementation(self):
        """Test that the format matches the Rust implementation.

        The Rust implementation returns "{prefix}_{short_id}"
        where short_id is 8 lowercase hex characters.
        """
        result = generate_prefixed_id("task")
        pattern = re.compile(r'^task_[0-9a-f]{8}$')
        assert pattern.match(result) is not None

    def test_integration_with_generate_short_id(self):
        """Test that generate_prefixed_id uses generate_short_id."""
        # The short_id part should match the format from generate_short_id
        result = generate_prefixed_id("test")
        short_id = result.split("_")[1]

        # Should have same properties as generate_short_id()
        assert len(short_id) == 8
        assert all(c in '0123456789abcdef' for c in short_id)


class TestModuleExports:
    """Tests for module exports."""

    def test_all_exports_present(self):
        """Test that __all__ contains expected exports."""
        from continuum_sdk.utils import __all__

        assert "generate_short_id" in __all__
        assert "generate_prefixed_id" in __all__
        assert len(__all__) == 2

    def test_import_from_module(self):
        """Test that functions can be imported from the module."""
        from continuum_sdk.utils import generate_prefixed_id, generate_short_id

        # Should be callable
        assert callable(generate_short_id)
        assert callable(generate_prefixed_id)

    def test_import_star(self):
        """Test that __all__ enables proper star imports."""
        # This should not raise an error
        exec("from continuum_sdk.utils import *")


class TestIdGenerationPatterns:
    """Tests for common ID generation patterns."""

    def test_multiple_ids_for_different_purposes(self):
        """Test generating IDs for different purposes."""
        task_id = generate_prefixed_id("task")
        call_id = generate_prefixed_id("call")
        tc_id = generate_prefixed_id("tc")

        # All should have correct prefixes
        assert task_id.startswith("task_")
        assert call_id.startswith("call_")
        assert tc_id.startswith("tc_")

        # All should be unique
        assert task_id != call_id
        assert task_id != tc_id
        assert call_id != tc_id

    def test_id_generation_in_loop(self):
        """Test generating many IDs in a loop."""
        task_ids = []
        for _i in range(50):
            task_ids.append(generate_prefixed_id("task"))

        # All should be unique
        assert len(set(task_ids)) == 50

        # All should have correct format
        for task_id in task_ids:
            assert task_id.startswith("task_")
            assert len(task_id) == 13

    def test_id_consistency(self):
        """Test that IDs are consistently formatted."""
        for _ in range(20):
            short_id = generate_short_id()
            prefixed_id = generate_prefixed_id("test")

            # Short ID should always be 8 hex chars
            assert len(short_id) == 8
            assert all(c in '0123456789abcdef' for c in short_id)

            # Prefixed ID should be prefix + underscore + 8 hex chars
            assert prefixed_id.startswith("test_")
            assert len(prefixed_id) == 13