"""SSRF Protection Tests for encode_image_from_url.

Tests that the function properly blocks:
- Private IP ranges (10.0.0.0/8, 172.16.0.0/12, 192.168.0.0/16)
- Localhost addresses (127.0.0.1, localhost, 0.0.0.0)
- Link-local addresses (169.254.0.0/16)
- Non-HTTP schemes (file://, ftp://, etc.)
- DNS rebinding attacks
- Open redirects to private IPs
"""

import socket
import sys
import os
from unittest import mock
import pytest

from continuum_sdk.python_impl import PythonMultimodalHandler


class TestSSRFPrivateIPRanges:
    """Test that private IP ranges are blocked."""

    @pytest.fixture
    def handler(self):
        return PythonMultimodalHandler()

    def _test_url_blocked(self, handler, ip_address):
        """Helper to test if an IP is blocked."""
        with pytest.raises(ValueError, match="private|internal|blocked"):
            handler._validate_url_for_ssrf(f"http://{ip_address}/image.png")

    def test_private_range_10_block(self, handler):
        """10.0.0.0/8 should be blocked."""
        test_ips = [
            "10.0.0.1",
            "10.1.2.3",
            "10.255.255.255",
        ]
        for ip in test_ips:
            with mock.patch('socket.getaddrinfo', return_value=[
                (socket.AF_INET, socket.SOCK_STREAM, 6, '', (ip, 80))
            ]):
                with pytest.raises(ValueError, match="private"):
                    handler.encode_image_from_url(f"http://example.com/image.png")

    def test_private_range_172_16_block(self, handler):
        """172.16.0.0/12 should be blocked."""
        test_ips = [
            "172.16.0.1",
            "172.20.0.1",
            "172.31.255.255",
        ]
        for ip in test_ips:
            with mock.patch('socket.getaddrinfo', return_value=[
                (socket.AF_INET, socket.SOCK_STREAM, 6, '', (ip, 80))
            ]):
                with pytest.raises(ValueError, match="private"):
                    handler.encode_image_from_url(f"http://example.com/image.png")

    def test_private_range_192_168_block(self, handler):
        """192.168.0.0/16 should be blocked."""
        test_ips = [
            "192.168.0.1",
            "192.168.1.1",
            "192.168.255.255",
        ]
        for ip in test_ips:
            with mock.patch('socket.getaddrinfo', return_value=[
                (socket.AF_INET, socket.SOCK_STREAM, 6, '', (ip, 80))
            ]):
                with pytest.raises(ValueError, match="private"):
                    handler.encode_image_from_url(f"http://example.com/image.png")


class TestSSRFLocalhost:
    """Test that localhost addresses are blocked."""

    @pytest.fixture
    def handler(self):
        return PythonMultimodalHandler()

    def test_localhost_hostname_blocked(self, handler):
        """localhost hostname should be blocked."""
        with pytest.raises(ValueError, match="[Bb]locked|private"):
            handler._validate_url_for_ssrf("http://localhost/image.png")

    def test_127_0_0_1_blocked(self, handler):
        """127.0.0.1 should be blocked."""
        with mock.patch('socket.getaddrinfo', return_value=[
            (socket.AF_INET, socket.SOCK_STREAM, 6, '', ('127.0.0.1', 80))
        ]):
            with pytest.raises(ValueError, match="private"):
                handler.encode_image_from_url("http://example.com/image.png")

    def test_127_any_blocked(self, handler):
        """Any 127.x.x.x should be blocked."""
        test_ips = ["127.0.0.1", "127.1.1.1", "127.255.255.255"]
        for ip in test_ips:
            with mock.patch('socket.getaddrinfo', return_value=[
                (socket.AF_INET, socket.SOCK_STREAM, 6, '', (ip, 80))
            ]):
                with pytest.raises(ValueError, match="private"):
                    handler.encode_image_from_url("http://example.com/image.png")

    def test_0_0_0_0_blocked(self, handler):
        """0.0.0.0 should be blocked."""
        with mock.patch('socket.getaddrinfo', return_value=[
            (socket.AF_INET, socket.SOCK_STREAM, 6, '', ('0.0.0.0', 80))
        ]):
            with pytest.raises(ValueError, match="private"):
                handler.encode_image_from_url("http://example.com/image.png")


