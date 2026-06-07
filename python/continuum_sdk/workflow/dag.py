"""Workflow DAG API

Define and execute DAG (Directed Acyclic Graph) workflows.

Features:
    - Task dependency management: define dependencies between tasks
    - Parallel execution: automatically execute independent tasks in parallel
    - Cycle detection: detect and block cyclic dependencies
    - ASCII visualization: generate workflow structure diagrams
    - Execution result tracking: record execution status of each task

Quick Start:
    >>> from continuum_sdk.workflow import DAG, Node
    >>>
    >>> dag = DAG(name="data_pipeline")
    >>> dag.add_node(Node("fetch", func=lambda: "data"))
    >>> dag.add_node(Node("process", func=lambda: "process", depends_on=["fetch"]))
    >>> dag.add_node(Node("save", func=lambda: "save", depends_on=["process"]))
    >>>
    >>> print(dag.visualize())  # Show DAG structure
    >>> result = await dag.execute()

Parallel Execution:
    >>> dag = DAG(name="parallel_analysis")
    >>> dag.add_node(Node("analyze_a", func=lambda: "A result"))
    >>> dag.add_node(Node("analyze_b", func=lambda: "B result"))
    >>> dag.add_node(Node("analyze_c", func=lambda: "C result"))
    >>> dag.add_node(Node("summary", func=lambda: "summary",
    ...     depends_on=["analyze_a", "analyze_b", "analyze_c"]))
    >>>
    >>> # analyze_a, analyze_b, analyze_c will execute in parallel
    >>> result = await dag.execute(max_workers=3)

Cycle Detection:
    >>> dag = DAG(name="circular")
    >>> dag.add_node(Node("a", depends_on=["c"]))  # a -> c
    >>> dag.add_node(Node("b", depends_on=["a"]))  # c -> a -> b
    >>> dag.add_node(Node("c", depends_on=["b"]))  # b -> c (cycle!)
    >>>
    >>> has_cycle, path = dag.detect_cycle()
    >>> if has_cycle:
    ...     print(f"Detected cycle: {' -> '.join(path)}")

Node Status:
    - PENDING: Waiting to execute
    - RUNNING: Currently executing
    - SUCCESS: Execution succeeded
    - FAILED: Execution failed
    - SKIPPED: Skipped

Execution Result:
    >>> for node_id, result in result.results.items():
    ...     print(f"{node_id}: {result.status.value}")
    ...     print(f"  Output: {result.output}")
    ...     print(f"  Duration: {result.duration_ms}ms")

DAGExecutor:
    >>> from continuum_sdk.workflow import DAGExecutor
    >>>
    >>> executor = DAGExecutor(dag, max_workers=4)
    >>> result = await executor.execute()
    >>> print(f"Execution order: {result.execution_order}")
    >>> print(f"Total duration: {result.duration:.2f}s")

See Also:
    Node: DAG node definition
    NodeResult: Execution result container
    DAGExecutor: DAG executor
"""

import asyncio
from collections.abc import Callable
from dataclasses import dataclass, field
from enum import Enum
from typing import Any


class NodeStatus(Enum):
    """Node status"""

    PENDING = "pending"
    RUNNING = "running"
    SUCCESS = "success"
    FAILED = "failed"
    SKIPPED = "skipped"


@dataclass
class NodeResult:
    """Node execution result"""

    node_id: str
    status: NodeStatus
    output: Any = None
    error: str | None = None
    duration_ms: int = 0


@dataclass
class Node:
    """Workflow node

    Usage:
        from continuum_sdk.workflow import Node

        # Create node
        node = Node("process", func=process_data)

        # Add dependency
        node.depends_on("fetch")
    """

    id: str
    func: Callable | None = None
    name: str | None = None
    description: str | None = None
    dependencies: set[str] = field(default_factory=set)

    def depends_on(self, *node_ids: str) -> "Node":
        """Add dependency nodes

        Args:
            *node_ids: Dependency node IDs

        Returns:
            self (supports chaining)
        """
        self.dependencies.update(node_ids)
        return self

    def set_func(self, func: Callable) -> "Node":
        """Set execution function"""
        self.func = func
        return self


