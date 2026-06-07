"""Hello World - The Simplest Quick Start Example

Goal: Launch Agent in 3 steps
"""

from continuum_sdk import Agent

# Step 1: Import (done above)

# Step 2: Create Agent (auto-configures from environment)
agent = Agent()

# Step 3: Run task
result = agent.run("hello")
print(f"Result: {result}")

# Optional: Continue conversation (using run method)
response = agent.run("Hello, please introduce yourself")
print(f"Agent: {response}")