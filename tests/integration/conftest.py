import shutil
import socket
import subprocess
import time

import pytest

IMAGE = "ztask-router:integration-test"
CONTAINER_NAME = "ztask-router-integration-test"
PORT = 17447


def _runtime() -> str:
    for candidate in ("container", "docker"):
        if shutil.which(candidate):
            return candidate
    pytest.skip("no container runtime (docker/container) found on PATH")


def _wait_for_port(host: str, port: int, timeout: float = 30.0) -> None:
    deadline = time.time() + timeout
    while time.time() < deadline:
        try:
            with socket.create_connection((host, port), timeout=1):
                return
        except OSError:
            time.sleep(0.5)
    raise TimeoutError(f"router did not open port {port} within {timeout}s")


@pytest.fixture(scope="session")
def router():
    runtime = _runtime()
    subprocess.run(
        [runtime, "build", "-f", "docker/router/Dockerfile", "-t", IMAGE, "."],
        check=True,
    )
    subprocess.run(
        [runtime, "rm", "-f", CONTAINER_NAME],
        check=False,
        capture_output=True,
    )
    subprocess.run(
        [
            runtime, "run", "--rm", "-d",
            "--name", CONTAINER_NAME,
            "-p", f"{PORT}:7447",
            IMAGE,
        ],
        check=True,
    )
    try:
        _wait_for_port("localhost", PORT)
        yield f"tcp/localhost:{PORT}"
    finally:
        subprocess.run([runtime, "stop", CONTAINER_NAME], check=False)
