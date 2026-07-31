from ztask import zenoh_client


def test_resolve_endpoint_default(monkeypatch):
    monkeypatch.delenv(zenoh_client.ENDPOINT_ENV_VAR, raising=False)
    assert zenoh_client.resolve_endpoint() == "tcp/localhost:7447"


def test_resolve_endpoint_from_env(monkeypatch):
    monkeypatch.setenv(zenoh_client.ENDPOINT_ENV_VAR, "tcp/zenoh-router:7447")
    assert zenoh_client.resolve_endpoint() == "tcp/zenoh-router:7447"


def test_open_session_configures_connect_endpoint(monkeypatch, mocker):
    monkeypatch.setenv(zenoh_client.ENDPOINT_ENV_VAR, "tcp/zenoh-router:7447")

    fake_config = mocker.MagicMock()
    fake_session = mocker.MagicMock()
    fake_session.__enter__.return_value = "the-session"
    fake_session.__exit__.return_value = False

    mocker.patch.object(zenoh_client.zenoh, "Config", return_value=fake_config)
    mock_open = mocker.patch.object(zenoh_client.zenoh, "open", return_value=fake_session)

    with zenoh_client.open_session() as session:
        assert session == "the-session"

    fake_config.insert_json5.assert_called_once_with(
        "connect/endpoints", '["tcp/zenoh-router:7447"]'
    )
    mock_open.assert_called_once_with(fake_config)
