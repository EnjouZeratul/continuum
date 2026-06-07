"""
部署示例 - Docker 容器化部署

本示例展示如何部署 Continuum Agent 应用：
- Docker 镜像构建
- 环境变量配置
- 多容器编排
- 生产环境最佳实践

运行方式:
    1. 构建 Docker 镜像:
       docker build -t continuum-agent .

    2. 运行容器:
       docker run -e ANTHROPIC_API_KEY=your-key continuum-agent

    3. 使用 Docker Compose:
       docker-compose up -d

依赖:
    Docker 和 Docker Compose
"""

import os
import subprocess
import json
from pathlib import Path


def generate_dockerfile():
    """生成 Dockerfile"""
    print("=== 生成 Dockerfile ===\n")

    dockerfile_content = """
# Continuum Agent Docker 镜像
# 多阶段构建，优化镜像大小

# 构建阶段
FROM python:3.11-slim as builder

WORKDIR /app

# 安装构建依赖
RUN pip install --no-cache-dir continuum-agent-sdk

# 复制应用代码
COPY . .

# 生产阶段
FROM python:3.11-slim as production

WORKDIR /app

# 创建非 root 用户
RUN useradd -m -u 1000 continuum

# 复制依赖和应用
COPY --from=builder /usr/local/lib/python3.11/site-packages /usr/local/lib/python3.11/site-packages
COPY --from=builder /app /app

# 设置环境变量
ENV PYTHONUNBUFFERED=1
ENV CONTINUUM_LOG_LEVEL=INFO

# 切换用户
USER continuum

# 健康检查
HEALTHCHECK --interval=30s --timeout=10s --start-period=5s --retries=3 \
    CMD python -c "import continuum_sdk; print('healthy')" || exit 1

# 启动命令
CMD ["python", "main.py"]
"""

    dockerfile_path = Path("examples/deployment/Dockerfile")
    dockerfile_path.parent.mkdir(parents=True, exist_ok=True)
    dockerfile_path.write_text(dockerfile_content.strip())

    print(f"✓ Dockerfile 已生成: {dockerfile_path}")
    return dockerfile_path


def generate_docker_compose():
    """生成 Docker Compose 配置"""
    print("\n=== 生成 Docker Compose 配置 ===\n")

    compose_content = """
version: '3.8'

services:
  # Agent 主服务
  agent:
    build:
      context: .
      dockerfile: Dockerfile
    container_name: continuum-agent
    environment:
      - ANTHROPIC_API_KEY=${ANTHROPIC_API_KEY}
      - OPENAI_API_KEY=${OPENAI_API_KEY}
      - CONTINUUM_LOG_LEVEL=${LOG_LEVEL:-INFO}
    volumes:
      - ./data:/app/data
      - ./logs:/app/logs
    restart: unless-stopped
    healthcheck:
      test: ["CMD", "python", "-c", "import continuum_sdk"]
      interval: 30s
      timeout: 10s
      retries: 3

  # 向量数据库服务
  vector_db:
    image: qdrant/qdrant:latest
    container_name: continuum-vectors
    ports:
      - "6333:6333"
    volumes:
      - vector_data:/qdrant/storage
    restart: unless-stopped

  # Redis 缓存服务
  cache:
    image: redis:7-alpine
    container_name: continuum-cache
    ports:
      - "6379:6379"
    volumes:
      - cache_data:/data
    restart: unless-stopped

  # Prometheus 监控
  prometheus:
    image: prom/prometheus:latest
    container_name: continuum-prometheus
    ports:
      - "9090:9090"
    volumes:
      - ./prometheus.yml:/etc/prometheus/prometheus.yml
      - prometheus_data:/prometheus
    restart: unless-stopped

volumes:
  vector_data:
  cache_data:
  prometheus_data:
"""

    compose_path = Path("examples/deployment/docker-compose.yml")
    compose_path.write_text(compose_content.strip())

    print(f"✓ Docker Compose 已生成: {compose_path}")
    return compose_path


def generate_kubernetes_manifests():
    """生成 Kubernetes 部署清单"""
    print("\n=== 生成 Kubernetes 部署清单 ===\n")

    k8s_dir = Path("examples/deployment/kubernetes")
    k8s_dir.mkdir(parents=True, exist_ok=True)

    # Deployment
    deployment_content = """
apiVersion: apps/v1
kind: Deployment
metadata:
  name: continuum-agent
  labels:
    app: continuum-agent
spec:
  replicas: 3
  selector:
    matchLabels:
      app: continuum-agent
  template:
    metadata:
      labels:
        app: continuum-agent
    spec:
      containers:
      - name: agent
        image: continuum-agent:latest
        ports:
        - containerPort: 8080
        env:
        - name: ANTHROPIC_API_KEY
          valueFrom:
            secretKeyRef:
              name: continuum-secrets
              key: anthropic-api-key
        - name: CONTINUUM_LOG_LEVEL
          value: "INFO"
        resources:
          requests:
            memory: "256Mi"
            cpu: "250m"
          limits:
            memory: "512Mi"
            cpu: "500m"
        livenessProbe:
          exec:
            command:
            - python
            - -c
            - "import continuum_sdk"
          initialDelaySeconds: 10
          periodSeconds: 30
        readinessProbe:
          exec:
            command:
            - python
            - -c
            - "import continuum_sdk"
          initialDelaySeconds: 5
          periodSeconds: 10
"""

    deployment_path = k8s_dir / "deployment.yaml"
    deployment_path.write_text(deployment_content.strip())

    # Service
    service_content = """
apiVersion: v1
kind: Service
metadata:
  name: continuum-agent-service
spec:
  selector:
    app: continuum-agent
  ports:
  - port: 80
    targetPort: 8080
  type: LoadBalancer
"""

    service_path = k8s_dir / "service.yaml"
    service_path.write_text(service_content.strip())

    # ConfigMap
    configmap_content = """
apiVersion: v1
kind: ConfigMap
metadata:
  name: continuum-config
data:
  LOG_LEVEL: "INFO"
  MAX_TOKENS: "4096"
  TEMPERATURE: "0.7"
"""

    configmap_path = k8s_dir / "configmap.yaml"
    configmap_path.write_text(configmap_content.strip())

    print(f"✓ Kubernetes 清单已生成: {k8s_dir}")
    return k8s_dir


