# Playwright MCP Integration

Playwright MCP provides browser automation capabilities for Continuum agents.

## Installation

### Prerequisites

- Node.js >= 18.0
- npm >= 9.0

### Quick Start

```bash
# Install Playwright MCP globally (optional)
npm install -g @playwright/mcp

# Or use npx (recommended)
npx @playwright/mcp@latest --help
```

## Configuration

### Basic Configuration

Add Playwright MCP to your `super.yaml`:

```yaml
tools:
  mcp:
    - name: playwright
      command: npx
      args:
        - "@playwright/mcp@latest"
        - "--headless"
        - "--browser"
        - "chrome"
```

### Advanced Configuration

```yaml
tools:
  mcp:
    - name: playwright
      command: npx
      args:
        - "@playwright/mcp@latest"
        - "--headless"
        - "--browser"
        - "chrome"
        - "--caps"              # Enable additional capabilities
        - "vision,pdf"
        - "--allowed-hosts"     # Security: restrict allowed hosts
        - "trusted-site.com"
        - "--timeout-navigation"
        - "30000"               # Navigation timeout (ms)
```

## Available Tools

Playwright MCP provides the following browser automation tools:

| Tool | Description | Parameters |
|------|-------------|------------|
| `browser_navigate` | Navigate to URL | `url` |
| `browser_click` | Click element | `selector` |
| `browser_type` | Type text | `selector`, `text` |
| `browser_screenshot` | Take screenshot | - |
| `browser_evaluate` | Execute JavaScript | `script` |
| `browser_wait_for` | Wait for element | `selector`, `timeout` |

## Usage Examples

### Web Scraping

```python
# Navigate to page
result = agent.run("Navigate to https://news.ycombinator.com")

# Extract data
result = agent.run("Extract all article titles from the page")
```

### Form Filling

```python
# Fill login form
agent.run("Go to https://example.com/login")
agent.run("Type 'user@example.com' in the email field")
agent.run("Type 'password' in the password field")
agent.run("Click the login button")
```

### Visual Testing

```python
# Enable vision capability for visual analysis
agent.run("Take a screenshot of the homepage")
agent.run("Describe what you see in the screenshot")
```

## Security Considerations

### Host Restriction

Limit which hosts the browser can access:

```yaml
args:
  - "--allowed-hosts"
  - "trusted-domain.com,api.example.com"
```

### Block Origins

Block specific origins:

```yaml
args:
  - "--blocked-origins"
  - "malware-site.com,ads.network"
```

### Headless Mode

Always use headless mode in production:

```yaml
args:
  - "--headless"  # No visible browser window
```

## Browser Options

| Browser | Description |
|---------|-------------|
| `chrome` | Google Chrome (default) |
| `firefox` | Mozilla Firefox |
| `webkit` | Safari/WebKit |
| `msedge` | Microsoft Edge |

## Capabilities

Enable additional capabilities with `--caps`:

| Capability | Description |
|------------|-------------|
| `vision` | AI vision for visual analysis |
| `pdf` | PDF generation |
| `devtools` | Chrome DevTools access |

Example:
```yaml
args:
  - "--caps"
  - "vision,pdf"
```

## Troubleshooting

### Browser not starting

1. Ensure Node.js is installed: `node --version`
2. Check Playwright browsers: `npx playwright install`
3. Verify MCP command: `npx @playwright/mcp@latest --help`

### Connection timeout

Increase navigation timeout:
```yaml
args:
  - "--timeout-navigation"
  - "60000"  # 60 seconds
```

### Memory issues

Use headless mode and close browser sessions:
```yaml
args:
  - "--headless"
  - "--isolated"  # Memory-only profile
```

## Docker Integration

Include Playwright MCP in Docker deployment:

```dockerfile
# Install Node.js
RUN apt-get update && apt-get install -y nodejs npm

# Install Playwright browsers
RUN npx playwright install chromium
```

## References

- [Playwright MCP GitHub](https://github.com/microsoft/playwright-mcp)
- [Playwright Documentation](https://playwright.dev)
- [MCP Protocol](https://modelcontextprotocol.io)