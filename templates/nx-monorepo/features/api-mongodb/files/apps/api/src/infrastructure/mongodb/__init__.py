"""MongoDB infrastructure: client + base repository utilities."""

from src.infrastructure.mongodb.client import (
    close_mongo_connection,
    connect_to_mongo,
    get_db,
)

__all__ = ["close_mongo_connection", "connect_to_mongo", "get_db"]
