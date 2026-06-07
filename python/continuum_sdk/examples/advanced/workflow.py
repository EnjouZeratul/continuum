"""Workflow DAG Example

Demonstrates how to create and execute DAG workflows.
"""

import asyncio

from continuum_sdk.workflow import DAG, Node

# ==================== Define Node Functions ====================


async def fetch_data():
    """Fetch data"""
    print("   [fetch_data] Fetching data...")
    await asyncio.sleep(0.5)
    return {"records": [1, 2, 3, 4, 5], "source": "api"}


def validate_data(fetch_data):
    """Validate data"""
    print("   [validate_data] Validating data...")
    data = fetch_data
    if not data.get("records"):
        raise ValueError("No data")
    return {"valid": True, "count": len(data["records"])}


async def process_batch_1(fetch_data):
    """Process batch 1"""
    print("   [process_batch_1] Processing...")
    await asyncio.sleep(0.3)
    records = fetch_data["records"][:3]
    return {"batch": 1, "processed": [r * 2 for r in records]}


async def process_batch_2(fetch_data):
    """Process batch 2"""
    print("   [process_batch_2] Processing...")
    await asyncio.sleep(0.4)
    records = fetch_data["records"][3:]
    return {"batch": 2, "processed": [r * 3 for r in records]}


def merge_results(process_batch_1, process_batch_2, validate_data):
    """Merge results"""
    print("   [merge_results] Merging...")
    return {
        "total_processed": len(process_batch_1["processed"])
        + len(process_batch_2["processed"]),
        "batch_1": process_batch_1,
        "batch_2": process_batch_2,
        "validation": validate_data,
    }


async def save_results(merge_results):
    """Save results"""
    print("   [save_results] Saving...")
    await asyncio.sleep(0.2)
    return {"saved": True, "records": merge_results["total_processed"]}


def notify_success(save_results):
    """Notify success"""
    print("   [notify_success] Sending success notification...")
    return {"notified": True, "message": "Workflow completed"}


def notify_failure(**kwargs):
    """Notify failure (fallback node)"""
    print("   [notify_failure] Sending failure notification...")
    return {"notified": False, "message": "Workflow failed"}


# ==================== Build DAG ====================


def create_data_pipeline():
    """Create data processing workflow"""
    dag = DAG("data-pipeline", name="Data Processing Pipeline")

    # Add nodes
    dag.add(Node("fetch", func=fetch_data, description="Fetch data"))
    dag.add(Node("validate", func=validate_data, description="Validate data"))
    dag.add(Node("batch1", func=process_batch_1, description="Process batch 1"))
    dag.add(Node("batch2", func=process_batch_2, description="Process batch 2"))
    dag.add(Node("merge", func=merge_results, description="Merge results"))
    dag.add(Node("save", func=save_results, description="Save results"))
    dag.add(Node("notify_ok", func=notify_success, description="Success notification"))
    dag.add(Node("notify_fail", func=notify_failure, description="Failure notification"))

    # Set dependencies
    dag.depends_on("validate", "fetch")
    dag.depends_on("batch1", "fetch")
    dag.depends_on("batch2", "fetch")
    dag.depends_on("merge", "batch1", "batch2", "validate")
    dag.depends_on("save", "merge")
    dag.depends_on("notify_ok", "save")

    return dag


# ==================== Run Example ====================


async def main():
    print("=== Workflow DAG Example ===\n")

    # 1. Create DAG
    dag = create_data_pipeline()

    # 2. Visualize DAG
    print("1. DAG Structure:")
    print(dag.visualize())
    print()

    # 3. Validate DAG
    print("2. Validate DAG:")
    errors = dag.validate()
    if errors:
        print(f"   Validation failed: {errors}")
        return
    print("   Validation passed\n")

    # 4. Sequential execution
    print("3. Sequential execution:")
    result = await dag.execute(parallel=False)
    print(f"   Status: {result.status.value}")
    print(f"   Execution order: {result.execution_order()}")
    print(f"   Final output: {result.get_output('save')}")
    print()

    # 5. Parallel execution
    print("4. Parallel execution:")
    dag2 = create_data_pipeline()
    result2 = await dag2.execute(parallel=True)
    print(f"   Status: {result2.status.value}")
    print(f"   All outputs: {result2.get_all_outputs()}")
    print()

    # 6. Test failure handling
    print("5. Test failure node:")

    def failing_func():
        raise RuntimeError("Simulated failure")

    dag3 = DAG("test-failure")
    dag3.add(Node("a", func=lambda: "ok"))
    dag3.add(Node("b", func=failing_func).depends_on("a"))
    dag3.add(Node("c", func=lambda: "ok").depends_on("b"))

    result3 = await dag3.execute()
    print(f"   Status: {result3.status.value}")
    print(f"   Failed nodes: {result3.failed_nodes()}")

    # Check node status
    b_result = result3.get_result("b")
    c_result = result3.get_result("c")
    print(f"   b status: {b_result.status.value}, error: {b_result.error}")
    print(f"   c status: {c_result.status.value}, reason: {c_result.error}")


if __name__ == "__main__":
    asyncio.run(main())