class TestSSRFLinkLocal:
    """Test that link-local addresses are blocked."""

    @pytest.fixture
    def handler(self):
        return PythonMultimodalHandler()

    def test_169_254_block(self, handler):
        """169.254.0.0/16 (link-local) should be blocked."""
        test_ips = ["169.254.0.1", "169.254.1.1", "169.254.255.255"]
        for ip in test_ips:
            with mock.patch('socket.getaddrinfo', return_value=[
                (socket.AF_INET, socket.SOCK_STREAM, 6, '', (ip, 80))
            ]):
                with pytest.raises(ValueError, match="private"):
                    handler.encode_image_from_url("http://example.com/image.png")


class TestSSRFSchemeValidation:
    """Test that non-HTTP schemes are blocked."""

    @pytest.fixture
    def handler(self):
        return PythonMultimodalHandler()

    def test_file_scheme_blocked(self, handler):
        """file:// scheme should be blocked."""
        with pytest.raises(ValueError, match="scheme"):
            handler.encode_image_from_url("file:///etc/passwd")

    def test_ftp_scheme_blocked(self, handler):
        """ftp:// scheme should be blocked."""
        with pytest.raises(ValueError, match="scheme"):
            handler.encode_image_from_url("ftp://example.com/image.png")

    def test_gopher_scheme_blocked(self, handler):
        """gopher:// scheme should be blocked."""
        with pytest.raises(ValueError, match="scheme"):
            handler.encode_image_from_url("gopher://example.com/image.png")

    def test_data_scheme_blocked(self, handler):
        """data:// scheme should be blocked."""
        with pytest.raises(ValueError, match="scheme"):
            handler.encode_image_from_url("data:image/png;base64,abc123")

    def test_http_allowed(self, handler):
        """http:// should be allowed for public IPs."""
        # Test validation only, not actual fetch
        with mock.patch('socket.getaddrinfo', return_value=[
            (socket.AF_INET, socket.SOCK_STREAM, 6, '', ('93.184.216.34', 80))
        ]):
            # Should not raise for validation
            result = handler._validate_url_for_ssrf("http://example.com/image.png")
            assert result == "example.com"

    def test_https_allowed(self, handler):
        """https:// should be allowed for public IPs."""
        with mock.patch('socket.getaddrinfo', return_value=[
            (socket.AF_INET, socket.SOCK_STREAM, 6, '', ('93.184.216.34', 443))
        ]):
            result = handler._validate_url_for_ssrf("https://example.com/image.png")
            assert result == "example.com"


class TestSSRFDNSRebinding:
    """Test DNS rebinding protection."""

    @pytest.fixture
    def handler(self):
        return PythonMultimodalHandler()

    def test_dns_to_private_ip_blocked(self, handler):
        """DNS resolving to private IP should be blocked."""
        with mock.patch('socket.getaddrinfo', return_value=[
            (socket.AF_INET, socket.SOCK_STREAM, 6, '', ('192.168.1.1', 80))
        ]):
            with pytest.raises(ValueError, match="private"):
                handler.encode_image_from_url("http://evil.example.com/image.png")

    def test_dns_to_localhost_blocked(self, handler):
        """DNS resolving to localhost should be blocked."""
        with mock.patch('socket.getaddrinfo', return_value=[
            (socket.AF_INET, socket.SOCK_STREAM, 6, '', ('127.0.0.1', 80))
        ]):
            with pytest.raises(ValueError, match="private"):
                handler.encode_image_from_url("http://evil.example.com/image.png")

    def test_dns_to_public_ip_allowed(self, handler):
        """DNS resolving to public IP should be allowed."""
        with mock.patch('socket.getaddrinfo', return_value=[
            (socket.AF_INET, socket.SOCK_STREAM, 6, '', ('93.184.216.34', 80))
        ]):
            # Should not raise
            result = handler._validate_url_for_ssrf("http://example.com/image.png")
            assert result == "example.com"


