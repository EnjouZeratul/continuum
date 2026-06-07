"""Audit Logger Tests - Comprehensive Coverage

Tests for continuum_sdk.security.audit_logger module.
"""

import os
import sys
import tempfile
import shutil
import json
import threading
from datetime import datetime, timedelta
from pathlib import Path
from unittest.mock import Mock, patch, mock_open

import pytest

from continuum_sdk.security.audit_logger import (
    AuditLogger,
    AuditOperation,
    AuditResult,
    AuditRecord,
)


class TestAuditOperation:
    """Test AuditOperation enum"""

    def test_operation_values(self):
        """Test all operation types have correct values"""
        assert AuditOperation.READ.value == "read"
        assert AuditOperation.WRITE.value == "write"
        assert AuditOperation.CREATE.value == "create"
        assert AuditOperation.DELETE.value == "delete"
        assert AuditOperation.MOVE.value == "move"
        assert AuditOperation.COPY.value == "copy"
        assert AuditOperation.RENAME.value == "rename"
        assert AuditOperation.MODIFY.value == "modify"
        assert AuditOperation.ACCESS.value == "access"
        assert AuditOperation.LIST.value == "list"
        assert AuditOperation.EXECUTE.value == "execute"


class TestAuditResult:
    """Test AuditResult enum"""

    def test_result_values(self):
        """Test all result types have correct values"""
        assert AuditResult.SUCCESS.value == "success"
        assert AuditResult.FAILURE.value == "failure"
        assert AuditResult.DENIED.value == "denied"
        assert AuditResult.ERROR.value == "error"


class TestAuditRecord:
    """Test AuditRecord dataclass"""

    def test_record_creation_basic(self):
        """Test basic record creation"""
        record = AuditRecord(
            id="test-001",
            timestamp=datetime.now(),
            operation=AuditOperation.READ,
            path="/test/file.py",
            result=AuditResult.SUCCESS,
        )
        assert record.id == "test-001"
        assert record.operation == AuditOperation.READ
        assert record.path == "/test/file.py"
        assert record.result == AuditResult.SUCCESS
        assert record.user is None
        assert record.process_id is None
        assert record.details is None
        assert record.metadata == {}

    def test_record_creation_full(self):
        """Test record creation with all fields"""
        now = datetime.now()
        record = AuditRecord(
            id="test-002",
            timestamp=now,
            operation=AuditOperation.WRITE,
            path="/test/file.py",
            result=AuditResult.FAILURE,
            user="testuser",
            process_id=12345,
            details="Write failed: permission denied",
            metadata={"key": "value", "count": 42},
        )
        assert record.id == "test-002"
        assert record.timestamp == now
        assert record.operation == AuditOperation.WRITE
        assert record.path == "/test/file.py"
        assert record.result == AuditResult.FAILURE
        assert record.user == "testuser"
        assert record.process_id == 12345
        assert record.details == "Write failed: permission denied"
        assert record.metadata == {"key": "value", "count": 42}

    def test_record_to_dict(self):
        """Test record serialization to dict"""
        now = datetime(2024, 1, 15, 10, 30, 45)
        record = AuditRecord(
            id="test-003",
            timestamp=now,
            operation=AuditOperation.READ,
            path="/test/file.py",
            result=AuditResult.SUCCESS,
            user="testuser",
            process_id=999,
            details="test details",
            metadata={"extra": "data"},
        )
        data = record.to_dict()

        assert data["id"] == "test-003"
        assert data["timestamp"] == "2024-01-15T10:30:45"
        assert data["operation"] == "read"
        assert data["path"] == "/test/file.py"
        assert data["result"] == "success"
        assert data["user"] == "testuser"
        assert data["process_id"] == 999
        assert data["details"] == "test details"
        assert data["metadata"] == {"extra": "data"}

    def test_record_from_dict(self):
        """Test record deserialization from dict"""
        data = {
            "id": "test-004",
            "timestamp": "2024-01-15T10:30:45",
            "operation": "write",
            "path": "/test/output.py",
            "result": "failure",
            "user": "writer",
            "process_id": 12345,
            "details": "disk full",
            "metadata": {"retry_count": 3},
        }
        record = AuditRecord.from_dict(data)

        assert record.id == "test-004"
        assert record.timestamp == datetime(2024, 1, 15, 10, 30, 45)
        assert record.operation == AuditOperation.WRITE
        assert record.path == "/test/output.py"
        assert record.result == AuditResult.FAILURE
        assert record.user == "writer"
        assert record.process_id == 12345
        assert record.details == "disk full"
        assert record.metadata == {"retry_count": 3}

    def test_record_from_dict_minimal(self):
        """Test record deserialization with minimal fields"""
        data = {
            "id": "test-005",
            "timestamp": "2024-01-15T10:30:45",
            "operation": "read",
            "path": "/test/file.py",
            "result": "success",
        }
        record = AuditRecord.from_dict(data)

        assert record.id == "test-005"
        assert record.user is None
        assert record.process_id is None
        assert record.details is None
        assert record.metadata == {}

    def test_record_roundtrip(self):
        """Test record serialization roundtrip"""
        original = AuditRecord(
            id="test-006",
            timestamp=datetime(2024, 2, 20, 15, 30, 0),
            operation=AuditOperation.DELETE,
            path="/test/delete.py",
            result=AuditResult.DENIED,
            user="admin",
            process_id=1,
            details="Access denied",
            metadata={"reason": "insufficient_permissions"},
        )
        data = original.to_dict()
        restored = AuditRecord.from_dict(data)

        assert restored.id == original.id
        assert restored.timestamp == original.timestamp
        assert restored.operation == original.operation
        assert restored.path == original.path
        assert restored.result == original.result
        assert restored.user == original.user
        assert restored.process_id == original.process_id
        assert restored.details == original.details
        assert restored.metadata == original.metadata


