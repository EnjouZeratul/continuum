"""Hello Agent Example

The simplest Agent usage example.
"""

import asyncio

from continuum_sdk import Agent, Session


async def main():
    # Create session
    session = Session()
    session.add_user_message("Hello! Please introduce yourself.")

    # Create Agent
    agent = Agent()

    print("=== Hello Agent Example ===\n")

    # Send message
    response = agent.run("Hello! Please introduce yourself.")
    print(f"Agent: {response}\n")
    session.add_assistant_message(response)

    # Continue conversation
    response = agent.run("What can you do?")
    print(f"Agent: {response}\n")
    session.add_assistant_message(response)

    # Save session
    session.save_to_default()
    print(f"Session saved, ID: {session.id}")


if __name__ == "__main__":
    asyncio.run(main())