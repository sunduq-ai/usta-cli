"""JWT auth router — login + me stubs.

The actual user store is left for the application code to define. This
router exists so routes are wired and the template ships a working
`/api/auth` namespace.
"""

from datetime import datetime, timedelta, timezone

from fastapi import APIRouter, Depends, HTTPException, status
from fastapi.security import OAuth2PasswordBearer, OAuth2PasswordRequestForm
from jose import JWTError, jwt
from passlib.context import CryptContext
from pydantic import BaseModel

from src.api.dependencies import get_settings
from src.infrastructure.config import Settings

router = APIRouter()
pwd_context = CryptContext(schemes=["argon2"], deprecated="auto")
oauth2_scheme = OAuth2PasswordBearer(tokenUrl="/api/auth/login")


class TokenResponse(BaseModel):
    access_token: str
    token_type: str = "bearer"


class UserResponse(BaseModel):
    username: str


def _create_access_token(sub: str, settings: Settings) -> str:
    expire = datetime.now(timezone.utc) + timedelta(minutes=60)
    payload = {"sub": sub, "exp": expire}
    return jwt.encode(payload, settings.SECRET_KEY, algorithm="HS256")


@router.post("/login", response_model=TokenResponse)
async def login(
    form: OAuth2PasswordRequestForm = Depends(),
    settings: Settings = Depends(get_settings),
) -> TokenResponse:
    # Replace this with a real user lookup against your domain.
    if form.password != "demo":
        raise HTTPException(status_code=status.HTTP_401_UNAUTHORIZED, detail="bad creds")
    return TokenResponse(access_token=_create_access_token(form.username, settings))


@router.get("/me", response_model=UserResponse)
async def me(
    token: str = Depends(oauth2_scheme),
    settings: Settings = Depends(get_settings),
) -> UserResponse:
    try:
        payload = jwt.decode(token, settings.SECRET_KEY, algorithms=["HS256"])
        return UserResponse(username=payload["sub"])
    except JWTError as exc:
        raise HTTPException(status_code=status.HTTP_401_UNAUTHORIZED, detail="invalid token") from exc
