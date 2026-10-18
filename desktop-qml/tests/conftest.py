"""Keep the long-lived Qt bridge suite ahead of other QCoreApplication users."""

from __future__ import annotations


def pytest_collection_modifyitems(items):
    # Statuses/Channels exercise repeated QCoreApplication event loops and
    # QtNetwork teardown. Running that module before other bridge fixtures
    # avoids stale posted events from unrelated test rigs on Qt 6.11.
    items.sort(key=lambda item: item.path.name != "test_status_channels.py")
