"""
Continuum - Terminal Agent Framework

Quick Start (3 steps):
    >>> from continuum import Agent
    >>> agent = Agent()
    >>> agent.run("hello")
"""

# Re-export from continuum_sdk
from continuum_sdk import Agent, Config, ConfigLoader, Session, load_config

__version__ = "1.0.0"
__all__ = ["Agent", "Session", "Config", "ConfigLoader", "load_config"]
