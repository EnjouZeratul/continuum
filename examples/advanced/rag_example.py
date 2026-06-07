"""
RAG (Retrieval-Augmented Generation) 示例

本示例展示如何使用 Continuum SDK 构建 RAG 应用：
- 文档加载与分块
- 向量存储与检索
- 混合搜索（向量 + 关键词）
- 上下文注入与回答生成

运行方式:
    python rag_example.py

依赖:
    pip install continuum-agent-sdk
"""

import asyncio
from continuum_sdk.knowledge import (
    RetrieverEngine,
    DocumentLoader,
    TextSplitter,
    Embeddings,
)


async def basic_rag():
    """基础 RAG 示例：文档问答"""
    print("=== 基础 RAG 示例 ===\n")

    # 1. 创建检索引擎
    engine = RetrieverEngine(embedding_dimension=128)
    print("✓ 检索引擎已创建")

    # 2. 加载文档
    loader = DocumentLoader(loader_type="text")
    print("✓ 文档加载器已创建")

    # 3. 添加文档到知识库
    documents = [
        ("doc_1", "Python 是一种高级编程语言，由 Guido van Rossum 创建。它的设计哲学强调代码可读性。"),
        ("doc_2", "Rust 是一种系统编程语言，专注于安全、并发和性能。它由 Mozilla Research 开发。"),
        ("doc_3", "TypeScript 是 JavaScript 的超集，添加了静态类型检查。由 Microsoft 开发维护。"),
    ]

    for doc_id, content in documents:
        engine.add_document(doc_id, content, None)
        print(f"  ✓ 文档 '{doc_id}' 已添加")

    # 4. 创建文本分割器
    splitter = TextSplitter(chunk_size=500, chunk_overlap=50)
    print("✓ 文本分割器已创建")

    # 5. 执行检索
    query = "什么语言专注于安全？"
    results = engine.retrieve(query, top_k=3)

    print(f"\n查询: {query}")
    print(f"检索结果 ({len(results)} 条):")
    for i, result in enumerate(results, 1):
        print(f"\n  [{i}] 分数: {result.score:.4f}")
        print(f"      内容: {result.content[:100]}...")

    # 6. 清理
    count = engine.count()
    print(f"\n知识库文档数: {count}")


async def hybrid_search():
    """混合搜索示例：向量 + 关键词"""
    print("\n=== 混合搜索示例 ===\n")

    engine = RetrieverEngine(embedding_dimension=256)

    # 添加技术文档
    tech_docs = [
        ("api_rest", "REST API 是一种基于 HTTP 协议的接口设计风格，使用标准 HTTP 方法进行 CRUD 操作。"),
        ("api_graphql", "GraphQL 是一种查询语言，允许客户端精确指定需要的数据字段，避免过度获取。"),
        ("api_grpc", "gRPC 是高性能 RPC 框架，使用 Protocol Buffers 序列化，支持双向流通信。"),
        ("api_websocket", "WebSocket 提供全双工通信通道，适合实时应用如聊天、游戏和协作工具。"),
    ]

    for doc_id, content in tech_docs:
        engine.add_document(doc_id, content, None)

    # 向量相似度搜索
    query = "实时通信用什么技术？"
    results = engine.retrieve(query, top_k=2)

    print(f"查询: {query}")
    print(f"最相关的结果:")
    for result in results:
        print(f"  - [{result.id}] 分数: {result.score:.4f}")
        print(f"    {result.content[:80]}...")


async def document_pipeline():
    """文档处理流水线示例"""
    print("\n=== 文档处理流水线 ===\n")

    # 创建组件
    loader = DocumentLoader(loader_type="text")
    splitter = TextSplitter(chunk_size=200, chunk_overlap=30)
    embeddings = Embeddings(model="text-embedding-3-small")

    # 模拟长文档
    long_document = """
Continuum 是一个现代化的 Agent 框架，提供了完整的开发工具链。

核心特性包括：
1. 多模型支持 - 支持 Anthropic、OpenAI、Gemini 等主流 LLM
2. 工具系统 - 内置文件操作、Shell 执行、搜索等工具
3. RAG 支持 - 文档加载、向量化、混合检索
4. 会话管理 - 支持检查点、恢复、历史记录

架构设计采用分层结构：
- Layer 0: 安全层（密钥管理、输入验证）
- Layer 1: 基础层（LLM 客户端、流式处理）
- Layer 2: 核心层（Agent 运行时、会话管理）
- Layer 3: 能力层（工具、RAG、Memory）
- Layer 4: 集成层（MCP、审计日志）

开发者可以通过 YAML 配置或 Python API 使用 Continuum。
    """.strip()

    # 分割文档
    chunks = splitter.split(long_document)
    print(f"文档分割为 {len(chunks)} 个块:")

    for i, (content, index, start) in enumerate(chunks):
        print(f"\n  块 {index + 1} (位置 {start}):")
        print(f"  {content[:100]}...")

    # 生成嵌入向量
    sample_text = "Continuum 支持多模型"
    embedding = embeddings.embed(sample_text)
    print(f"\n嵌入向量维度: {len(embedding)}")

    # 批量嵌入
    texts = ["这是第一段文本", "这是第二段文本", "这是第三段文本"]
    batch_embeddings = embeddings.embed_batch(texts)
    print(f"批量嵌入数量: {len(batch_embeddings)}")


async def main():
    """运行所有示例"""
    await basic_rag()
    await hybrid_search()
    await document_pipeline()
    print("\n✓ 所有 RAG 示例完成")


if __name__ == "__main__":
    asyncio.run(main())