class TestAuditLoggerInit:
    """Test AuditLogger initialization"""

    @pytest.fixture
    def temp_dir(self):
        """Create temp directory"""
        dir_path = tempfile.mkdtemp()
        yield dir_path
        shutil.rmtree(dir_path)

    def test_init_default(self):
        """Test default initialization"""
        logger = AuditLogger()
        assert logger._log_file is None
        assert logger._max_records == 10000
        assert logger._auto_flush is True
        assert logger._flush_interval == 5.0
        assert len(logger._records) == 0

    def test_init_with_log_file(self, temp_dir):
        """Test initialization with log file"""
        log_file = os.path.join(temp_dir, "logs", "audit.jsonl")
        logger = AuditLogger(log_file=log_file)

        assert logger._log_file == Path(log_file)
        assert os.path.exists(os.path.dirname(log_file))

    def test_init_custom_params(self):
        """Test initialization with custom parameters"""
        logger = AuditLogger(
            max_records=5000,
            auto_flush=False,
            flush_interval=10.0,
        )
        assert logger._max_records == 5000
        assert logger._auto_flush is False
        assert logger._flush_interval == 10.0

    def test_init_creates_parent_directory(self, temp_dir):
        """Test that initialization creates parent directories"""
        log_file = os.path.join(temp_dir, "deeply", "nested", "dir", "audit.jsonl")
        AuditLogger(log_file=log_file)

        assert os.path.exists(os.path.dirname(log_file))