def generate_main_app():
    """生成主应用代码"""
    print("\n=== 生成主应用代码 ===\n")

    main_content = """
#!/usr/bin/env python3
"""
Continuum Agent 主应用

生产环境启动脚本，支持：
- 多 Provider 配置
- 环境变量读取
- 优雅关闭
"""

import asyncio
import signal
import os
import logging
from continuum_sdk import Agent, LlmConfig

# 配置日志
logging.basicConfig(
    level=os.getenv("CONTINUUM_LOG_LEVEL", "INFO"),
    format="%(asctime)s - %(name)s - %(levelname)s - %(message)s",
)
logger = logging.getLogger("continuum")

# 全局 Agent 实例
agent = None


def setup_shutdown_handlers():
    """设置优雅关闭处理"""
    loop = asyncio.get_event_loop()

    def shutdown_handler():
        logger.info("收到关闭信号，正在清理...")
        if agent:
            # 保存会话状态
            agent.save_session()
        loop.stop()

    signal.signal(signal.SIGINT, shutdown_handler)
    signal.signal(signal.SIGTERM, shutdown_handler)


async def main():
    """主函数"""
    global agent

    # 从环境变量读取配置
    api_key = os.getenv("ANTHROPIC_API_KEY") or os.getenv("OPENAI_API_KEY")
    if not api_key:
        raise ValueError("需要设置 ANTHROPIC_API_KEY 或 OPENAI_API_KEY")

    # 创建 Agent
    config = LlmConfig(
        provider="anthropic",
        model="claude-sonnet-4-6",
        api_key=api_key,
    )
    agent = Agent(config=config)

    logger.info("Continuum Agent 已启动")

    # 示例：处理请求
    try:
        # 等待外部请求（实际应用中从 API/队列接收）
        await asyncio.sleep(60)  # 模拟运行
    except asyncio.CancelledError:
        logger.info("任务被取消")

    logger.info("Continuum Agent 已关闭")


if __name__ == "__main__":
    setup_shutdown_handlers()
    asyncio.run(main())
"""

    main_path = Path("examples/deployment/main.py")
    main_path.write_text(main_content.strip())

    print(f"✓ 主应用已生成: {main_path}")
    return main_path


def generate_env_example():
    """生成环境变量示例"""
    print("\n=== 生成环境变量示例 ===\n")

    env_content = """
# Continuum Agent 环境变量配置
# 复制此文件为 .env 并填入真实值

# API Keys
ANTHROPIC_API_KEY=your-anthropic-key-here
OPENAI_API_KEY=your-openai-key-here
GEMINI_API_KEY=your-gemini-key-here

# Azure OpenAI (可选)
AZURE_OPENAI_ENDPOINT=https://your-resource.openai.azure.com
AZURE_OPENAI_KEY=your-azure-key

# AWS Bedrock (可选)
AWS_ACCESS_KEY_ID=your-aws-key
AWS_SECRET_ACCESS_KEY=your-aws-secret
AWS_REGION=us-east-1

# Ollama (本地)
OLLAMA_HOST=http://localhost:11434

# 日志配置
CONTINUUM_LOG_LEVEL=INFO

# 模型配置
DEFAULT_MODEL=claude-sonnet-4-6
MAX_TOKENS=4096
TEMPERATURE=0.7
"""

    env_path = Path("examples/deployment/.env.example")
    env_path.write_text(env_content.strip())

    print(f"✓ 环境变量示例已生成: {env_path}")
    return env_path


def generate_prometheus_config():
    """生成 Prometheus 配置"""
    print("\n=== 生成 Prometheus 配置 ===\n")

    prometheus_content = """
global:
  scrape_interval: 15s
  evaluation_interval: 15s

scrape_configs:
  - job_name: 'continuum-agent'
    static_configs:
      - targets: ['agent:8080']
    metrics_path: '/metrics'

  - job_name: 'prometheus'
    static_configs:
      - targets: ['localhost:9090']
"""

    prometheus_path = Path("examples/deployment/prometheus.yml")
    prometheus_path.write_text(prometheus_content.strip())

    print(f"✓ Prometheus 配置已生成: {prometheus_path}")
    return prometheus_path


def main():
    """运行所有部署配置生成"""
    print("=== Continuum 部署配置生成器 ===\n")

    generate_dockerfile()
    generate_docker_compose()
    generate_kubernetes_manifests()
    generate_main_app()
    generate_env_example()
    generate_prometheus_config()

    print("\n✓ 所有部署配置已生成")
    print("\n目录结构:")
    print("  examples/deployment/")
    print("    ├── Dockerfile")
    print("    ├── docker-compose.yml")
    print("    ├── main.py")
    print("    ├── .env.example")
    print("    ├── prometheus.yml")
    print("    └── kubernetes/")
    print("        ├── deployment.yaml")
    print("        ├── service.yaml")
    print("        └── configmap.yaml")

    print("\n使用方式:")
    print("  1. Docker: docker build -t continuum-agent examples/deployment/")
    print("  2. Compose: cd examples/deployment && docker-compose up -d")
    print("  3. Kubernetes: kubectl apply -f examples/deployment/kubernetes/")


if __name__ == "__main__":
    main()