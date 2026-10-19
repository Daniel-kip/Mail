The **Sending API** is Mailvora's REST interface for applications. It sends transactional email such as invoices, receipts, sign-in codes and alerts, and reports what happened to each message.

| | |
|---|---|
| **Base URL** | `https://api.<your-domain>/v1` |
| **Authentication** | `Authorization: Bearer mv_live_…` (or `mv_test_…`) |
| **Format** | JSON over HTTPS |
| **Idempotency** | `Idempotency-Key` header on `POST /v1/messages` |
| **Events** | Signed webhooks (`Mailvora-Signature`) |

The in-app **API quickstart** page (`/developers`) has copy-ready Node, Python and PHP examples.

## Authentication

Create a key in the console under **API keys**. A key looks like `mv_live_<12 characters>_<40 characters>`; `mv_test_` keys process messages without delivering them. The secret is shown once and stored only as a hash.

Give each application its own key with only the permissions it needs, restrict it by IP address and sender domain, set an expiry, and rotate it regularly.

## Sending a message

```bash
curl https://api.example.com/v1/messages \
  -H "Authorization: Bearer $MAILVORA_API_KEY" \
  -H "Idempotency-Key: $(uuidgen)" \
  -H "Content-Type: application/json" \
  -d '{"from":"billing@school.example","to":"parent@example.org",
       "template":"invoice","variables":{"number":"INV-1042"}}'
```

`202 Accepted` means the message was stored and queued, not that it was delivered. Track progress with `GET /v1/messages/{id}`, in the console, or through webhooks. Statuses: `scheduled`, `cancelled`, `queued`, `processing`, `accepted_by_mta`, `delivered`, `deferred`, `bounced`, `failed`, `suppressed` and `sandboxed` (test keys).

Repeating a request with the same `Idempotency-Key` returns the original result instead of sending twice, so it is safe to retry after a timeout.

## Endpoints

| Method | Path | Permission |
|---|---|---|
| `POST` | `/v1/messages` | `messages:send` |
| `GET` | `/v1/messages` | `messages:read` |
| `GET` | `/v1/messages/{id}` | `messages:read` |
| `POST` | `/v1/messages/{id}/cancel` | `messages:send` |
| `GET` | `/v1/messages/export.csv` | `messages:read` |
| `GET` | `/v1/templates` | `templates:read` |
| `POST` | `/v1/templates` | `templates:write` |
| `POST` | `/v1/templates/{id}/preview` | `templates:read` |
| `GET` | `/v1/suppressions` | `suppressions:read` |
| `POST` | `/v1/suppressions` | `suppressions:write` |
| `DELETE` | `/v1/suppressions/{email}` | `suppressions:write` |
| `GET` | `/v1/domains` | `domains:read` |
| `GET` | `/v1/webhooks` | `webhooks:read` |
| `POST` | `/v1/webhooks` | `webhooks:write` |
| `GET` | `/v1/usage` | `usage:read` |
| `GET` | `/v1/mailboxes` | `mailboxes:read` |

## Webhooks

Register an HTTPS endpoint in the console to receive message events: `scheduled`, `queued`, `accepted`, `delivered`, `deferred`, `bounced`, `complained`, `failed`, `suppressed`, `unsubscribed` and `cancelled` (each prefixed with `message.`). Every request carries these headers:

| Header | Meaning |
|---|---|
| `Mailvora-Signature` | `t=<unix time>,v1=<hex HMAC-SHA256>` over `"<t>.<raw body>"`, keyed with the endpoint secret |
| `Mailvora-Event-Id` | Unique event ID (use it to ignore duplicates) |
| `Mailvora-Event-Type` | For example `message.delivered` |
| `Mailvora-Delivery-Id` | This delivery attempt's ID |
| `Mailvora-Delivery-Attempt` | Attempt number, starting at 1 |

Verify the signature over the **raw** body and reject timestamps older than five minutes to prevent replay. Failed deliveries are retried with exponential backoff and can be replayed from the console.

## Bounces, complaints and suppressions

Hard bounces, spam complaints and one-click unsubscribes (RFC 8058) add the address to the organisation's suppression list automatically; later messages to it are refused without being sent. Remove an entry with `DELETE /v1/suppressions/{email}` once the problem is fixed.

## See also

- [Administrator guide](Administrator-Guide.md)
- [Deliverability tools](Deliverability-Tools.md)
- [Security model](Security-Model.md)