class TestSSRFOpenRedirect:
    """Test open redirect protection."""

    @pytest.fixture
    def handler(self):
        return PythonMultimodalHandler()

    def test_redirect_to_private_ip_blocked(self, handler):
        """Redirects to private IPs should be blocked."""
        import urllib.error

        def mock_urlopen(request, timeout=None):
            # First call returns redirect to private IP
            headers = {"Location": "http://192.168.1.1/evil.png"}
            err = urllib.error.HTTPError(
                request.full_url if hasattr(request, 'full_url') else str(request),
                302,
                "Found",
                headers,
                None
            )
            err.headers = headers
            raise err

        with mock.patch('urllib.request.urlopen', side_effect=mock_urlopen):
            with mock.patch('socket.getaddrinfo', side_effect=[
                [(socket.AF_INET, socket.SOCK_STREAM, 6, '', ('93.184.216.34', 80))],  # Initial URL
                [(socket.AF_INET, socket.SOCK_STREAM, 6, '', ('192.168.1.1', 80))],   # Redirect target
            ]):
                with pytest.raises(ValueError, match="private"):
                    handler.encode_image_from_url("http://example.com/redirect")

    def test_max_redirects_enforced(self, handler):
        """Max redirects should be enforced."""
        import urllib.error

        redirect_count = 0

        def mock_urlopen(request, timeout=None):
            nonlocal redirect_count
            redirect_count += 1
            # Create a proper HTTPError for redirect simulation
            headers = {"Location": f"http://example.com/redirect{redirect_count}"}
            # HTTPError needs a valid response object
            err = urllib.error.HTTPError(
                request.full_url if hasattr(request, 'full_url') else str(request),
                302,
                "Found",
                headers,
                None
            )
            err.headers = headers
            raise err

        with mock.patch('socket.getaddrinfo', return_value=[
            (socket.AF_INET, socket.SOCK_STREAM, 6, '', ('93.184.216.34', 80))
        ]):
            with mock.patch('urllib.request.urlopen', side_effect=mock_urlopen):
                with pytest.raises(ValueError, match="redirect"):
                    handler.encode_image_from_url("http://example.com/start")


class TestSSRFPublicURLAllowed:
    """Test that valid public URLs work correctly."""

    @pytest.fixture
    def handler(self):
        return PythonMultimodalHandler()

    def test_public_url_allowed(self, handler):
        """Public URLs with public IPs should work."""
        # Create a proper mock response object
        from io import BytesIO

        mock_response = BytesIO(b"fake_image_data")
        mock_response.geturl = lambda: "http://example.com/image.png"
        mock_response.headers = {"Content-Type": "image/png"}
        mock_response.__enter__ = lambda self: self
        mock_response.__exit__ = lambda self, *args: None

        def mock_urlopen(request, timeout=None):
            return mock_response

        with mock.patch('urllib.request.urlopen', side_effect=mock_urlopen):
            with mock.patch('socket.getaddrinfo', return_value=[
                (socket.AF_INET, socket.SOCK_STREAM, 6, '', ('93.184.216.34', 80))
            ]):
                result = handler.encode_image_from_url("http://example.com/image.png")
                assert result["type"] == "image"
                assert "source" in result
                assert result["source"]["type"] == "base64"


class TestSSRFIPv6:
    """Test IPv6 address handling."""

    @pytest.fixture
    def handler(self):
        return PythonMultimodalHandler()

    def test_ipv6_loopback_blocked(self, handler):
        """IPv6 loopback (::1) should be blocked."""
        with mock.patch('socket.getaddrinfo', return_value=[
            (socket.AF_INET6, socket.SOCK_STREAM, 6, '', ('::1', 80))
        ]):
            with pytest.raises(ValueError, match="private"):
                handler.encode_image_from_url("http://example.com/image.png")

    def test_ipv6_link_local_blocked(self, handler):
        """IPv6 link-local (fe80::) should be blocked."""
        with mock.patch('socket.getaddrinfo', return_value=[
            (socket.AF_INET6, socket.SOCK_STREAM, 6, '', ('fe80::1', 80))
        ]):
            with pytest.raises(ValueError, match="private"):
                handler.encode_image_from_url("http://example.com/image.png")

    def test_ipv6_ula_blocked(self, handler):
        """IPv6 ULA (fc00::) should be blocked."""
        with mock.patch('socket.getaddrinfo', return_value=[
            (socket.AF_INET6, socket.SOCK_STREAM, 6, '', ('fc00::1', 80))
        ]):
            with pytest.raises(ValueError, match="private"):
                handler.encode_image_from_url("http://example.com/image.png")

    def test_ipv6_public_allowed(self, handler):
        """Public IPv6 addresses should be allowed."""
        with mock.patch('socket.getaddrinfo', return_value=[
            (socket.AF_INET6, socket.SOCK_STREAM, 6, '', ('2001:4860:4860::8888', 80))
        ]):
            # Should not raise for validation
            result = handler._validate_url_for_ssrf("http://example.com/image.png")
            assert result == "example.com"


