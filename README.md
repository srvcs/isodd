# srvcs-isodd

The other parity primitive of the srvcs.cloud distributed standard library.

Its single concern: **is the number odd?** Odd is defined as "not even." Rather
than reimplement parity — and risk diverging from it — `srvcs-isodd` delegates to
[`srvcs-iseven`](https://github.com/srvcs/iseven) over HTTP and negates the
result. This keeps a single source of truth for parity across the platform.

If `srvcs-iseven` is unreachable, `srvcs-isodd` reports itself **degraded
(503)**. Invalid input is forwarded from `srvcs-iseven` unchanged (`422`).

## API

| Method | Path | Purpose |
| --- | --- | --- |
| `GET` | `/` | Service identity, concern, and dependency list |
| `POST` | `/` | Is `value` odd? |
| `GET` | `/healthz` `/readyz` `/metrics` `/openapi.json` | srvcs service standard surface |

```sh
curl -s -X POST localhost:8080/ -H 'content-type: application/json' -d '{"value": 7}'
# {"value":7,"result":true}
```

Responses:

- `200 {"value": n, "result": bool}` — evaluated (the negation of `srvcs-iseven`).
- `422` — invalid input, forwarded from `srvcs-iseven`.
- `503` — a dependency is unavailable.

## Dependencies

- [`srvcs-iseven`](https://github.com/srvcs/iseven) — parity (negated here).

Transitively, this means `srvcs-isodd` also depends on `srvcs-isnumber`, through
`srvcs-iseven`.

## Configuration

| Variable | Default | Purpose |
| --- | --- | --- |
| `SRVCS_BIND_ADDR` | `0.0.0.0:8080` | Bind address |
| `SRVCS_ISEVEN_URL` | `http://127.0.0.1:8082` | Base URL of `srvcs-iseven` |
| `SRVCS_ENV` | `development` | Environment label for logs |
| `RUST_LOG` | `info,tower_http=info` | Tracing filter |

## Local checks

```sh
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
```

Orchestration tests stand up a mock `srvcs-iseven` in-process. See
[`srvcs/platform`](https://github.com/srvcs/platform) for the shared standard.

> Note: the `cargoHash` in `flake.nix` is inherited from the template and must be
> refreshed with a `nix build` before the Nix gates pass.