class TestAuditLoggerLog:
    """Test AuditLogger log functionality"""

    @pytest.fixture
    def temp_dir(self):
        """Create temp directory"""
        dir_path = tempfile.mkdtemp()
        yield dir_path
        shutil.rmtree(dir_path)

    def test_log_basic(self):
        """Test basic logging operation"""
        logger = AuditLogger()
        record = logger.log(
            operation=AuditOperation.READ,
            path="/test/file.py",
            result=AuditResult.SUCCESS,
        )

        assert record.operation == AuditOperation.READ
        assert record.path == "/test/file.py"
        assert record.result == AuditResult.SUCCESS
        assert record.id.startswith("audit-")
        assert len(logger) == 1

    def test_log_with_all_params(self):
        """Test logging with all parameters"""
        logger = AuditLogger()
        record = logger.log(
            operation=AuditOperation.WRITE,
            path="/test/output.py",
            result=AuditResult.FAILURE,
            user="testuser",
            details="Write failed",
            metadata={"error_code": "E001"},
        )

        assert record.operation == AuditOperation.WRITE
        assert record.path == "/test/output.py"
        assert record.result == AuditResult.FAILURE
        assert record.user == "testuser"
        assert record.details == "Write failed"
        assert record.metadata == {"error_code": "E001"}
        assert record.process_id == os.getpid()

    def test_log_with_path_object(self):
        """Test logging with Path object"""
        logger = AuditLogger()
        # Use a path that works cross-platform
        record = logger.log(
            operation=AuditOperation.READ,
            path=Path("test/file.py"),
            result=AuditResult.SUCCESS,
        )

        # str(Path) produces platform-native separators
        assert "test" in record.path
        assert "file.py" in record.path

    def test_log_auto_user(self):
        """Test automatic user detection"""
        logger = AuditLogger()
        record = logger.log(
            operation=AuditOperation.READ,
            path="/test/file.py",
            result=AuditResult.SUCCESS,
        )

        # Should have a user (either os.getlogin() or 'unknown')
        assert record.user is not None
        assert isinstance(record.user, str)

    def test_log_max_records_limit(self):
        """Test max records limit is enforced"""
        logger = AuditLogger(max_records=5)

        # Log 10 records
        for i in range(10):
            logger.log(
                operation=AuditOperation.READ,
                path=f"/test/file{i}.py",
                result=AuditResult.SUCCESS,
            )

        # Should only keep last 5
        assert len(logger) == 5
        records = logger.get_recent(10)
        # Check oldest is file5, newest is file9
        assert "file5" in records[-1].path
        assert "file9" in records[0].path

    def test_log_returns_record(self):
        """Test that log returns the created record"""
        logger = AuditLogger()
        record = logger.log(
            operation=AuditOperation.CREATE,
            path="/test/new.py",
            result=AuditResult.SUCCESS,
        )

        assert isinstance(record, AuditRecord)
        assert record.operation == AuditOperation.CREATE

    def test_log_thread_safety(self):
        """Test thread safety of logging"""
        logger = AuditLogger()
        num_threads = 10
        records_per_thread = 100

        def log_operations():
            for i in range(records_per_thread):
                logger.log(
                    operation=AuditOperation.READ,
                    path=f"/test/thread_file.py",
                    result=AuditResult.SUCCESS,
                )

        threads = [
            threading.Thread(target=log_operations)
            for _ in range(num_threads)
        ]

        for t in threads:
            t.start()
        for t in threads:
            t.join()

        assert len(logger) == num_threads * records_per_thread

    def test_generate_id_uniqueness(self):
        """Test ID generation is unique"""
        logger = AuditLogger()
        ids = set()

        for _ in range(100):
            record = logger.log(
                operation=AuditOperation.READ,
                path="/test/file.py",
                result=AuditResult.SUCCESS,
            )
            ids.add(record.id)

        assert len(ids) == 100


