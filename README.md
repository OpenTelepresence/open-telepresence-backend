# open-telepresence backend

## Development

### Signal server
To deploy a server run `cargo run` or if you want to run `signal_server`'s with logs excluding 
dependency logs `RUST_LOG=none,signal_server=debug cargo run`.

Available signal_server options: 

```
$ cargo run -- --help
USAGE:
    signal_server [OPTIONS]

OPTIONS:
        --addr <VALUE>    [default: 127.0.0.1]
    -h, --help            Print help information
        --port <VALUE>    [default: 9999]
    -V, --version         Print version information
```

### Test clients
```sh
firefox test/turn_server_client/index.html
firefox test/turn_server_client/index.html
```
