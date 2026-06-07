import pytest

import continuum
import continuum_sdk
from continuum import Agent, Config, Session
from continuum_sdk.agent.session import MessageRole


def test_continuum_reexports_stable_sdk_api():
    assert Agent is continuum_sdk.Agent
    assert Session is continuum_sdk.Session
    assert Config is continuum_sdk.Config
    assert continuum.__all__ == [
        "Agent",
        "Session",
        "Config",
        "ConfigLoader",
        "load_config",
    ]


def test_public_session_uses_real_session_implementation():
    session = Session(id="contract-session")

    user_message = session.add_user_message("Hello")
    assistant_message = session.add_assistant_message("Hi")
    system_message = session.add_system_message("System")

    assert session.id == "contract-session"
    assert session.message_count == 3
    assert user_message.role == MessageRole.USER
    assert assistant_message.role == MessageRole.ASSISTANT
    assert system_message.role == MessageRole.SYSTEM
    assert [message.content for message in session.get_messages()] == [
        "Hello",
        "Hi",
        "System",
    ]
    assert [message.content for message in session.get_messages(limit=2)] == [
        "Hi",
        "System",
    ]
    assert session.get_last_message().content == "System"


def test_public_session_save_load_path_round_trip(tmp_path):
    path = tmp_path / "session.json"
    session = Session(id="persisted-session")
    session.add_user_message("Hello")
    session.add_assistant_message("Hi")

    saved_path = session.save(path)
    loaded = Session.load(path)

    assert saved_path == path
    assert loaded.id == "persisted-session"
    assert loaded.message_count == 2
    assert [message.content for message in loaded.get_messages()] == ["Hello", "Hi"]


def test_public_session_to_dict_from_dict_round_trip():
    session = Session(id="dict-session")
    session.add_user_message("Hello")
    session.add_assistant_message("Hi")

    data = session.to_dict()
    restored = Session.from_dict(data)

    assert data["id"] == "dict-session"
    assert restored.id == "dict-session"
    assert restored.message_count == 2
    assert [message.content for message in restored.get_messages()] == ["Hello", "Hi"]


def test_public_agent_constructor_overrides_config():
    agent = Agent(config=Config(api_key="k"), model="custom-model", provider="openai")

    config = agent._agent._config

    assert config.api_key == "k"
    assert config.provider == "openai"
    assert config.model == "custom-model"


def test_public_agent_explicit_args_override_config_fields():
    agent = Agent(
        config=Config(api_key="config-key", model="config-model", provider="anthropic"),
        model="custom-model",
        provider="openai",
        api_key="explicit-key",
    )

    config = agent._agent._config

    assert config.api_key == "explicit-key"
    assert config.provider == "openai"
    assert config.model == "custom-model"


def test_public_agent_contract_without_api_key(monkeypatch):
    for key in [
        "CONTINUUM_API_KEY",
        "ANTHROPIC_API_KEY",
        "OPENAI_API_KEY",
        "GOOGLE_API_KEY",
        "GEMINI_API_KEY",
        "TOGETHER_API_KEY",
        "GROQ_API_KEY",
        "DEEPSEEK_API_KEY",
        "MOONSHOT_API_KEY",
        "GLM_API_KEY",
        "KIMI_API_KEY",
        "QWEN_API_KEY",
        "XAI_API_KEY",
    ]:
        monkeypatch.delenv(key, raising=False)

    agent = Agent(config=Config())
    session = agent.create_session("contract-agent-session")

    assert session.id == "contract-agent-session"
    assert not hasattr(agent, "chat")
    assert not hasattr(agent, "resume_session")
    with pytest.raises(ValueError, match="API key"):
        agent.run("hello")


def test_public_config_from_env_and_use(monkeypatch):
    monkeypatch.setenv("CONTINUUM_PROVIDER", "openai")
    monkeypatch.setenv("CONTINUUM_API_KEY", "test-key")
    monkeypatch.setenv("CONTINUUM_MODEL", "gpt-test")

    config = Config.from_env()

    assert config.provider == "openai"
    assert config.api_key == "test-key"
    assert config.model == "gpt-test"
    assert config.use("anthropic") is config
    assert config.provider == "anthropic"


def test_public_config_from_file(tmp_path):
    path = tmp_path / "config.toml"
    path.write_text(
        'provider = "anthropic"\napi_key = "file-key"\nmodel = "claude-test"\n',
        encoding="utf-8",
    )

    config = Config.from_file(path)

    assert config.provider == "anthropic"
    assert config.api_key == "file-key"
    assert config.model == "claude-test"
