class FakePayload:
    def __init__(self, value: str):
        self._value = value

    def to_string(self) -> str:
        return self._value


class FakeOk:
    def __init__(self, key_expr: str, payload: str):
        self.key_expr = key_expr
        self.payload = FakePayload(payload)


class FakeReply:
    def __init__(self, key_expr: str = "", payload: str = "", ok: bool = True):
        self.ok = FakeOk(key_expr, payload) if ok else None


class FakeSession:
    """Maps key_expr strings (as passed to .get) to a list of FakeReply."""

    def __init__(self, replies_by_key_expr: dict):
        self._replies_by_key_expr = replies_by_key_expr
        self.put_calls = []

    def get(self, key_expr: str):
        return self._replies_by_key_expr.get(key_expr, [])

    def put(self, key_expr: str, value: str):
        self.put_calls.append((key_expr, value))
