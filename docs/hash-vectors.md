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

## Vector 2 (IssueUpdated)

**Fields**

- `schema_version`: 1
- `issue_id`: `000102030405060708090a0b0c0d0e0f`
- `actor`: `101112131415161718191a1b1c1d1e1f`
- `ts_unix_ms`: 1700000000000
- `parent`: null
- `kind_tag`: 2 (`IssueUpdated`)
- `kind_payload`: `["Title 2", null]`

**Canonical CBOR preimage (hex)**

```
870150000102030405060708090a0b0c0d0e0f50101112131415161718191a1b1c1d1e1f1b0000018bcfe56800f60282675469746c652032f6
```

**Expected `event_id` (BLAKE2b-256, hex)**

```
5227efec6ae3d41725827edb3e62d00a595784d7adec58fb4e1b787c44c4b333
```

## Vector 3 (CommentAdded)

**Fields**

- `schema_version`: 1
- `issue_id`: `000102030405060708090a0b0c0d0e0f`
- `actor`: `101112131415161718191a1b1c1d1e1f`
- `ts_unix_ms`: 1700000001000
- `parent`: `202122232425262728292a2b2c2d2e2f303132333435363738393a3b3c3d3e3f`
- `kind_tag`: 3 (`CommentAdded`)
- `kind_payload`: `["Looks good"]`

**Canonical CBOR preimage (hex)**

```
870150000102030405060708090a0b0c0d0e0f50101112131415161718191a1b1c1d1e1f1b0000018bcfe56be85820202122232425262728292a2b2c2d2e2f303132333435363738393a3b3c3d3e3f03816a4c6f6f6b7320676f6f64
```

**Expected `event_id` (BLAKE2b-256, hex)**

```
fca597420160df9f7230b28384a27dc86656b206520e5c8085e78cbb02a46e27
```

## Vector 4 (LabelAdded)

**Fields**

- `schema_version`: 1
- `issue_id`: `000102030405060708090a0b0c0d0e0f`
- `actor`: `101112131415161718191a1b1c1d1e1f`
- `ts_unix_ms`: 1700000002000
- `parent`: null
- `kind_tag`: 4 (`LabelAdded`)
- `kind_payload`: `["bug"]`

**Canonical CBOR preimage (hex)**

```
870150000102030405060708090a0b0c0d0e0f50101112131415161718191a1b1c1d1e1f1b0000018bcfe56fd0f6048163627567
```

**Expected `event_id` (BLAKE2b-256, hex)**

```
d742a0d9c83f17176e30511d62045686b491ddf55f8d1dfe7a74921787bdd436
```

## Vector 5 (LabelRemoved)

**Fields**

- `schema_version`: 1
- `issue_id`: `000102030405060708090a0b0c0d0e0f`
- `actor`: `101112131415161718191a1b1c1d1e1f`
- `ts_unix_ms`: 1700000003000
- `parent`: null
- `kind_tag`: 5 (`LabelRemoved`)
- `kind_payload`: `["wip"]`

**Canonical CBOR preimage (hex)**

```
870150000102030405060708090a0b0c0d0e0f50101112131415161718191a1b1c1d1e1f1b0000018bcfe573b8f6058163776970
```

**Expected `event_id` (BLAKE2b-256, hex)**

```
f23e9c69c3fa4cd2889e57fe1c547630afa132052197a5fe449e6d5acf22c40c
```

## Vector 6 (StateChanged)

**Fields**

- `schema_version`: 1
- `issue_id`: `000102030405060708090a0b0c0d0e0f`
- `actor`: `101112131415161718191a1b1c1d1e1f`
- `ts_unix_ms`: 1700000004000
- `parent`: null
- `kind_tag`: 6 (`StateChanged`)
- `kind_payload`: `["closed"]`

**Canonical CBOR preimage (hex)**

```
870150000102030405060708090a0b0c0d0e0f50101112131415161718191a1b1c1d1e1f1b0000018bcfe577a0f6068166636c6f736564
```

**Expected `event_id` (BLAKE2b-256, hex)**

```
839ae6d0898f48efcc7a41fdbb9631e64ba1f05a6c1725fc196971bfd1645b2b
```

## Vector 7 (LinkAdded)

**Fields**

- `schema_version`: 1
- `issue_id`: `000102030405060708090a0b0c0d0e0f`
- `actor`: `101112131415161718191a1b1c1d1e1f`
- `ts_unix_ms`: 1700000005000
- `parent`: null
- `kind_tag`: 7 (`LinkAdded`)
- `kind_payload`: `["https://example.com", "ref"]`

**Canonical CBOR preimage (hex)**

```
870150000102030405060708090a0b0c0d0e0f50101112131415161718191a1b1c1d1e1f1b0000018bcfe57b88f607827368747470733a2f2f6578616d706c652e636f6d63726566
```

**Expected `event_id` (BLAKE2b-256, hex)**

```
b8af76be8b7a40244bb8e731130ed52969a77b87532dadf9a00a352eeb00e3b5
```

## Vector 8 (AssigneeAdded)

**Fields**

- `schema_version`: 1
- `issue_id`: `000102030405060708090a0b0c0d0e0f`
- `actor`: `101112131415161718191a1b1c1d1e1f`
- `ts_unix_ms`: 1700000006000
- `parent`: null
- `kind_tag`: 8 (`AssigneeAdded`)
- `kind_payload`: `["alice"]`

**Canonical CBOR preimage (hex)**

```
870150000102030405060708090a0b0c0d0e0f50101112131415161718191a1b1c1d1e1f1b0000018bcfe57f70f6088165616c696365
```

**Expected `event_id` (BLAKE2b-256, hex)**

```
42f329d826d34d425dd67080d91f6c909bc56411c9add54389fbec5d457b14e4
```

## Vector 9 (AssigneeRemoved)

**Fields**

- `schema_version`: 1
- `issue_id`: `000102030405060708090a0b0c0d0e0f`
- `actor`: `101112131415161718191a1b1c1d1e1f`
- `ts_unix_ms`: 1700000007000
- `parent`: null
- `kind_tag`: 9 (`AssigneeRemoved`)
- `kind_payload`: `["alice"]`

**Canonical CBOR preimage (hex)**

```
870150000102030405060708090a0b0c0d0e0f50101112131415161718191a1b1c1d1e1f1b0000018bcfe58358f6098165616c696365
```

**Expected `event_id` (BLAKE2b-256, hex)**

```
bfb0fdfed0f0ee36f31107963317dd904143f37d9ef8792f64272cf2f07f6a1e
```

## Vector 10 (AttachmentAdded)

**Fields**

- `schema_version`: 1
- `issue_id`: `000102030405060708090a0b0c0d0e0f`
- `actor`: `101112131415161718191a1b1c1d1e1f`
- `ts_unix_ms`: 1700000008000
- `parent`: null
- `kind_tag`: 10 (`AttachmentAdded`)
- `kind_payload`: `["log.txt", <sha256>, "text/plain"]`
- `sha256`: `000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f`

**Canonical CBOR preimage (hex)**

```
870150000102030405060708090a0b0c0d0e0f50101112131415161718191a1b1c1d1e1f1b0000018bcfe58740f60a83676c6f672e7478745820000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f6a746578742f706c61696e
```

**Expected `event_id` (BLAKE2b-256, hex)**

```
dc83946d33437f0b73d8b04c63f7b0b85b9e9a24e790fee3ca129d3d8b870749
```