class TestAuditLoggerQuery:
    """Test AuditLogger query functionality"""

    def test_query_all(self):
        """Test querying all records"""
        logger = AuditLogger()
        for i in range(5):
            logger.log(AuditOperation.READ, f"/file{i}.py", AuditResult.SUCCESS)

        records = logger.query(limit=10)
        assert len(records) == 5

    def test_query_by_path_prefix(self):
        """Test querying by path prefix"""
        logger = AuditLogger()
        logger.log(AuditOperation.READ, "/project/src/main.py", AuditResult.SUCCESS)
        logger.log(AuditOperation.READ, "/project/tests/test.py", AuditResult.SUCCESS)
        logger.log(AuditOperation.READ, "/other/file.py", AuditResult.SUCCESS)

        records = logger.query(path="/project/")
        assert len(records) == 2

    def test_query_by_operation(self):
        """Test querying by operation type"""
        logger = AuditLogger()
        logger.log(AuditOperation.READ, "/file.py", AuditResult.SUCCESS)
        logger.log(AuditOperation.WRITE, "/file.py", AuditResult.SUCCESS)
        logger.log(AuditOperation.WRITE, "/file.py", AuditResult.FAILURE)
        logger.log(AuditOperation.DELETE, "/file.py", AuditResult.SUCCESS)

        records = logger.query(operation=AuditOperation.WRITE)
        assert len(records) == 2
        assert all(r.operation == AuditOperation.WRITE for r in records)

    def test_query_by_result(self):
        """Test querying by result"""
        logger = AuditLogger()
        logger.log(AuditOperation.READ, "/file.py", AuditResult.SUCCESS)
        logger.log(AuditOperation.READ, "/file.py", AuditResult.FAILURE)
        logger.log(AuditOperation.READ, "/file.py", AuditResult.DENIED)

        records = logger.query(result=AuditResult.DENIED)
        assert len(records) == 1
        assert records[0].result == AuditResult.DENIED

    def test_query_by_user(self):
        """Test querying by user"""
        logger = AuditLogger()
        logger.log(AuditOperation.READ, "/file.py", AuditResult.SUCCESS, user="alice")
        logger.log(AuditOperation.READ, "/file.py", AuditResult.SUCCESS, user="bob")
        logger.log(AuditOperation.READ, "/file.py", AuditResult.SUCCESS, user="alice")

        records = logger.query(user="alice")
        assert len(records) == 2
        assert all(r.user == "alice" for r in records)

    def test_query_by_time_range(self):
        """Test querying by time range"""
        logger = AuditLogger()
        now = datetime.now()

        # Create records with different timestamps
        # We'll use the query method's time filters
        logger.log(AuditOperation.READ, "/old.py", AuditResult.SUCCESS)
        logger.log(AuditOperation.READ, "/new.py", AuditResult.SUCCESS)

        start_time = now - timedelta(hours=1)
        end_time = now + timedelta(hours=1)

        records = logger.query(start_time=start_time, end_time=end_time)
        # Should include recent records
        assert len(records) >= 2

    def test_query_time_range_excludes_records(self):
        """Test that time range properly excludes records outside range"""
        logger = AuditLogger()

        # Clear any previous records
        logger.clear()

        # Log records at specific "times" by manipulating timestamps
        # We'll log 3 records and filter by time range
        logger.log(AuditOperation.READ, "/file1.py", AuditResult.SUCCESS)
        logger.log(AuditOperation.READ, "/file2.py", AuditResult.SUCCESS)
        logger.log(AuditOperation.READ, "/file3.py", AuditResult.SUCCESS)

        # Use very narrow time range to potentially exclude some records
        now = datetime.now()
        # Start time in future to trigger the start_time filter
        future_start = now + timedelta(hours=10)
        records = logger.query(start_time=future_start)
        # All records should be excluded since they're before the start time
        assert len(records) == 0

        # End time in past to trigger the end_time filter
        past_end = now - timedelta(hours=10)
        records = logger.query(end_time=past_end)
        # All records should be excluded since they're after the end time
        assert len(records) == 0

    def test_query_start_time_filter(self):
        """Test query with only start_time"""
        logger = AuditLogger()

        # Log some records
        logger.log(AuditOperation.READ, "/file1.py", AuditResult.SUCCESS)
        logger.log(AuditOperation.READ, "/file2.py", AuditResult.SUCCESS)

        # Query from 1 hour ago
        start_time = datetime.now() - timedelta(hours=1)
        records = logger.query(start_time=start_time)
        assert len(records) == 2

    def test_query_end_time_filter(self):
        """Test query with only end_time"""
        logger = AuditLogger()

        # Log some records
        logger.log(AuditOperation.READ, "/file1.py", AuditResult.SUCCESS)
        logger.log(AuditOperation.READ, "/file2.py", AuditResult.SUCCESS)

        # Query until 1 hour in future
        end_time = datetime.now() + timedelta(hours=1)
        records = logger.query(end_time=end_time)
        assert len(records) == 2

    def test_query_limit(self):
        """Test query limit"""
        logger = AuditLogger()
        for i in range(20):
            logger.log(AuditOperation.READ, f"/file{i}.py", AuditResult.SUCCESS)

        records = logger.query(limit=5)
        assert len(records) == 5

    def test_query_combined_filters(self):
        """Test combined query filters"""
        logger = AuditLogger()
        logger.log(AuditOperation.READ, "/src/main.py", AuditResult.SUCCESS, user="alice")
        logger.log(AuditOperation.WRITE, "/src/main.py", AuditResult.SUCCESS, user="alice")
        logger.log(AuditOperation.READ, "/src/test.py", AuditResult.FAILURE, user="bob")
        logger.log(AuditOperation.READ, "/lib/util.py", AuditResult.SUCCESS, user="alice")

        records = logger.query(
            path="/src/",
            operation=AuditOperation.READ,
            result=AuditResult.SUCCESS,
            user="alice",
        )
        assert len(records) == 1
        assert records[0].path == "/src/main.py"

    def test_query_returns_newest_first(self):
        """Test that query returns records newest first"""
        logger = AuditLogger()
        logger.log(AuditOperation.READ, "/file1.py", AuditResult.SUCCESS)
        logger.log(AuditOperation.READ, "/file2.py", AuditResult.SUCCESS)
        logger.log(AuditOperation.READ, "/file3.py", AuditResult.SUCCESS)

        records = logger.query(limit=10)
        assert records[0].path == "/file3.py"
        assert records[-1].path == "/file1.py"


