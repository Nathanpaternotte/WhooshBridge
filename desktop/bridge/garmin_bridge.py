"""Private JSON-lines subprocess. No credentials in argv, files, or logs.

Reuses the GarminService dependency (garminconnect) without changing the CLI.
Python is bundled by PyInstaller for the macOS .app.
"""
import json
import logging
import sys

logging.disable(logging.CRITICAL)


def send(value):
    print(json.dumps(value), flush=True)


def receive():
    line = sys.stdin.readline()
    if not line:
        raise EOFError()
    return json.loads(line)


def mfa():
    send({"type": "mfa"})
    request = receive()
    if request.get("op") != "mfa" or not request.get("code"):
        raise ValueError("Code MFA absent")
    return request["code"]


def classify_upload(response):
    """An HTTP 200 alone does not prove Garmin accepted the FIT."""
    if not isinstance(response, dict) and callable(getattr(response, "json", None)):
        response.raise_for_status()
        try:
            response = response.json()
        except ValueError:
            response = None
    result = response.get("detailedImportResult") if isinstance(response, dict) else None
    if isinstance(result, dict):
        successes = result.get("successes")
        failures = result.get("failures")
        if isinstance(successes, list):
            for success in successes:
                if isinstance(success, dict):
                    activity_id = success.get("internalId")
                    if isinstance(activity_id, (int, str)) and not isinstance(activity_id, bool) and str(activity_id).isdigit() and int(activity_id) > 0:
                        return {"type": "uploaded", "activity_id": activity_id}
        if isinstance(failures, list) and failures:
            messages = [m for f in failures if isinstance(f, dict) for m in (f.get("messages") or []) if isinstance(m, dict)]
            if any(str(m.get("code")) == "202" for m in messages):
                return {"type": "duplicate"}
            return {"type": "error", "message": "Garmin a rejeté le fichier FIT."}
        # Recent upload API returns a receipt without an activity ID. A receipt
        # confirms reception, not the completion of processing. Never retry it.
        receipt = result.get("uploadId")
        if successes == [] and failures == [] and isinstance(receipt, (str, int)) and not isinstance(receipt, bool) and str(receipt).isdigit() and int(receipt) > 0:
            return {"type": "received", "code": "upload_received"}
    return {"type": "error", "code": "upload_unconfirmed_result" if isinstance(result, dict) else "upload_unconfirmed_body", "message": "Réponse Garmin sans confirmation d’import. Vérifiez Garmin Connect avant de réessayer."}


def http_status(exc):
    """Garth wraps requests.HTTPError in .error, not .response."""
    pending, seen = [exc], set()
    while pending:
        current = pending.pop()
        if id(current) in seen:
            continue
        seen.add(id(current))
        response = getattr(current, "response", None)
        status = getattr(response, "status_code", None)
        if isinstance(status, int) and 100 <= status <= 599:
            return status
        for attr in ("error", "__cause__", "__context__"):
            nested = getattr(current, attr, None)
            if isinstance(nested, BaseException):
                pending.append(nested)
    return None


def auth_refused(exc):
    if http_status(exc) in (401, 403):
        return True
    # OAuth can reject an expired/revoked renewal grant with HTTP 400.
    pending, seen = [exc], set()
    while pending:
        current = pending.pop()
        if id(current) in seen:
            continue
        seen.add(id(current))
        response = getattr(current, "response", None)
        if getattr(response, "status_code", None) == 400:
            try:
                payload = response.json()
                if isinstance(payload, dict) and payload.get("error") in ("invalid_grant", "invalid_token"):
                    return True
            except (ValueError, TypeError, AttributeError):
                pass
        for attr in ("error", "__cause__", "__context__"):
            nested = getattr(current, attr, None)
            if isinstance(nested, BaseException):
                pending.append(nested)
    return False


def classify_error(exc, operation):
    status = http_status(exc)
    stage = "upload" if operation == "upload" else "login"
    code = f"{stage}_http_{status}" if status else f"{stage}_error"
    if stage == "upload" and status == 409:
        return {"type": "duplicate", "code": code}
    if status in (401, 403):
        message = "Garmin refuse l’accès (HTTP %s). Reconnectez le compte Garmin." % status
    elif status == 429:
        message = "Garmin limite les requêtes (HTTP 429). Patientez avant de réessayer."
    elif stage == "upload" and status in (400, 415, 422):
        message = "Garmin a refusé le fichier FIT (HTTP %s). Ce n’est pas une erreur de connexion." % status
    elif status and status >= 500:
        message = "Erreur du serveur Garmin (HTTP %s). Vérifiez si l’activité a été importée avant de réessayer." % status
    elif status:
        message = "Requête Garmin refusée (HTTP %s)." % status
    elif stage == "upload":
        # Never include exception text: it can contain URLs, tokens and paths.
        message = "Envoi Garmin non confirmé. Vérifiez si l’activité apparaît sur Garmin Connect avant de réessayer."
    else:
        message = "Connexion Garmin impossible. Vérifiez les identifiants, le code MFA et la connexion réseau."
    return {"type": "error", "code": code, "message": message}


def checkpoint(client, response):
    """Return refreshed tokens only through the private parent pipe."""
    if client is not None and getattr(client, "garth", None) is not None:
        try:
            response["tokens"] = client.garth.dumps()
        except Exception:
            response["session_warning"] = True
    return response


def main():
    from garminconnect import Garmin
    client = None
    while True:
        try:
            request = receive()
        except EOFError:
            return
        try:
            if request["op"] == "resume":
                candidate = Garmin()
                tokens = request["tokens"]
                # Validate the serialized store before network I/O. Never interpret
                # a short/corrupt token string as a filesystem path.
                try:
                    candidate.garth.loads(tokens)
                except (ValueError, TypeError, KeyError, AssertionError):
                    send({"type": "auth_required", "code": "saved_session_invalid"})
                    continue
                if len(tokens) <= 512:
                    send({"type": "auth_required", "code": "saved_session_invalid"})
                    continue
                client = candidate
                candidate.login(tokens)
                send(checkpoint(client, {"type": "ready"}))
            elif request["op"] == "login":
                client = None
                candidate = Garmin(request["email"], request["password"], prompt_mfa=mfa)
                candidate.login()
                client = candidate
                send(checkpoint(client, {"type": "ready"}))
            elif request["op"] == "upload" and client is not None:
                send(checkpoint(client, classify_upload(client.upload_activity(request["path"]))))
            else:
                send({"type": "error", "code": "session_missing", "message": "Reconnectez-vous à Garmin Connect."})
        except Exception as exc:
            if auth_refused(exc):
                send({"type": "auth_required", "code": "session_refused"})
            else:
                send(checkpoint(client, classify_error(exc, request.get("op"))))
        finally:
            request.clear()


if __name__ == "__main__":
    main()
