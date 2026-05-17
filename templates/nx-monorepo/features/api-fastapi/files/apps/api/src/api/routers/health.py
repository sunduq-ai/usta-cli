"""Health-check router — the only built-in endpoint shipped by the template."""

from fastapi import APIRouter

router = APIRouter()


@router.get("/health")
async def health() -> dict[str, str]:
    return {"status": "healthy"}