class TestAuditLoggerConvenienceMethods:
    """Test AuditLogger convenience methods"""

    def test_get_by_path(self):
        """Test get_by_path method"""
        logger = AuditLogger()
        logger.log(AuditOperation.READ, "/project/src/main.py", AuditResult.SUCCESS)
        logger.log(AuditOperation.READ, "/project/tests/test.py", AuditResult.SUCCESS)
        logger.log(AuditOperation.READ, "/other/file.py", AuditResult.SUCCESS)

        records = logger.get_by_path("/project/")
        assert len(records) == 2

    def test_get_by_time_range(self):
        """Test get_by_time_range method"""
        logger = AuditLogger()
        logger.log(AuditOperation.READ, "/file1.py", AuditResult.SUCCESS)
        logger.log(AuditOperation.READ, "/file2.py", AuditResult.SUCCESS)

        start = datetime.now() - timedelta(hours=1)
        end = datetime.now() + timedelta(hours=1)

        records = logger.get_by_time_range(start, end)
        assert len(records) >= 2

    def test_get_recent(self):
        """Test get_recent method"""
        logger = AuditLogger()
        for i in range(20):
            logger.log(AuditOperation.READ, f"/file{i}.py", AuditResult.SUCCESS)

        records = logger.get_recent(5)
        assert len(records) == 5
        # Should be newest first
        assert "file19" in records[0].path

    def test_get_recent_default_count(self):
        """Test get_recent with default count"""
        logger = AuditLogger()
        for i in range(20):
            logger.log(AuditOperation.READ, f"/file{i}.py", AuditResult.SUCCESS)

        records = logger.get_recent()  # Default count=10
        assert len(records) == 10


class TestAuditLoggerStatistics:
    """Test AuditLogger statistics"""

    def test_get_statistics_empty(self):
        """Test statistics with no records"""
        logger = AuditLogger()
        stats = logger.get_statistics()

        assert stats["total"] == 0
        assert stats["operations"] == {}
        assert stats["results"] == {}

    def test_get_statistics_with_records(self):
        """Test statistics with records"""
        logger = AuditLogger()
        logger.log(AuditOperation.READ, "/file1.py", AuditResult.SUCCESS)
        logger.log(AuditOperation.READ, "/file2.py", AuditResult.SUCCESS)
        logger.log(AuditOperation.WRITE, "/file3.py", AuditResult.FAILURE)
        logger.log(AuditOperation.DELETE, "/file4.py", AuditResult.DENIED)

        stats = logger.get_statistics()

        assert stats["total"] == 4
        assert stats["operations"]["read"] == 2
        assert stats["operations"]["write"] == 1
        assert stats["operations"]["delete"] == 1
        assert stats["results"]["success"] == 2
        assert stats["results"]["failure"] == 1
        assert stats["results"]["denied"] == 1
        assert stats["unique_paths"] == 4
        assert stats["first_record"] is not None
        assert stats["last_record"] is not None

    def test_get_statistics_same_path(self):
        """Test statistics with same path"""
        logger = AuditLogger()
        logger.log(AuditOperation.READ, "/file.py", AuditResult.SUCCESS)
        logger.log(AuditOperation.WRITE, "/file.py", AuditResult.SUCCESS)

        stats = logger.get_statistics()

        assert stats["unique_paths"] == 1


class TestAuditLoggerExport:
    """Test AuditLogger export functionality"""

    @pytest.fixture
    def temp_dir(self):
        """Create temp directory"""
        dir_path = tempfile.mkdtemp()
        yield dir_path
        shutil.rmtree(dir_path)

    def test_export_json(self, temp_dir):
        """Test JSON export"""
        logger = AuditLogger()
        logger.log(AuditOperation.READ, "/file.py", AuditResult.SUCCESS, user="test")
        logger.log(AuditOperation.WRITE, "/file.py", AuditResult.FAILURE)

        export_path = os.path.join(temp_dir, "audit.json")
        count = logger.export_json(export_path)

        assert count == 2
        assert os.path.exists(export_path)

        with open(export_path, encoding="utf-8") as f:
            data = json.load(f)
        assert len(data) == 2
        assert data[0]["operation"] == "read"
        assert data[1]["operation"] == "write"

    def test_export_json_with_path_object(self, temp_dir):
        """Test JSON export with Path object"""
        logger = AuditLogger()
        logger.log(AuditOperation.READ, "/file.py", AuditResult.SUCCESS)

        export_path = Path(temp_dir) / "audit.json"
        count = logger.export_json(export_path)

        assert count == 1
        assert export_path.exists()

    def test_export_csv(self, temp_dir):
        """Test CSV export"""
        logger = AuditLogger()
        logger.log(AuditOperation.READ, "/file.py", AuditResult.SUCCESS, user="test")
        logger.log(AuditOperation.WRITE, "/file.py", AuditResult.FAILURE, details="error")

        export_path = os.path.join(temp_dir, "audit.csv")
        count = logger.export_csv(export_path)

        assert count == 2
        assert os.path.exists(export_path)

        with open(export_path, encoding="utf-8", newline="") as f:
            import csv
            reader = csv.reader(f)
            rows = list(reader)

        assert len(rows) == 3  # header + 2 records
        assert rows[0] == ["id", "timestamp", "operation", "path", "result", "user", "process_id", "details"]
        assert rows[1][2] == "read"
        assert rows[2][2] == "write"

    def test_export_csv_with_path_object(self, temp_dir):
        """Test CSV export with Path object"""
        logger = AuditLogger()
        logger.log(AuditOperation.READ, "/file.py", AuditResult.SUCCESS)

        export_path = Path(temp_dir) / "audit.csv"
        count = logger.export_csv(export_path)

        assert count == 1
        assert export_path.exists()

    def test_export_json_empty(self, temp_dir):
        """Test JSON export with no records"""
        logger = AuditLogger()

        export_path = os.path.join(temp_dir, "audit.json")
        count = logger.export_json(export_path)

        assert count == 0

        with open(export_path, encoding="utf-8") as f:
            data = json.load(f)
        assert data == []

    def test_export_csv_empty(self, temp_dir):
        """Test CSV export with no records"""
        logger = AuditLogger()

        export_path = os.path.join(temp_dir, "audit.csv")
        count = logger.export_csv(export_path)

        assert count == 0

        with open(export_path, encoding="utf-8", newline="") as f:
            import csv
            reader = csv.reader(f)
            rows = list(reader)

        # Only header
        assert len(rows) == 1