class TestSSRFEdgeCases:
    """Test edge cases and attack vectors."""

    @pytest.fixture
    def handler(self):
        return PythonMultimodalHandler()

    def test_ipv4_mapped_ipv6_blocked(self, handler):
        """IPv4-mapped IPv6 addresses (like ::ffff:127.0.0.1) should be blocked."""
        with mock.patch('socket.getaddrinfo', return_value=[
            (socket.AF_INET6, socket.SOCK_STREAM, 6, '', ('::ffff:127.0.0.1', 80))
        ]):
            with pytest.raises(ValueError, match="private"):
                handler.encode_image_from_url("http://example.com/image.png")

    def test_decimal_ip_blocked(self, handler):
        """Decimal IP representation should be blocked after resolution."""
        # 2130706433 = 127.0.0.1 in decimal
        with mock.patch('socket.getaddrinfo', return_value=[
            (socket.AF_INET, socket.SOCK_STREAM, 6, '', ('127.0.0.1', 80))
        ]):
            with pytest.raises(ValueError, match="private"):
                handler.encode_image_from_url("http://2130706433/image.png")

    def test_hex_ip_blocked(self, handler):
        """Hex IP representation should be blocked after resolution."""
        # 0x7f000001 = 127.0.0.1 in hex
        with mock.patch('socket.getaddrinfo', return_value=[
            (socket.AF_INET, socket.SOCK_STREAM, 6, '', ('127.0.0.1', 80))
        ]):
            with pytest.raises(ValueError, match="private"):
                handler.encode_image_from_url("http://0x7f000001/image.png")

    def test_missing_hostname_blocked(self, handler):
        """URLs without hostname should be rejected."""
        with pytest.raises(ValueError, match="hostname"):
            handler._validate_url_for_ssrf("http:///image.png")

    def test_dns_failure_blocked(self, handler):
        """DNS resolution failure should be blocked."""
        with mock.patch('socket.getaddrinfo', side_effect=socket.gaierror("DNS failed")):
            with pytest.raises(ValueError, match="resolve"):
                handler.encode_image_from_url("http://nonexistent.invalid/image.png")


class TestSSRFHelperFunctions:
    """Test SSRF helper functions directly."""

    @pytest.fixture
    def handler(self):
        return PythonMultimodalHandler()

    def test_is_private_ip_private_ranges(self, handler):
        """Test _is_private_ip for private ranges."""
        assert handler._is_private_ip("10.0.0.1")
        assert handler._is_private_ip("172.16.0.1")
        assert handler._is_private_ip("192.168.1.1")
        assert handler._is_private_ip("127.0.0.1")
        assert handler._is_private_ip("169.254.1.1")
        assert handler._is_private_ip("0.0.0.0")

    def test_is_private_ip_public_ips(self, handler):
        """Test _is_private_ip for public IPs."""
        assert not handler._is_private_ip("8.8.8.8")
        assert not handler._is_private_ip("93.184.216.34")
        assert not handler._is_private_ip("1.1.1.1")

    def test_is_private_ip_ipv6(self, handler):
        """Test _is_private_ip for IPv6."""
        assert handler._is_private_ip("::1")
        assert handler._is_private_ip("fe80::1")
        assert handler._is_private_ip("fc00::1")
        assert not handler._is_private_ip("2001:4860:4860::8888")

    def test_is_private_ip_invalid(self, handler):
        """Invalid IPs should be treated as private for safety."""
        assert handler._is_private_ip("invalid-ip")
        assert handler._is_private_ip("")


if __name__ == "__main__":
    pytest.main([__file__, "-v"])
