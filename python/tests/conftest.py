"""
Pytest configuration for VCR recording/playback.

This enables running API-dependent tests without real API keys
by using recorded HTTP interactions (cassettes).
"""

import os

import pytest

# Cassette storage directory
CASSETTE_DIR = os.path.join(os.path.dirname(__file__), "cassettes")


def pytest_configure(config):
    """Configure pytest with VCR markers."""
    config.addinivalue_line(
        "markers", "vcr: mark test to use VCR recording/playback"
    )


@pytest.fixture(scope="module")
def vcr_config():
    """Global VCR configuration."""
    return {
        "record_mode": "once",  # Record once, then replay
        "match_on": ["method", "scheme", "host", "port", "path", "query", "body"],
        "filter_headers": [
            "authorization",
            "x-api-key",
            "anthropic-api-key",
            "openai-api-key",
            "api-key",
        ],
        "filter_post_data_parameters": ["api_key", "key"],
        "decode_compressed_response": True,
        "record_on_exception": False,
    }


@pytest.fixture(scope="module")
def vcr_cassette_dir(request):
    """Return the cassette directory for the test module."""
    module_name = request.module.__name__.replace("test_", "").replace(".", os.sep)
    return os.path.join(CASSETTE_DIR, module_name)


# Mock API keys for VCR mode
@pytest.fixture(autouse=True)
def mock_api_keys_for_vcr(request):
    """Set mock API keys when running with VCR cassettes."""
    # Check if this test uses VCR
    marker = request.node.get_closest_marker("vcr")
    if not marker:
        return

    # Set mock API keys for the test
    # These are fake keys that match what was used when recording
    if not os.environ.get("ANTHROPIC_API_KEY"):
        os.environ["ANTHROPIC_API_KEY"] = "mock-anthropic-key-for-vcr"
    if not os.environ.get("OPENAI_API_KEY"):
        os.environ["OPENAI_API_KEY"] = "mock-openai-key-for-vcr"
    if not os.environ.get("DEEPSEEK_API_KEY"):
        os.environ["DEEPSEEK_API_KEY"] = "mock-deepseek-key-for-vcr"
    if not os.environ.get("CONTINUUM_API_KEY"):
        os.environ["CONTINUUM_API_KEY"] = "mock-continuum-key-for-vcr"


def has_cassette(test_name, cassette_dir):
    """Check if a cassette exists for the test."""
    cassette_path = os.path.join(cassette_dir, f"{test_name}.yaml")
    return os.path.exists(cassette_path)
