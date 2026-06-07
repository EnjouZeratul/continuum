//! MCP 传输层
//!
//! 支持 stdio、TCP 和 Unix socket 三种传输方式。

use async_trait::async_trait;
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt, BufReader};
#[cfg(unix)]
use tokio::net::UnixStream;
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::Mutex;

use super::protocol::McpMessage;
use anyhow::{anyhow, Result};

/// 传输类型
#[derive(Debug, Clone)]
pub enum McpTransportType {
    /// 标准输入输出
    Stdio {
        /// 命令
        command: String,
        /// 参数
        args: Vec<String>,
    },
    /// TCP Socket
    Tcp {
        /// 地址
        addr: String,
    },
    /// Unix Socket (仅 Unix 系统)
    #[cfg(unix)]
    Unix {
        /// 路径
        path: String,
    },
}

/// MCP 传输 trait
#[async_trait]
pub trait McpTransport: Send + Sync {
    /// 发送消息
    async fn send(&self, message: &McpMessage) -> Result<()>;

    /// 接收消息
    async fn receive(&self) -> Result<Option<McpMessage>>;

    /// 关闭传输
    async fn close(&self) -> Result<()>;
}

/// Stdio 传输实现
pub struct StdioTransport {
    /// 子进程
    process: Arc<Mutex<Option<tokio::process::Child>>>,
    /// 标准输入
    stdin: Arc<Mutex<Option<tokio::process::ChildStdin>>>,
    /// 标准输出读取器
    stdout: Arc<Mutex<Option<tokio::io::BufReader<tokio::process::ChildStdout>>>>,
}

impl StdioTransport {
    /// 创建新的 Stdio 传输
    pub fn new(_command: &str, _args: &[String]) -> Result<Self> {
        Ok(Self {
            process: Arc::new(Mutex::new(None)),
            stdin: Arc::new(Mutex::new(None)),
            stdout: Arc::new(Mutex::new(None)),
        })
    }

    /// 启动子进程
    pub async fn start(&self, command: &str, args: &[String]) -> Result<()> {
        use std::process::Stdio;

        let mut cmd = tokio::process::Command::new(command);
        cmd.args(args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null());

        let mut child = cmd.spawn()?;

        let stdin = child
            .stdin
            .take()
            .ok_or_else(|| anyhow!("Failed to open stdin"))?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| anyhow!("Failed to open stdout"))?;

        *self.stdin.lock().await = Some(stdin);
        *self.stdout.lock().await = Some(BufReader::new(stdout));
        *self.process.lock().await = Some(child);

        Ok(())
    }
}

#[async_trait]
impl McpTransport for StdioTransport {
    async fn send(&self, message: &McpMessage) -> Result<()> {
        let mut stdin_guard = self.stdin.lock().await;
        let stdin = stdin_guard
            .as_mut()
            .ok_or_else(|| anyhow!("Transport not started"))?;

        let json = serde_json::to_string(message)?;
        let frame = format!("Content-Length: {}\r\n\r\n{}", json.len(), json);
        stdin.write_all(frame.as_bytes()).await?;
        stdin.flush().await?;
        Ok(())
    }

    async fn receive(&self) -> Result<Option<McpMessage>> {
        let mut stdout_guard = self.stdout.lock().await;
        let stdout = stdout_guard
            .as_mut()
            .ok_or_else(|| anyhow!("Transport not started"))?;

        // 读取 Content-Length 头
        let mut header_buf = vec![0u8; 1024];
        let mut total_read = 0;

        loop {
            let n = stdout.read(&mut header_buf[total_read..]).await?;
            if n == 0 {
                return Ok(None); // 连接关闭
            }
            total_read += n;

            // 查找 \r\n\r\n 分隔符
            if let Some(pos) = find_header_end(&header_buf[..total_read]) {
                let header = String::from_utf8_lossy(&header_buf[..pos]);
                let content_length = parse_content_length(&header)?;

                // 读取消息体
                let header_size = pos + 4; // 包含 \r\n\r\n
                let body_size = content_length;
                let mut body_buf = vec![0u8; body_size];

                // 处理已经读取的部分
                let already_read = total_read - header_size;
                if already_read > 0 {
                    body_buf[..already_read].copy_from_slice(&header_buf[header_size..total_read]);
                }

                // 读取剩余部分
                if already_read < body_size {
                    stdout.read_exact(&mut body_buf[already_read..]).await?;
                }

                let message: McpMessage = serde_json::from_slice(&body_buf)?;
                return Ok(Some(message));
            }

            if total_read >= header_buf.len() {
                return Err(anyhow!("Header too large"));
            }
        }
    }