class TestAuditLoggerClear:
    """Test AuditLogger clear functionality"""

    def test_clear_records(self):
        """Test clearing records"""
        logger = AuditLogger()
        logger.log(AuditOperation.READ, "/file.py", AuditResult.SUCCESS)
        logger.log(AuditOperation.WRITE, "/file.py", AuditResult.SUCCESS)

        count = logger.clear()
        assert count == 2
        assert len(logger) == 0

    def test_clear_empty_logger(self):
        """Test clearing empty logger"""
        logger = AuditLogger()
        count = logger.clear()
        assert count == 0

    def test_clear_resets_counter(self):
        """Test that clear resets counter"""
        logger = AuditLogger()
        logger.log(AuditOperation.READ, "/file.py", AuditResult.SUCCESS)
        logger.clear()

        # Next record should start fresh (counter reset)
        # Note: ID includes timestamp, so we can't easily verify counter directly
        # But we can verify the logger is empty
        assert len(logger) == 0


class TestAuditLoggerFilePersistence:
    """Test AuditLogger file persistence"""

    @pytest.fixture
    def temp_dir(self):
        """Create temp directory"""
        dir_path = tempfile.mkdtemp()
        yield dir_path
        shutil.rmtree(dir_path)

    def test_flush_to_file(self, temp_dir):
        """Test manual flush to file"""
        log_file = os.path.join(temp_dir, "audit.jsonl")
        logger = AuditLogger(log_file=log_file, auto_flush=False)

        logger.log(AuditOperation.READ, "/file.py", AuditResult.SUCCESS)
        logger.log(AuditOperation.WRITE, "/file.py", AuditResult.FAILURE)

        logger.flush()

        assert os.path.exists(log_file)

        with open(log_file, encoding="utf-8") as f:
            lines = [l.strip() for l in f if l.strip()]
        assert len(lines) == 2

    def test_flush_without_log_file(self):
        """Test flush when no log file set"""
        logger = AuditLogger()
        # Should not raise
        logger.flush()

    def test_auto_flush_on_interval(self, temp_dir):
        """Test auto-flush when interval is reached"""
        log_file = os.path.join(temp_dir, "audit.jsonl")
        logger = AuditLogger(
            log_file=log_file,
            auto_flush=True,
            flush_interval=0.001,  # Very short interval
        )

        # Log multiple records
        logger.log(AuditOperation.READ, "/file.py", AuditResult.SUCCESS)

        # Small delay to trigger flush interval
        import time
        time.sleep(0.01)

        logger.log(AuditOperation.WRITE, "/file.py", AuditResult.SUCCESS)

        # File should exist after auto-flush
        assert os.path.exists(log_file)

    def test_append_to_file_directly(self, temp_dir):
        """Test _append_to_file method directly"""
        log_file = os.path.join(temp_dir, "audit.jsonl")
        logger = AuditLogger(log_file=log_file)

        record = AuditRecord(
            id="test-direct",
            timestamp=datetime.now(),
            operation=AuditOperation.READ,
            path="/direct.py",
            result=AuditResult.SUCCESS,
        )

        logger._append_to_file(record)

        assert os.path.exists(log_file)
        with open(log_file, encoding="utf-8") as f:
            content = f.read()
        assert "test-direct" in content

    def test_append_to_file_without_log_file(self):
        """Test _append_to_file when no log file set"""
        logger = AuditLogger()
        record = AuditRecord(
            id="test-no-file",
            timestamp=datetime.now(),
            operation=AuditOperation.READ,
            path="/no_file.py",
            result=AuditResult.SUCCESS,
        )
        # Should not raise
        logger._append_to_file(record)

    def test_load_existing_records(self, temp_dir):
        """Test loading existing records from file"""
        log_file = os.path.join(temp_dir, "audit.jsonl")

        # Create initial logger and log some records
        logger1 = AuditLogger(log_file=log_file, auto_flush=False)
        logger1.log(AuditOperation.READ, "/file1.py", AuditResult.SUCCESS)
        logger1.log(AuditOperation.WRITE, "/file2.py", AuditResult.FAILURE)
        logger1.flush()

        # Create new logger with same file
        logger2 = AuditLogger(log_file=log_file)

        # Should load existing records
        assert len(logger2) == 2

    def test_load_existing_records_corrupted_line(self, temp_dir):
        """Test loading when file has corrupted lines"""
        log_file = os.path.join(temp_dir, "audit.jsonl")

        # Write some valid and invalid lines
        with open(log_file, "w", encoding="utf-8") as f:
            valid_record = AuditRecord(
                id="test-001",
                timestamp=datetime.now(),
                operation=AuditOperation.READ,
                path="/file.py",
                result=AuditResult.SUCCESS,
            )
            f.write(json.dumps(valid_record.to_dict()) + "\n")
            f.write("invalid json line\n")
            f.write('{"incomplete": "record"\n')
            f.write("")

        # Should load without error
        logger = AuditLogger(log_file=log_file)
        assert len(logger) == 1

    def test_load_existing_records_missing_fields(self, temp_dir):
        """Test loading when records have missing required fields"""
        log_file = os.path.join(temp_dir, "audit.jsonl")

        # Write valid record and record with missing fields
        with open(log_file, "w", encoding="utf-8") as f:
            valid_record = AuditRecord(
                id="test-001",
                timestamp=datetime.now(),
                operation=AuditOperation.READ,
                path="/file.py",
                result=AuditResult.SUCCESS,
            )
            f.write(json.dumps(valid_record.to_dict()) + "\n")
            # Missing required fields - should be skipped
            f.write('{"id": "test-002"}\n')

        logger = AuditLogger(log_file=log_file)
        # Should only load valid record
        assert len(logger) == 1

    def test_load_existing_records_file_not_found(self, temp_dir):
        """Test loading when file doesn't exist"""
        log_file = os.path.join(temp_dir, "nonexistent.jsonl")
        # Should not raise
        logger = AuditLogger(log_file=log_file)
        assert len(logger) == 0

    def test_load_existing_records_io_error(self, temp_dir):
        """Test loading when file has IO error"""
        log_file = os.path.join(temp_dir, "audit.jsonl")

        # Create the file first
        Path(log_file).write_text("")

        # Mock open to raise IOError during loading
        with patch('builtins.open', side_effect=IOError("Mocked IO error")):
            # Should not raise, just log warning
            logger = AuditLogger(log_file=log_file)
            assert len(logger) == 0

    def test_load_existing_records_os_error(self, temp_dir):
        """Test loading when file has OS error"""
        log_file = os.path.join(temp_dir, "audit.jsonl")

        # Create the file first
        Path(log_file).write_text("")

        # Mock open to raise OSError during loading
        with patch('builtins.open', side_effect=OSError("Mocked OS error")):
            # Should not raise, just log warning
            logger = AuditLogger(log_file=log_file)
            assert len(logger) == 0


