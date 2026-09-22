# examples/

Runnable examples of using The Familiar. Today the surface is the CLI; as the kernel
and an embeddable API land, `cargo` examples (`cargo run --example <name>`) will be
added here.

## CLI walkthrough — the service signal (Law I)

```sh
# isolate state in a throwaway dir
D=$(mktemp -d)

# a host-internal observation: the familiar sees nothing it serves
cargo run -p familiar-cli -- observe --actor host --action reports --object cpu_load --data-dir "$D"
cargo run -p familiar-cli -- service --data-dir "$D"
#   service signal 0.00 (...)  "continuation unjustified by service (Law I)"

# served-facing observations: now there are people to serve
cargo run -p familiar-cli -- observe --actor client --action requests --object status_report --data-dir "$D"
cargo run -p familiar-cli -- observe --actor support_team --action resolves --object customer_ticket --data-dir "$D"
cargo run -p familiar-cli -- service --data-dir "$D"
#   service signal 0.40 (2 of 3 ...; e.g. client)

cargo run -p familiar-cli -- observations --data-dir "$D"
rm -rf "$D"
```

## Using the sample log

```sh
D=$(mktemp -d); cp data/sample/observations.jsonl "$D"/
cargo run -p familiar-cli -- service --data-dir "$D"
rm -rf "$D"
```

See [../data/](../data/) for the record format and schema.
