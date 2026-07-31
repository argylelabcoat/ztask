import os
from contextlib import contextmanager

import zenoh

DEFAULT_ENDPOINT = "tcp/localhost:7447"
ENDPOINT_ENV_VAR = "ZTASK_ZENOH_ENDPOINT"


def resolve_endpoint() -> str:
    return os.environ.get(ENDPOINT_ENV_VAR, DEFAULT_ENDPOINT)


@contextmanager
def open_session():
    endpoint = resolve_endpoint()
    config = zenoh.Config()
    config.insert_json5("connect/endpoints", f'["{endpoint}"]')
    with zenoh.open(config) as session:
        yield session
