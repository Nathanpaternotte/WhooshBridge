"""Offline protocol tests; never log in or upload to a real account."""
import importlib.util
from pathlib import Path
from unittest import TestCase, main
from unittest.mock import patch
import io
import json
import sys
import types

spec = importlib.util.spec_from_file_location("bridge", Path(__file__).parents[1] / "bridge/garmin_bridge.py")
bridge = importlib.util.module_from_spec(spec)
spec.loader.exec_module(bridge)


class BridgeTests(TestCase):
    def test_requires_explicit_import_success(self):
        self.assertEqual(bridge.classify_upload({})["type"], "error")
        self.assertEqual(bridge.classify_upload({"detailedImportResult": {"successes": [{"internalId": 123}]}}), {"type": "uploaded", "activity_id": 123})
        self.assertEqual(bridge.classify_upload({"detailedImportResult": {"failures": [{"messages": [{"code": 202}]}]}})["type"], "duplicate")
        self.assertEqual(bridge.classify_upload({"detailedImportResult": {"failures": [{"messages": [{"code": 999}]}]}})["type"], "error")

    def run_protocol(self, commands, client):
        output = io.StringIO()
        source = io.StringIO("".join(json.dumps(c) + "\n" for c in commands))
        with patch.dict(sys.modules, {"garminconnect": types.SimpleNamespace(Garmin=client)}), patch.object(sys, "stdin", source), patch.object(sys, "stdout", output):
            bridge.main()
        return [json.loads(line) for line in output.getvalue().splitlines()]

    def test_mfa_and_upload_protocol(self):
        class Fake:
            def __init__(self, email, password, prompt_mfa):
                self.mfa = prompt_mfa
            def login(self):
                assert self.mfa() == "123456"
            def upload_activity(self, path):
                assert path == "/private/tmp/test.fit"
                return {"detailedImportResult": {"successes": [{"internalId": 5}]}}
        responses = self.run_protocol([
            {"op": "login", "email": "test", "password": "secret"},
            {"op": "mfa", "code": "123456"},
            {"op": "upload", "path": "/private/tmp/test.fit"},
        ], Fake)
        self.assertEqual([r["type"] for r in responses], ["mfa", "ready", "uploaded"])

    def test_errors_do_not_disclose_credentials(self):
        class Fake:
            def __init__(self, *args, **kwargs):
                raise RuntimeError("password=do-not-disclose token=secret")
        responses = self.run_protocol([{"op": "login", "email": "test", "password": "secret"}], Fake)
        self.assertEqual(responses[0]["type"], "error")
        self.assertNotIn("secret", str(responses))

    def test_http_conflict_is_duplicate(self):
        class Conflict(Exception):
            response = types.SimpleNamespace(status_code=409)
        class Fake:
            def __init__(self, *args, **kwargs): pass
            def login(self): pass
            def upload_activity(self, path): raise Conflict()
        responses = self.run_protocol([{"op": "login", "email": "test", "password": "secret"}, {"op": "upload", "path": "test.fit"}], Fake)
        self.assertEqual(responses[-1]["type"], "duplicate")

    def test_real_garth_wrapper_exposes_http_conflict(self):
        from requests import HTTPError, Response
        from garth.exc import GarthHTTPError
        response = Response()
        response.status_code = 409
        wrapped = GarthHTTPError(msg="private-token", error=HTTPError("private-url", response=response))
        # requests.Response is falsey for 4xx: status extraction must not use `or`.
        self.assertFalse(response)
        self.assertEqual(bridge.http_status(wrapped), 409)
        self.assertEqual(bridge.classify_error(wrapped, "upload")["type"], "duplicate")
        self.assertNotIn("private", str(bridge.classify_error(wrapped, "upload")))

    def test_distinguishes_fit_rejection_auth_and_rate_limit(self):
        from requests import HTTPError, Response
        from garth.exc import GarthHTTPError
        for status in (400, 401, 403, 415, 422, 429, 500):
            response = Response()
            response.status_code = status
            error = GarthHTTPError(msg="secret", error=HTTPError("token", response=response))
            result = bridge.classify_error(error, "upload")
            self.assertEqual(result["type"], "error")
            self.assertEqual(result["code"], f"upload_http_{status}")
            self.assertIn(str(status), result["message"])
            self.assertNotIn("secret", str(result))
            self.assertNotIn("token", str(result))

    def test_cycles_and_text_409_do_not_prove_duplicate(self):
        error = RuntimeError("409 Conflict secret")
        error.__cause__ = error
        self.assertIsNone(bridge.http_status(error))
        self.assertEqual(bridge.classify_error(error, "upload")["type"], "error")

    def test_real_nested_conflict_over_protocol(self):
        from requests import HTTPError, Response
        from garth.exc import GarthHTTPError
        class Fake:
            def __init__(self, *args, **kwargs): pass
            def login(self): pass
            def upload_activity(self, path):
                response = Response()
                response.status_code = 409
                raise GarthHTTPError(msg="HTTP request failed", error=HTTPError(response=response))
        responses = self.run_protocol([{"op": "login", "email": "test", "password": "secret"}, {"op": "upload", "path": "test.fit"}], Fake)
        self.assertEqual(responses[-1], {"type": "duplicate", "code": "upload_http_409"})

    def test_restores_and_checkpoints_rotated_tokens(self):
        class Fake:
            def __init__(self):
                self.garth = types.SimpleNamespace(loads=lambda token: None, dumps=lambda: "ROTATED_TOKEN")
            def login(self, token):
                assert token == "x" * 600
        result = self.run_protocol([{"op": "resume", "tokens": "x" * 600}], Fake)
        self.assertEqual(result, [{"type": "ready", "tokens": "ROTATED_TOKEN"}])

    def test_restore_network_errors_do_not_request_credentials(self):
        from requests import HTTPError, Response, Timeout
        for status in (None, 401, 403, 429, 503):
            class Fake:
                def __init__(self):
                    self.garth = types.SimpleNamespace(loads=lambda token: None, dumps=lambda: "REFRESHED")
                def login(self, token):
                    if status is None: raise Timeout("private")
                    response = Response()
                    response.status_code = status
                    raise HTTPError(response=response)
            result = self.run_protocol([{"op": "resume", "tokens": "x" * 600}], Fake)[0]
            self.assertEqual(result["type"], "auth_required" if status in (401, 403) else "error")
            if status not in (401, 403): self.assertEqual(result["tokens"], "REFRESHED")

    def test_short_store_never_becomes_a_token_file_path(self):
        class Fake:
            def __init__(self): self.garth = types.SimpleNamespace(loads=lambda token: None)
            def login(self, token): raise AssertionError("must not access a file")
        result = self.run_protocol([{"op": "resume", "tokens": "/tmp/token"}], Fake)
        self.assertEqual(result[0]["type"], "auth_required")

    def test_revoked_refresh_grant_is_not_a_fit_or_network_error(self):
        from requests import HTTPError, Response
        response = Response()
        response.status_code = 400
        response._content = b'{"error":"invalid_grant"}'
        self.assertTrue(bridge.auth_refused(HTTPError(response=response)))
        response._content = b'{"error":"invalid_fit"}'
        self.assertFalse(bridge.auth_refused(HTTPError(response=response)))

    def test_real_requests_response_success_and_unknown_body(self):
        from requests import Response
        response = Response()
        response.status_code = 202
        response._content = b'{"detailedImportResult":{"successes":[{"internalId":123}]}}'
        self.assertEqual(bridge.classify_upload(response), {"type":"uploaded", "activity_id":123})
        for body in [b'{}', b'null', b'<html>OK</html>', b'{"detailedImportResult":null}', b'{"detailedImportResult":{"successes":[{}]}}']:
            response._content = body
            self.assertEqual(bridge.classify_upload(response)["type"], "error")

    def test_library_upload_returns_response_over_protocol(self):
        from requests import Response
        class Fake:
            def __init__(self, *args, **kwargs): pass
            def login(self): pass
            def upload_activity(self, path):
                r = Response(); r.status_code = 201
                r._content = b'{"detailedImportResult":{"successes":[{"internalId":321}]}}'
                return r
        result = self.run_protocol([{"op":"login","email":"test","password":"test"},{"op":"upload","path":"test.fit"}], Fake)
        self.assertEqual(result[-1], {"type":"uploaded", "activity_id":321})

    def test_receipt_without_activity_id_is_received_not_failed_or_confirmed(self):
        from requests import Response
        response = Response(); response.status_code = 202
        response._content = b'{"detailedImportResult":{"uploadId":123456,"successes":[],"failures":[]}}'
        self.assertEqual(bridge.classify_upload(response), {"type":"received", "code":"upload_received"})
        for receipt in (None, "", 0, True):
            self.assertEqual(bridge.classify_upload({"detailedImportResult":{"uploadId":receipt,"successes":[],"failures":[]}})["type"], "error")
        self.assertEqual(bridge.classify_upload({"detailedImportResult":{"uploadId":123,"successes":[],"failures":[{"messages":[{"code":999}]}]}})["type"], "error")


if __name__ == "__main__":
    main()
