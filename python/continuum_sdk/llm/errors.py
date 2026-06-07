"""
LLM Errors

Error types for LLM client operations.
"""


class LlmError(Exception):
    """
    Base error for LLM operations.

    All LLM-related errors inherit from this class.
    """

    def __init__(self, message: str, provider: str | None = None):
        super().__init__(message)
        self.provider = provider

    def __str__(self) -> str:
        if self.provider:
            return f"[{self.provider}] {super().__str__()}"
        return super().__str__()


class AuthenticationError(LlmError):
    """
    Raised when API authentication fails.

    Common causes:
        - Invalid API key
        - Expired API key
        - Wrong API key for provider
    """

    pass


class RateLimitError(LlmError):
    """
    Raised when rate limit is exceeded.

    Attributes:
        retry_after: Seconds to wait before retry (if provided)
    """

    def __init__(
        self,
        message: str,
        provider: str | None = None,
        retry_after: float | None = None,
    ):
        super().__init__(message, provider)
        self.retry_after = retry_after


class NetworkError(LlmError):
    """
    Raised when network connection fails.

    Common causes:
        - DNS resolution failure
        - Connection refused
        - SSL/TLS errors
    """

    pass


class TimeoutError(LlmError):
    """
    Raised when request times out.

    Attributes:
        timeout: The timeout duration in seconds
    """

    def __init__(
        self,
        message: str,
        provider: str | None = None,
        timeout: float | None = None,
    ):
        super().__init__(message, provider)
        self.timeout = timeout


class InvalidResponseError(LlmError):
    """
    Raised when API returns invalid response.

    Common causes:
        - Malformed JSON
        - Missing required fields
        - Unexpected response format
    """

    def __init__(
        self,
        message: str,
        provider: str | None = None,
        response_data: dict | None = None,
    ):
        super().__init__(message, provider)
        self.response_data = response_data


class ContentFilterError(LlmError):
    """
    Raised when content is filtered by provider safety systems.

    Common causes:
        - Harmful content detected
        - Policy violation
    """

    def __init__(
        self,
        message: str,
        provider: str | None = None,
        filter_reason: str | None = None,
    ):
        super().__init__(message, provider)
        self.filter_reason = filter_reason


class ModelNotFoundError(LlmError):
    """
    Raised when specified model is not available.
    """

    pass


class InsufficientQuotaError(LlmError):
    """
    Raised when account quota is exceeded.
    """

    pass