    async fn close(&self) -> Result<()> {
        let mut process_guard = self.process.lock().await;
        if let Some(mut process) = process_guard.take() {
            process.kill().await?;
        }
        Ok(())
    }
}

/// TCP 传输实现
pub struct TcpTransport {
    /// 连接流
    stream: Arc<Mutex<Option<TcpStream>>>,
    /// 服务器监听器 (服务端模式)
    listener: Arc<Mutex<Option<TcpListener>>>,
}

impl TcpTransport {
    /// 创建客户端连接
    pub async fn connect(addr: &str) -> Result<Self> {
        let stream = TcpStream::connect(addr).await?;
        Ok(Self {
            stream: Arc::new(Mutex::new(Some(stream))),
            listener: Arc::new(Mutex::new(None)),
        })
    }

    /// 创建服务器监听
    pub async fn bind(addr: &str) -> Result<Self> {
        let listener = TcpListener::bind(addr).await?;
        Ok(Self {
            stream: Arc::new(Mutex::new(None)),
            listener: Arc::new(Mutex::new(Some(listener))),
        })
    }

    /// 接受客户端连接 (服务端模式)
    pub async fn accept(&self) -> Result<()> {
        let mut listener_guard = self.listener.lock().await;
        let listener = listener_guard
            .as_mut()
            .ok_or_else(|| anyhow!("Not in server mode"))?;

        let (stream, _) = listener.accept().await?;
        *self.stream.lock().await = Some(stream);
        Ok(())
    }
}

#[async_trait]
impl McpTransport for TcpTransport {
    async fn send(&self, message: &McpMessage) -> Result<()> {
        let mut stream_guard = self.stream.lock().await;
        let stream = stream_guard
            .as_mut()
            .ok_or_else(|| anyhow!("Not connected"))?;

        let json = serde_json::to_string(message)?;
        let frame = format!("Content-Length: {}\r\n\r\n{}", json.len(), json);
        stream.write_all(frame.as_bytes()).await?;
        stream.flush().await?;
        Ok(())
    }

    async fn receive(&self) -> Result<Option<McpMessage>> {
        let mut stream_guard = self.stream.lock().await;
        let stream = stream_guard
            .as_mut()
            .ok_or_else(|| anyhow!("Not connected"))?;

        // 读取 Content-Length 头
        let mut header_buf = vec![0u8; 1024];
        let mut total_read = 0;

        loop {
            let n = stream.read(&mut header_buf[total_read..]).await?;
            if n == 0 {
                return Ok(None); // 连接关闭
            }
            total_read += n;

            // 查找 \r\n\r\n 分隔符
            if let Some(pos) = find_header_end(&header_buf[..total_read]) {
                let header = String::from_utf8_lossy(&header_buf[..pos]);
                let content_length = parse_content_length(&header)?;

                // 读取消息体
                let header_size = pos + 4; // 包含 \r\n\r\n
                let body_size = content_length;
                let mut body_buf = vec![0u8; body_size];

                // 处理已经读取的部分
                let already_read = total_read - header_size;
                if already_read > 0 {
                    body_buf[..already_read].copy_from_slice(&header_buf[header_size..total_read]);
                }

                // 读取剩余部分
                if already_read < body_size {
                    stream.read_exact(&mut body_buf[already_read..]).await?;
                }

                let message: McpMessage = serde_json::from_slice(&body_buf)?;
                return Ok(Some(message));
            }

            if total_read >= header_buf.len() {
                return Err(anyhow!("Header too large"));
            }
        }
    }

    async fn close(&self) -> Result<()> {
        let mut stream_guard = self.stream.lock().await;
        stream_guard.take();
        Ok(())
    }
}

/// 查找 HTTP 风格的头部结束位置
fn find_header_end(buf: &[u8]) -> Option<usize> {
    for i in 0..buf.len().saturating_sub(3) {
        if &buf[i..i + 4] == b"\r\n\r\n" {
            return Some(i);
        }
    }
    None
}

/// 解析 Content-Length 头
fn parse_content_length(header: &str) -> Result<usize> {
    for line in header.lines() {
        if let Some(value) = line.strip_prefix("Content-Length:") {
            return Ok(value.trim().parse()?);
        }
    }
    Err(anyhow!("Content-Length header not found"))
}

/// Unix Socket 传输实现 (仅 Unix 系统)
#[cfg(unix)]
pub struct UnixSocketTransport {
    /// Unix socket 流
    stream: Arc<Mutex<Option<UnixStream>>>,
}

