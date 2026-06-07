"""Session Management Example

Demonstrates session creation, saving, loading, and other features.
"""

import asyncio

from continuum_sdk import Session


async def main():
    print("=== Session Management Example ===\n")

    # 1. Create new session
    print("1. Create new session")
    session = Session()
    print(f"   Session ID: {session.id}")
    print(f"   Message count: {session.message_count}\n")

    # 2. Add messages
    print("2. Add messages")
    session.add_user_message("First message")
    session.add_assistant_message("Received first message")
    session.add_user_message("Second message")

    messages = session.get_messages()
    print(f"   Message count: {len(messages)}")
    for msg in messages:
        print(f"   - [{msg.role}]: {msg.content[:30]}...")
    print()

    # 3. Save session
    print("3. Save session")
    path = session.save_to_default()
    print(f"   Save path: {path}\n")

    # 4. Session info
    print("4. Session info")
    print(f"   Total cost: ${session.cost:.4f}")
    print(f"   Token count: {session.tokens}")
    print(f"   Tools used: {session.get_tools_used()}\n")

    # 5. List all sessions
    print("5. List all sessions")
    sessions = Session.list_saved_sessions()
    for s in sessions[:5]:  # Show only first 5
        print(f"   - {s}")
    print()

    # 6. Load session
    print("6. Load session")
    loaded = Session.load_from_default(session.id)
    print(f"   Load successful, message count: {loaded.message_count}\n")

    # 7. Export session
    print("7. Export session")
    export_data = session.export()
    print(f"   Export data length: {len(export_data)} characters\n")


if __name__ == "__main__":
    asyncio.run(main())
