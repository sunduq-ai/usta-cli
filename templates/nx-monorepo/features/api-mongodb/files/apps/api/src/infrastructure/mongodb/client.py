"""Async MongoDB client lifecycle.

Wired into `main.py`'s `lifespan` via the engine's anchor injection.
Repositories should depend on `get_db()` rather than reaching for the
client directly.
"""

from typing import Optional

from motor.motor_asyncio import AsyncIOMotorClient, AsyncIOMotorDatabase

from src.infrastructure.config import settings

_client: Optional[AsyncIOMotorClient] = None


async def connect_to_mongo() -> None:
    global _client
    _client = AsyncIOMotorClient(settings.MONGODB_URL)


async def close_mongo_connection() -> None:
    global _client
    if _client is not None:
        _client.close()
        _client = None


def get_db() -> AsyncIOMotorDatabase:
    if _client is None:
        raise RuntimeError("MongoDB client is not initialized")
    return _client[settings.MONGODB_DATABASE]