#[cfg(unix)]
impl UnixSocketTransport {
    /// 连接到 Unix socket
    pub async fn connect(path: &str) -> Result<Self> {
        let stream = UnixStream::connect(path).await?;
        Ok(Self {
            stream: Arc::new(Mutex::new(Some(stream))),
        })
    }
}

#[cfg(unix)]
#[async_trait]
impl McpTransport for UnixSocketTransport {
    async fn send(&self, message: &McpMessage) -> Result<()> {
        let mut stream_guard = self.stream.lock().await;
        let stream = stream_guard
            .as_mut()
            .ok_or_else(|| anyhow!("Not connected"))?;

        let json = serde_json::to_string(message)?;
        let frame = format!("Content-Length: {}\r\n\r\n{}", json.len(), json);
        stream.write_all(frame.as_bytes()).await?;
        stream.flush().await?;
        Ok(())
    }

    async fn receive(&self) -> Result<Option<McpMessage>> {
        let mut stream_guard = self.stream.lock().await;
        let stream = stream_guard
            .as_mut()
            .ok_or_else(|| anyhow!("Not connected"))?;

        // 读取 Content-Length 头
        let mut header_buf = vec![0u8; 1024];
        let mut total_read = 0;

        loop {
            let n = stream.read(&mut header_buf[total_read..]).await?;
            if n == 0 {
                return Ok(None); // 连接关闭
            }
            total_read += n;

            // 查找 \r\n\r\n 分隔符
            if let Some(pos) = find_header_end(&header_buf[..total_read]) {
                let header = String::from_utf8_lossy(&header_buf[..pos]);
                let content_length = parse_content_length(&header)?;

                // 读取消息体
                let header_size = pos + 4; // 包含 \r\n\r\n
                let body_size = content_length;
                let mut body_buf = vec![0u8; body_size];

                // 处理已经读取的部分
                let already_read = total_read - header_size;
                if already_read > 0 {
                    body_buf[..already_read].copy_from_slice(&header_buf[header_size..total_read]);
                }

                // 读取剩余部分
                if already_read < body_size {
                    stream.read_exact(&mut body_buf[already_read..]).await?;
                }

                let message: McpMessage = serde_json::from_slice(&body_buf)?;
                return Ok(Some(message));
            }

            if total_read >= header_buf.len() {
                return Err(anyhow!("Header too large"));
            }
        }
    }

    async fn close(&self) -> Result<()> {
        let mut stream_guard = self.stream.lock().await;
        stream_guard.take();
        Ok(())
    }
}

/// 内存传输 (用于测试)
pub struct MemoryTransport {
    messages: Arc<Mutex<Vec<McpMessage>>>,
    position: Arc<Mutex<usize>>,
}

impl Default for MemoryTransport {
    fn default() -> Self {
        Self::new()
    }
}

impl MemoryTransport {
    pub fn new() -> Self {
        Self {
            messages: Arc::new(Mutex::new(Vec::new())),
            position: Arc::new(Mutex::new(0)),
        }
    }

    pub async fn push(&self, message: McpMessage) {
        self.messages.lock().await.push(message);
    }
}

#[async_trait]
impl McpTransport for MemoryTransport {
    async fn send(&self, message: &McpMessage) -> Result<()> {
        self.messages.lock().await.push(message.clone());
        Ok(())
    }

    async fn receive(&self) -> Result<Option<McpMessage>> {
        let messages = self.messages.lock().await;
        let mut pos = self.position.lock().await;

        if *pos < messages.len() {
            let message = messages[*pos].clone();
            *pos += 1;
            Ok(Some(message))
        } else {
            Ok(None)
        }
    }

    async fn close(&self) -> Result<()> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::super::protocol::{McpRequest, RequestId};
    use super::*;

    #[test]
    fn test_parse_content_length() {
        let header = "Content-Length: 42\r\n";
        let len = parse_content_length(header).unwrap();
        assert_eq!(len, 42);
    }

    #[test]
    fn test_find_header_end() {
        let buf = b"Content-Length: 10\r\n\r\n";
        let pos = find_header_end(buf).unwrap();
        assert_eq!(pos, 18); // "\r\n\r\n" starts at position 18 (after "Content-Length: 10")
    }

    #[tokio::test]
    async fn test_memory_transport() {
        let transport = MemoryTransport::new();

        let msg = McpMessage::Request(McpRequest {
            id: RequestId::Number(1),
            method: "test".to_string(),
            params: None,
        });

        transport.send(&msg).await.unwrap();
        let received = transport.receive().await.unwrap();
        assert!(received.is_some());
    }
}