def _build_api_key_error_message(
    status_code: int, provider: str, response_body: str
) -> str:
    """
    Build detailed, user-friendly API key error message.

    Args:
        status_code: HTTP status code (401 or 403)
        provider: Provider name
        response_body: Response body for additional context

    Returns:
        Detailed error message with cause and solution
    """
    provider_display = provider.upper() if provider else "API"

    # Detect specific error type from response body
    body_lower = response_body.lower() if response_body else ""
    is_invalid_key = "invalid" in body_lower or "incorrect" in body_lower
    is_expired_key = "expired" in body_lower
    is_missing_key = "missing" in body_lower or "required" in body_lower
    is_permission_denied = "permission" in body_lower or "forbidden" in body_lower
    is_billing = (
        "billing" in body_lower or "quota" in body_lower or "payment" in body_lower
    )

    if status_code == 401:
        if is_invalid_key:
            return (
                f"{provider_display} API key is invalid or incorrect.\n"
                f"\n"
                f"Possible causes:\n"
                f"  1. API key was mistyped or copied incorrectly\n"
                f"  2. API key was revoked or regenerated\n"
                f"  3. Using the wrong key for this provider\n"
                f"\n"
                f"Solutions:\n"
                f"  - Verify your API key in the provider dashboard\n"
                f"  - Copy the complete key (no trailing spaces)\n"
                f"  - Set via environment variable: export {provider.upper()}_API_KEY=your-key\n"
                f"  - Or in config: api_key = 'your-key'"
            )
        elif is_expired_key:
            return (
                f"{provider_display} API key has expired.\n"
                f"\n"
                f"Solutions:\n"
                f"  - Generate a new API key from the provider dashboard\n"
                f"  - Update your configuration with the new key"
            )
        elif is_missing_key:
            return (
                f"{provider_display} API key is missing.\n"
                f"\n"
                f"Solutions:\n"
                f"  - Set environment variable: export {provider.upper()}_API_KEY=your-key\n"
                f"  - Or pass api_key parameter when creating the client\n"
                f"  - Or add to config file: api_key = 'your-key'"
            )
        else:
            return (
                f"{provider_display} authentication failed (HTTP 401).\n"
                f"\n"
                f"Possible causes:\n"
                f"  1. API key is invalid, expired, or missing\n"
                f"  2. Wrong API key format for this provider\n"
                f"  3. API key lacks required permissions\n"
                f"\n"
                f"Solutions:\n"
                f"  - Verify your API key is correct and active\n"
                f"  - Check provider dashboard for key status\n"
                f"  - Set via: export {provider.upper()}_API_KEY=your-key\n"
                f"\n"
                f"Server response: {response_body[:200] if response_body else '(none)'}"
            )
    else:  # 403
        if is_billing:
            return (
                f"{provider_display} access denied - billing/quota issue.\n"
                f"\n"
                f"Possible causes:\n"
                f"  1. Account has insufficient credits or quota\n"
                f"  2. Payment method expired or invalid\n"
                f"  3. Trial period expired\n"
                f"\n"
                f"Solutions:\n"
                f"  - Check your billing status in the provider dashboard\n"
                f"  - Add credits or update payment method\n"
                f"  - Verify your plan includes access to this model"
            )
        elif is_permission_denied:
            return (
                f"{provider_display} API key lacks required permissions.\n"
                f"\n"
                f"Possible causes:\n"
                f"  1. API key scoped to limited endpoints\n"
                f"  2. Model not available on your plan\n"
                f"  3. Feature requires additional approval\n"
                f"\n"
                f"Solutions:\n"
                f"  - Check API key permissions in provider dashboard\n"
                f"  - Upgrade your plan if needed\n"
                f"  - Contact support if you believe this is an error"
            )
        else:
            return (
                f"{provider_display} access forbidden (HTTP 403).\n"
                f"\n"
                f"Possible causes:\n"
                f"  1. API key valid but lacks permissions\n"
                f"  2. Account suspended or restricted\n"
                f"  3. Geographic or IP restriction\n"
                f"  4. Model not available on your plan\n"
                f"\n"
                f"Solutions:\n"
                f"  - Verify API key permissions\n"
                f"  - Check account status in provider dashboard\n"
                f"  - Ensure model is available on your plan\n"
                f"\n"
                f"Server response: {response_body[:200] if response_body else '(none)'}"
            )


def classify_http_error(
    status_code: int,
    response_body: str,
    provider: str,
) -> LlmError:
    """
    Classify HTTP error into appropriate LlmError subclass.

    Args:
        status_code: HTTP status code
        response_body: Response body text
        provider: Provider name

    Returns:
        Appropriate LlmError subclass instance
    """
    if status_code == 401:
        return AuthenticationError(
            _build_api_key_error_message(401, provider, response_body),
            provider=provider,
        )
    elif status_code == 403:
        return AuthenticationError(
            _build_api_key_error_message(403, provider, response_body),
            provider=provider,
        )
    elif status_code == 404:
        return ModelNotFoundError(
            "Model or endpoint not found.",
            provider=provider,
        )
    elif status_code == 429:
        return RateLimitError(
            "Rate limit exceeded. Please wait and retry.",
            provider=provider,
        )
    elif status_code == 500:
        return LlmError(
            f"Server error: {response_body}",
            provider=provider,
        )
    elif status_code == 502:
        return NetworkError(
            "Bad gateway. Provider may be experiencing issues.",
            provider=provider,
        )
    elif status_code == 503:
        return NetworkError(
            "Service unavailable. Provider may be down.",
            provider=provider,
        )
    elif status_code == 504:
        return TimeoutError(
            "Gateway timeout. Request took too long.",
            provider=provider,
        )
    else:
        return LlmError(
            f"HTTP {status_code}: {response_body}",
            provider=provider,
        )
