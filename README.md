# bearssl

BearSSL bindings for the Rust programming language.

## Configuration

By default, it builds a particular git revision. The revision can
be changed by setting `BEARSSL_GIT_HASH`.

Other options include:

- `BEARSSL_PRECOMPILED_PATH`: the directory where pre-built
  libraries can be found.
- `BEARSSL_SOURCE_PATH`: the directory where BoringSSL source file
  can be found.
- `BEARSSL_INCLUDE_PATH`: the directory where BoringSSL header files
  can be found. (Note: make sure this stays up-to-date with the
  source files!)

Note that `BEARSSL_GIT_HASH`, `BEARSSL_PRECOMPILED_PATH`, and
`BEARSSL_SOURCE_PATH` are mutually exclusive.