class TestAuditLoggerErrorHandling:
    """Test AuditLogger error handling"""

    @pytest.fixture
    def temp_dir(self):
        """Create temp directory"""
        dir_path = tempfile.mkdtemp()
        yield dir_path
        shutil.rmtree(dir_path)

    def test_get_current_user_fallback(self):
        """Test user detection fallback on error"""
        logger = AuditLogger()

        # Mock os.getlogin to raise error
        with patch('os.getlogin', side_effect=OSError("Mocked error")):
            record = logger.log(
                operation=AuditOperation.READ,
                path="/file.py",
                result=AuditResult.SUCCESS,
            )
            assert record.user == "unknown"

    def test_get_current_user_permission_error(self):
        """Test user detection with permission error"""
        logger = AuditLogger()

        with patch('os.getlogin', side_effect=PermissionError("Access denied")):
            record = logger.log(
                operation=AuditOperation.READ,
                path="/file.py",
                result=AuditResult.SUCCESS,
            )
            assert record.user == "unknown"


class TestAuditLoggerIteration:
    """Test AuditLogger iteration"""

    def test_iterate_records(self):
        """Test iterating over records"""
        logger = AuditLogger()
        logger.log(AuditOperation.READ, "/file1.py", AuditResult.SUCCESS)
        logger.log(AuditOperation.READ, "/file2.py", AuditResult.SUCCESS)
        logger.log(AuditOperation.READ, "/file3.py", AuditResult.SUCCESS)

        records = list(logger)
        assert len(records) == 3

    def test_iterate_empty_logger(self):
        """Test iterating empty logger"""
        logger = AuditLogger()
        records = list(logger)
        assert len(records) == 0

    def test_iter_returns_copy(self):
        """Test that iteration returns copy of records"""
        logger = AuditLogger()
        logger.log(AuditOperation.READ, "/file.py", AuditResult.SUCCESS)

        records = list(logger)
        # Modifying returned list shouldn't affect internal records
        records.clear()

        assert len(logger) == 1


