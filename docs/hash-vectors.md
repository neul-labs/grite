# Hash Vectors

This file provides canonical test vectors for `event_id` computation.

## Vector 1 (IssueCreated)

**Fields**

- `schema_version`: 1
- `issue_id`: `000102030405060708090a0b0c0d0e0f`
- `actor`: `101112131415161718191a1b1c1d1e1f`
- `ts_unix_ms`: 1700000000000
- `parent`: null
- `kind_tag`: 1 (`IssueCreated`)
- `kind_payload`: `["Test", "Body", ["bug", "p0"]]`

**Canonical CBOR preimage (hex)**

```
870150000102030405060708090a0b0c0d0e0f50101112131415161718191a1b1c1d1e1f1b0000018bcfe56800f60183645465737464426f64798263627567627030
```

**Expected `event_id` (BLAKE2b-256, hex)**

```
9c2aee7924bf7482dd3842c6ec32fd5103883b9d2354f63df2075ac61fe3d827
```