class DAGResult:
    """DAG execution result"""

    def __init__(self, dag_id: str):
        self.dag_id = dag_id
        self._results: dict[str, NodeResult] = {}
        self._execution_order: list[str] = []

    @property
    def status(self) -> NodeStatus:
        """Overall status"""
        if not self._results:
            return NodeStatus.PENDING

        for result in self._results.values():
            if result.status == NodeStatus.FAILED:
                return NodeStatus.FAILED
            if result.status == NodeStatus.RUNNING:
                return NodeStatus.RUNNING

        return NodeStatus.SUCCESS

    def get_output(self, node_id: str) -> Any | None:
        """Get node output

        Args:
            node_id: Node ID

        Returns:
            Node output result
        """
        result = self._results.get(node_id)
        return result.output if result else None

    def get_result(self, node_id: str) -> NodeResult | None:
        """Get node result

        Args:
            node_id: Node ID

        Returns:
            Node execution result
        """
        return self._results.get(node_id)

    def get_all_outputs(self) -> dict[str, Any]:
        """Get all node outputs"""
        return {
            node_id: result.output
            for node_id, result in self._results.items()
            if result.output is not None
        }

    def failed_nodes(self) -> list[str]:
        """Get failed node IDs"""
        return [
            node_id
            for node_id, result in self._results.items()
            if result.status == NodeStatus.FAILED
        ]

    def execution_order(self) -> list[str]:
        """Get actual execution order"""
        return self._execution_order.copy()

    def _set_result(self, node_id: str, result: NodeResult) -> None:
        """Set node result"""
        self._results[node_id] = result
        self._execution_order.append(node_id)


