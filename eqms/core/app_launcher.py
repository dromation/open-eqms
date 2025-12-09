"""Simple app launcher stub used by Flask endpoint."""

from typing import Dict


def launch(app_name: str) -> Dict[str, str]:
    """Return a stub response for the requested app."""
    return {
        "app": app_name,
        "status": "ok",
        "message": f"Launch stub for {app_name} (implement dispatcher)",
    }
