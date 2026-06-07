"""
安全文件操作 - TOCTOU保护

使用原子操作和文件描述符操作避免竞态条件。
"""

from __future__ import annotations

import logging
import os
import time
from contextlib import contextmanager
from pathlib import Path
from typing import Any, Callable, Generator

from ..errors import SecurityError

logger = logging.getLogger(__name__)


@contextmanager
def safe_open_read(
    path: Path,
    validate_func: Callable[[Path], Any]
) -> Generator[bytes, None, None]:
    """
    安全读取文件，带TOCTOU保护

    通过文件描述符打开文件，验证路径，然后读取。
    防止符号链接替换攻击。

    Args:
        path: 文件路径
        validate_func: 路径验证函数，返回验证结果对象

    Yields:
        文件内容（bytes）

    Raises:
        SecurityError: 路径验证失败
        FileNotFoundError: 文件不存在
    """
    # 1. 打开文件获取文件描述符
    try:
        fd = os.open(str(path), os.O_RDONLY)
    except FileNotFoundError:
        raise FileNotFoundError(f"File not found: {path}")
    except PermissionError:
        raise SecurityError(f"Permission denied: {path}")

    try:
        # 2. Linux: 通过 /proc/self/fd/{fd} 获取真实路径
        # Windows: 直接使用原路径（Windows符号链接行为不同）
        if os.name != 'nt':
            try:
                real_path = Path(os.readlink(f"/proc/self/fd/{fd}"))
            except (OSError, FileNotFoundError):
                # 如果无法读取 fd 路径，使用原路径
                real_path = path
        else:
            real_path = path

        # 3. 验证真实路径
        result = validate_func(real_path)
        if hasattr(result, 'is_valid') and not result.is_valid:
            raise SecurityError(
                f"Path validation failed: {getattr(result, 'reason', 'unknown')}"
            )

        # 4. 通过 fd 读取内容
        with os.fdopen(fd, 'rb') as f:
            content = f.read()

        yield content

    finally:
        # fd 已被 fdopen 关闭，无需手动关闭
        pass


def safe_write_atomic(
    path: Path,
    content: bytes,
    validate_func: Callable[[Path], Any],
    create_dirs: bool = True
) -> None:
    """
    原子写入文件

    先写入临时文件，然后原子重命名。
    确保写入操作的原子性。

    Args:
        path: 目标文件路径
        content: 文件内容
        validate_func: 路径验证函数
        create_dirs: 是否创建父目录

    Raises:
        SecurityError: 路径验证失败
    """
    # 1. 验证目标路径
    result = validate_func(path)
    if hasattr(result, 'is_valid') and not result.is_valid:
        raise SecurityError(
            f"Path validation failed: {getattr(result, 'reason', 'unknown')}"
        )

    # 2. 创建父目录
    if create_dirs and not path.parent.exists():
        path.parent.mkdir(parents=True, exist_ok=True)

    # 3. 写入临时文件
    temp_path = path.with_suffix(path.suffix + '.tmp')
    try:
        with open(temp_path, 'wb') as f:
            f.write(content)
            f.flush()
            os.fsync(f.fileno())  # 确保写入磁盘

        # 4. 原子重命名（POSIX 保证原子性）
        if os.name == 'nt':
            # Windows: 使用 replace（Python 3.3+ 原子操作）
            os.replace(temp_path, path)
        else:
            os.rename(temp_path, path)

        logger.debug(f"Atomically wrote {len(content)} bytes to {path}")

    finally:
        # 清理临时文件
        temp_path.unlink(missing_ok=True)


def safe_read_with_retry(
    path: Path,
    validate_func: Callable[[Path], Any],
    max_retries: int = 3,
    retry_delay: float = 0.1
) -> bytes:
    """
    带重试的安全读取

    对于可能存在竞态的场景，提供重试机制。

    Args:
        path: 文件路径
        validate_func: 路径验证函数
        max_retries: 最大重试次数
        retry_delay: 重试延迟（秒）

    Returns:
        文件内容
    """
    last_error = None
    for attempt in range(max_retries):
        try:
            with safe_open_read(path, validate_func) as content:
                return content
        except (FileNotFoundError, SecurityError) as e:
            last_error = e
            if attempt < max_retries - 1:
                time.sleep(retry_delay)

    raise last_error  # type: ignore


__all__ = [
    "safe_open_read",
    "safe_write_atomic",
    "safe_read_with_retry",
]
