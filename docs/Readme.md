# Documentation

Our documentation can be found at: <https://rustic.cli.rs/docs>

## Generate OpenAPI Schema

You can generate the HTTP API OpenAPI schema directly from code and write it under
`docs/` with:

```bash
cargo run -- serve --openapi-output docs/openapi.json
```

This command serializes `ApiDoc::openapi()` and exits without starting the server.
