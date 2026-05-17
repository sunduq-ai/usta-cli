"""FastAPI dependency providers.

Concrete adapter wiring lives here. Use cases in `src/application/` should
declare their dependencies as protocol-typed parameters and have them
provided from this module.
"""

from src.infrastructure.config import Settings, settings


def get_settings() -> Settings:
    return settings