class TestAuditLoggerRepr:
    """Test AuditLogger representation"""

    def test_repr_without_file(self):
        """Test repr without log file"""
        logger = AuditLogger()
        repr_str = repr(logger)
        assert "AuditLogger" in repr_str
        assert "records=0" in repr_str
        assert "file=None" in repr_str

    def test_repr_with_file(self, tmp_path):
        """Test repr with log file"""
        log_file = tmp_path / "audit.jsonl"
        logger = AuditLogger(log_file=log_file)
        logger.log(AuditOperation.READ, "/file.py", AuditResult.SUCCESS)

        repr_str = repr(logger)
        assert "AuditLogger" in repr_str
        assert "records=1" in repr_str
        assert "audit.jsonl" in repr_str


class TestAuditLoggerLen:
    """Test AuditLogger len"""

    def test_len_empty(self):
        """Test len of empty logger"""
        logger = AuditLogger()
        assert len(logger) == 0

    def test_len_with_records(self):
        """Test len with records"""
        logger = AuditLogger()
        logger.log(AuditOperation.READ, "/file.py", AuditResult.SUCCESS)
        assert len(logger) == 1

        logger.log(AuditOperation.WRITE, "/file.py", AuditResult.SUCCESS)
        assert len(logger) == 2


class TestRemainingBranchCoverage:
    """Tests for remaining branch coverage in audit_logger.py."""

    @pytest.fixture
    def temp_dir(self):
        """Create temp directory."""
        dir_path = tempfile.mkdtemp()
        yield dir_path
        shutil.rmtree(dir_path)

    def test_load_existing_records_with_empty_lines(self, temp_dir):
        """Test loading records when file has empty lines (line 481->479)."""
        log_file = os.path.join(temp_dir, "audit.jsonl")

        # Write valid record followed by empty lines
        valid_record = AuditRecord(
            id="test-001",
            timestamp=datetime.now(),
            operation=AuditOperation.READ,
            path="/file.py",
            result=AuditResult.SUCCESS,
        )
        with open(log_file, "w", encoding="utf-8") as f:
            f.write(json.dumps(valid_record.to_dict()) + "\n")
            f.write("\n")  # Empty line
            f.write("   \n")  # Whitespace-only line
            f.write("\n")  # Another empty line

        # Should load only the valid record, skipping empty lines
        logger = AuditLogger(log_file=log_file)
        assert len(logger) == 1

    def test_load_existing_records_with_whitespace_only_lines(self, temp_dir):
        """Test loading records when file has whitespace-only lines."""
        log_file = os.path.join(temp_dir, "audit_whitespace.jsonl")

        valid_record = AuditRecord(
            id="test-002",
            timestamp=datetime.now(),
            operation=AuditOperation.WRITE,
            path="/file.py",
            result=AuditResult.SUCCESS,
        )
        with open(log_file, "w", encoding="utf-8") as f:
            f.write("   \n")  # Whitespace-only line at start
            f.write(json.dumps(valid_record.to_dict()) + "\n")
            f.write("\t\t\n")  # Tab-only line

        logger = AuditLogger(log_file=log_file)
        assert len(logger) == 1


if __name__ == "__main__":
    pytest.main([__file__, "-v"])