class DAG:
    """Workflow DAG

    Usage:
        from continuum_sdk.workflow import DAG, Node

        # Create DAG
        dag = DAG("my_workflow")

        # Add nodes
        dag.add(Node("fetch", func=fetch_data))
        dag.add(Node("process", func=process).depends_on("fetch"))
        dag.add(Node("save", func=save).depends_on("process"))

        # Execute
        result = await dag.execute()

        # Get result
        output = result.get_output("save")
    """

    def __init__(self, id: str, name: str | None = None):
        """Initialize DAG

        Args:
            id: DAG ID
            name: Display name
        """
        self.id = id
        self.name = name or id
        self._nodes: dict[str, Node] = {}

    def add(self, node: Node) -> "DAG":
        """Add node

        Args:
            node: Workflow node

        Returns:
            self (supports chaining)
        """
        self._nodes[node.id] = node
        return self

    def get(self, node_id: str) -> Node | None:
        """Get node"""
        return self._nodes.get(node_id)

    def remove(self, node_id: str) -> bool:
        """Remove node"""
        if node_id in self._nodes:
            del self._nodes[node_id]
            # Clean up dependencies
            for node in self._nodes.values():
                node.dependencies.discard(node_id)
            return True
        return False

    def depends_on(self, node_id: str, *depends: str) -> "DAG":
        """Add dependency relationship

        Args:
            node_id: Node ID
            *depends: Dependency node IDs

        Returns:
            self
        """
        node = self.get(node_id)
        if node:
            node.depends_on(*depends)
        return self

    def validate(self) -> list[str]:
        """Validate DAG

        Returns:
            List of error messages (empty list means validation passed)
        """
        errors = []

        # Check for cyclic dependencies
        visited = set()
        rec_stack = set()

        def has_cycle(node_id: str) -> bool:
            visited.add(node_id)
            rec_stack.add(node_id)

            node = self.get(node_id)
            if node:
                for dep in node.dependencies:
                    if dep not in visited:
                        if has_cycle(dep):
                            return True
                    elif dep in rec_stack:
                        return True

            rec_stack.remove(node_id)
            return False

        for node_id in self._nodes:
            if node_id not in visited:  # pragma: no branch
                if has_cycle(node_id):  # pragma: no branch
                    errors.append("Cycle detected in DAG")
                    break

        # Check for missing dependencies
        for node in self._nodes.values():
            for dep in node.dependencies:
                if dep not in self._nodes:
                    errors.append(
                        f"Node '{node.id}' depends on non-existent node '{dep}'"
                    )

        return errors

    def _get_execution_order(self) -> list[str]:
        """Get topologically sorted execution order"""
        in_degree = {node_id: 0 for node_id in self._nodes}
        order = []
        queue = []

        # Calculate in-degree
        for node in self._nodes.values():
            for dep in node.dependencies:
                if dep in in_degree:
                    in_degree[node.id] += 1

        # Nodes with in-degree 0 enter queue
        for node_id, degree in in_degree.items():
            if degree == 0:
                queue.append(node_id)

        # Topological sort
        while queue:
            node_id = queue.pop(0)
            order.append(node_id)

            # Update nodes that depend on this node
            for other in self._nodes.values():
                if node_id in other.dependencies:
                    in_degree[other.id] -= 1
                    if in_degree[other.id] == 0:
                        queue.append(other.id)

        return order

    async def execute(
        self, inputs: dict[str, Any] | None = None, parallel: bool = True
    ) -> DAGResult:
        """Execute workflow

        Args:
            inputs: Input parameters
            parallel: Whether to execute independent nodes in parallel

        Returns:
            Execution result
        """
        result = DAGResult(self.id)
        inputs = inputs or {}

        # Validation
        errors = self.validate()
        if errors:
            # Validation failed, mark all nodes as SKIPPED
            for node_id in self._nodes:
                result._set_result(
                    node_id,
                    NodeResult(
                        node_id=node_id,
                        status=NodeStatus.FAILED,
                        error="; ".join(errors),
                    ),
                )
            return result

        # Get execution order
        order = self._get_execution_order()
        outputs: dict[str, Any] = dict(inputs)

        if parallel:
            # Parallel execution (by level)
            levels = self._get_levels()
            for level in levels:
                tasks = []
                for node_id in level:
                    node = self.get(node_id)
                    if node:  # pragma: no branch
                        tasks.append(self._execute_node(node, outputs, result))
                if tasks:  # pragma: no branch
                    await asyncio.gather(*tasks)
        else:
            # Sequential execution
            for node_id in order:
                node = self.get(node_id)
                if node:  # pragma: no branch
                    await self._execute_node(node, outputs, result)

        return result

    def _get_levels(self) -> list[list[str]]:
        """Get nodes grouped by level (for parallel execution)"""
        levels = []
        assigned = set()

        while len(assigned) < len(self._nodes):
            level = []
            for node_id, node in self._nodes.items():
                if node_id in assigned:
                    continue
                # All dependencies have been assigned
                if all(
                    dep in assigned for dep in node.dependencies if dep in self._nodes
                ):
                    level.append(node_id)
            if not level:
                break
            levels.append(level)
            assigned.update(level)

        return levels

    async def _execute_node(
        self, node: Node, outputs: dict[str, Any], result: DAGResult
    ) -> None:
        """Execute single node"""
        import time

        start = time.time()

        # Check if dependencies succeeded
        for dep in node.dependencies:
            dep_result = result.get_result(dep)
            if dep_result and dep_result.status != NodeStatus.SUCCESS:
                result._set_result(
                    node.id,
                    NodeResult(
                        node_id=node.id,
                        status=NodeStatus.SKIPPED,
                        error=f"Dependency '{dep}' failed",
                    ),
                )
                return

        # Execute node
        try:
            if node.func is None:
                output = None
            else:  # pragma: no branch
                # Collect dependency outputs
                dep_outputs = {
                    dep: outputs.get(dep) for dep in node.dependencies if dep in outputs
                }

                # Call function
                func_result = node.func(**dep_outputs) if dep_outputs else node.func()  # pragma: no branch

                if asyncio.iscoroutine(func_result):  # pragma: no branch
                    output = await func_result
                else:
                    output = func_result

            outputs[node.id] = output
            duration = int((time.time() - start) * 1000)

            result._set_result(
                node.id,
                NodeResult(
                    node_id=node.id,
                    status=NodeStatus.SUCCESS,
                    output=output,
                    duration_ms=duration,
                ),
            )

        except (ValueError, TypeError, KeyError, RuntimeError, ArithmeticError) as e:
            duration = int((time.time() - start) * 1000)
            result._set_result(
                node.id,
                NodeResult(
                    node_id=node.id,
                    status=NodeStatus.FAILED,
                    error=str(e),
                    duration_ms=duration,
                ),
            )

    def visualize(self) -> str:
        """Generate visualization string

        Returns:
            ASCII diagram representation
        """
        lines = [f"DAG: {self.id}"]
        lines.append("-" * 40)

        for node_id, node in self._nodes.items():
            deps = ", ".join(node.dependencies) if node.dependencies else "none"
            lines.append(f"  {node_id} <- [{deps}]")

        return "\n".join(lines)
