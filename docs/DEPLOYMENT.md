# Continuum Deployment Guide

## Quick Start

### Local Development with Docker

```bash
# Build the image
docker build -t continuum:latest .

# Run the CLI
docker run -it continuum:latest

# Run with API key
docker run -it -e ANTHROPIC_API_KEY=your-key continuum:latest
```

### Docker Compose (Development)

```bash
# Start development environment
docker compose up dev

# Start with observability stack
docker compose --profile observability up

# Access services:
# - Continuum CLI: docker compose exec dev continuum
# - Prometheus: http://localhost:9090
# - Grafana: http://localhost:3000 (admin/admin)
```

## Kubernetes Deployment

### Prerequisites

- Kubernetes cluster (minikube, GKE, EKS, etc.)
- kubectl configured
- Docker image built and pushed to registry

### Steps

1. **Build and push image:**
   ```bash
   docker build -t continuum:latest .
   docker tag continuum:latest your-registry/continuum:latest
   docker push your-registry/continuum:latest
   ```

2. **Update image reference in deployment.yaml:**
   ```yaml
   image: your-registry/continuum:latest
   ```

3. **Set secrets:**
   ```bash
   kubectl create secret generic continuum-secrets \
     --from-literal=ANTHROPIC_API_KEY=your-key \
     --from-literal=OPENAI_API_KEY=your-key \
     -n continuum
   ```

4. **Deploy:**
   ```bash
   kubectl apply -f deploy/k8s/deployment.yaml
   ```

5. **Verify:**
   ```bash
   kubectl get pods -n continuum
   kubectl logs -f deployment/continuum -n continuum
   ```

### Scaling

The deployment includes a HorizontalPodAutoscaler (HPA) that automatically scales based on CPU/memory usage:

- Min replicas: 1
- Max replicas: 5
- CPU threshold: 70%
- Memory threshold: 80%

```bash
# Check HPA status
kubectl get hpa -n continuum
```

## Environment Variables

| Variable | Description | Required |
|----------|-------------|----------|
| `ANTHROPIC_API_KEY` | Anthropic API key | Optional |
| `OPENAI_API_KEY` | OpenAI API key | Optional |
| `CONTINUUM_API_KEY` | Continuum API key | Optional |
| `RUST_LOG` | Log level (debug, info, warn, error) | Optional |
| `CONTINUUM_HOME` | Config directory | Optional |

## Observability

### Enable Observability Profile

```bash
docker compose --profile observability up
```

This starts:
- OpenTelemetry Collector (ports 4317, 4318)
- Prometheus (port 9090)
- Grafana (port 3000)

### Enable OTLP in Continuum

Build with `otel` feature:
```bash
cargo build --features otel
```

Or in Docker:
```dockerfile
RUN cargo build --release --features otel
```

## Troubleshooting

### Docker build fails

Check Rust version and dependencies:
```bash
rustc --version
cargo --version
```

### Container can't connect to API

Ensure environment variables are set:
```bash
docker run -it -e ANTHROPIC_API_KEY=sk-xxx continuum:latest
```

### Kubernetes pod not starting

Check events:
```bash
kubectl describe pod -n continuum
kubectl logs -n continuum <pod-name>
```