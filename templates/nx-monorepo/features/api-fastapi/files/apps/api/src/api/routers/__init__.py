"""HTTP routers — exported for `main.py` to register."""

from src.api.routers.health import router as health_router

__all__ = ["health_router"]
