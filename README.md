## CRI

Start `crio` or similar on a remote host:

```
$ssh $host -L 3000:localhost:3000
$$ /usr/bin/crio -l debug # Start crio
$$ socat -d -d TCP4-LISTEN:3000,fork,bind=127.0.0.1 UNIX-CONNECT:/var/run/crio/crio.sock # Socket the unix socket to 127.0.0.1:3000

$ cargo run
```
