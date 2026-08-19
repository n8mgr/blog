---
{
  "title": "A faster Reed-Solomon library for Rust and why we wrote our own",
  "description": "How SIMD helped us build a wire-compatible Reed-Solomon library that's up to 25× faster in native Rust and 8× faster in the browser.",
  "tags": ["rust", "sia"]
}
---

Reed-Solomon erasure coding is what keeps user data durable on Sia. Spread data across enough independent hosts and a file outlives any that drop offline or slow down. That redundancy costs far less than full replication for the same durability, and since you only need a subset of shards, the client can read from whichever hosts answer first and skip the stragglers. None of this is novel in the storage space: [Amazon S3](https://www.allthingsdistributed.com/2023/07/building-and-operating-a-pretty-big-storage-system.html), MinIO, Backblaze, Ceph, and others use erasure coding for the same basic reason. 

For most of Sia’s life, erasure coding was the server’s job while your devices just sent and fetched files, a centralized wrapper over a decentralized network. With Sia’s new architecture, the middleman is gone. Clients interact directly with storage providers. Now the client SDKs have to fetch, encrypt, erasure code, and fan out data themselves. They also have to hide that complexity from developers and run wherever users need them. That, unfortunately, includes a browser tab.

That left us with an interesting problem... *How fast can Reed-Solomon run in a browser?*

We needed an implementation that was fast on x86, ARM, and WebAssembly, and produced the exact same parity bytes as the data already on the network. There is one Rust crate that produces the same parity bytes and compiles to WebAssembly: `reed_solomon_erasure`. Without SIMD acceleration, its browser performance was not fast enough. 

So we decided to write a new one. That became `sia_reed_solomon`: Reed-Solomon erasure coding over GF(2^8), SIMD-accelerated (even in browsers), MIT licensed, and wire-compatible with the data already stored on Sia. 

`sia_reed_solomon` is a Rust port of Klaus Post's reedsolomon, the Go original. Sia has used it in production for eleven years. It’s the standard for erasure coding in Go and a common target of ports for other languages.

## The benchmarks

Setup: 10 data shards \+ 20 parity, 4 MiB shards, AWS \*.4xlarge spot runners (16 vCPU each). The shard layout is Sia's default. It keeps a file recoverable when untrusted hosts drop offline without overpaying for redundancy, and 4 MiB shards reflect the network's minimum chunk size. SIMD is the default build. "Scalar" is the `--no-default-features --features parallel` build, no SIMD but still multithreaded. The backend is picked at runtime. AVX2 and GFNI run on x86\_64, NEON on aarch64, scalar everywhere else.

Throughput comes from Criterion (Rust) and go test \-bench (Go), each timing only the operation, not shard setup or the random fill. 

Reconstruct throughput is reported per data slab (data\_shards × shard\_size), which makes it comparable to download throughput. It also means `reconstruct -1 data lost` looks very high, since only one shard is rebuilt while the rate is normalized to the full slab.

### Throughput across backends

<figure class="benchmark-chart">
<figcaption>Throughput across native backends</figcaption>
<ul class="benchmark-legend" aria-label="Series">
<li class="series-1">AVX2</li>
<li class="series-2">GFNI</li>
<li class="series-3">NEON</li>
<li class="series-4">Scalar</li>
</ul>
<div class="benchmark-group">
<p>encode</p>
<div class="benchmark-row"><progress class="series-1" max="65" value="22.5" aria-label="AVX2: 22.5 GiB/s">22.5 GiB/s</progress><span>22.5</span></div>
<div class="benchmark-row"><progress class="series-2" max="65" value="24.6" aria-label="GFNI: 24.6 GiB/s">24.6 GiB/s</progress><span>24.6</span></div>
<div class="benchmark-row"><progress class="series-3" max="65" value="28.3" aria-label="NEON: 28.3 GiB/s">28.3 GiB/s</progress><span>28.3</span></div>
<div class="benchmark-row"><progress class="series-4" max="65" value="4.2" aria-label="Scalar: 4.2 GiB/s">4.2 GiB/s</progress><span>4.2</span></div>
</div>
<div class="benchmark-group">
<p>reconstruct −1 data shard</p>
<div class="benchmark-row"><progress class="series-1" max="65" value="36.9" aria-label="AVX2: 36.9 GiB/s">36.9 GiB/s</progress><span>36.9</span></div>
<div class="benchmark-row"><progress class="series-2" max="65" value="32.8" aria-label="GFNI: 32.8 GiB/s">32.8 GiB/s</progress><span>32.8</span></div>
<div class="benchmark-row"><progress class="series-3" max="65" value="58.7" aria-label="NEON: 58.7 GiB/s">58.7 GiB/s</progress><span>58.7</span></div>
<div class="benchmark-row"><progress class="series-4" max="65" value="14.9" aria-label="Scalar: 14.9 GiB/s">14.9 GiB/s</progress><span>14.9</span></div>
</div>
<div class="benchmark-group">
<p>reconstruct −10 data shards</p>
<div class="benchmark-row"><progress class="series-1" max="65" value="7.2" aria-label="AVX2: 7.2 GiB/s">7.2 GiB/s</progress><span>7.2</span></div>
<div class="benchmark-row"><progress class="series-2" max="65" value="8.2" aria-label="GFNI: 8.2 GiB/s">8.2 GiB/s</progress><span>8.2</span></div>
<div class="benchmark-row"><progress class="series-3" max="65" value="10.7" aria-label="NEON: 10.7 GiB/s">10.7 GiB/s</progress><span>10.7</span></div>
<div class="benchmark-row"><progress class="series-4" max="65" value="2.4" aria-label="Scalar: 2.4 GiB/s">2.4 GiB/s</progress><span>2.4</span></div>
</div>
<div class="benchmark-scale"><span>0</span><span>65</span></div>
</figure>

### Against the field

Klaus's Go is the baseline. It isn't a Rust crate we could pull into the SDK, but it's the high bar we measure against. `reed_solomon_erasure` (built with simd-accel) is the one actual Rust alternative for our usecase.

On c5.4xlarge (AVX2):

<figure class="benchmark-chart">
<figcaption>AVX2 throughput</figcaption>
<ul class="benchmark-legend" aria-label="Series">
<li class="series-1">sia_reed_solomon</li>
<li class="series-2">klauspost (Go)</li>
<li class="series-3">reed_solomon_erasure</li>
</ul>
<div class="benchmark-group">
<p>encode</p>
<div class="benchmark-row"><progress class="series-1" max="42" value="22.5" aria-label="sia_reed_solomon: 22.5 GiB/s">22.5 GiB/s</progress><span>22.5</span></div>
<div class="benchmark-row"><progress class="series-2" max="42" value="37.2" aria-label="klauspost: 37.2 GiB/s">37.2 GiB/s</progress><span>37.2</span></div>
<div class="benchmark-row"><progress class="series-3" max="42" value="1.1" aria-label="reed_solomon_erasure: 1.1 GiB/s">1.1 GiB/s</progress><span>1.1</span></div>
</div>
<div class="benchmark-group">
<p>reconstruct −1 data shard</p>
<div class="benchmark-row"><progress class="series-1" max="42" value="36.9" aria-label="sia_reed_solomon: 36.9 GiB/s">36.9 GiB/s</progress><span>36.9</span></div>
<div class="benchmark-row"><progress class="series-2" max="42" value="30.3" aria-label="klauspost: 30.3 GiB/s">30.3 GiB/s</progress><span>30.3</span></div>
<div class="benchmark-row"><progress class="series-3" max="42" value="5.7" aria-label="reed_solomon_erasure: 5.7 GiB/s">5.7 GiB/s</progress><span>5.7</span></div>
</div>
<div class="benchmark-group">
<p>reconstruct −10 data shards</p>
<div class="benchmark-row"><progress class="series-1" max="42" value="7.2" aria-label="sia_reed_solomon: 7.2 GiB/s">7.2 GiB/s</progress><span>7.2</span></div>
<div class="benchmark-row"><progress class="series-2" max="42" value="3.8" aria-label="klauspost: 3.8 GiB/s">3.8 GiB/s</progress><span>3.8</span></div>
<div class="benchmark-row"><progress class="series-3" max="42" value="0.583" aria-label="reed_solomon_erasure: 597 MiB/s">597 MiB/s</progress><span class="unit-mib">597</span></div>
</div>
<div class="benchmark-scale"><span>0</span><span>42</span></div>
</figure>

On c7i.4xlarge (GFNI):

<figure class="benchmark-chart">
<figcaption>GFNI throughput</figcaption>
<ul class="benchmark-legend" aria-label="Series">
<li class="series-1">sia_reed_solomon</li>
<li class="series-2">klauspost (Go)</li>
<li class="series-3">reed_solomon_erasure</li>
</ul>
<div class="benchmark-group">
<p>encode</p>
<div class="benchmark-row"><progress class="series-1" max="60" value="24.6" aria-label="sia_reed_solomon: 24.6 GiB/s">24.6 GiB/s</progress><span>24.6</span></div>
<div class="benchmark-row"><progress class="series-2" max="60" value="54.7" aria-label="klauspost: 54.7 GiB/s">54.7 GiB/s</progress><span>54.7</span></div>
<div class="benchmark-row"><progress class="series-3" max="60" value="0.977" aria-label="reed_solomon_erasure: 1000 MiB/s">1000 MiB/s</progress><span class="unit-mib">1000</span></div>
</div>
<div class="benchmark-group">
<p>reconstruct −1 data shard</p>
<div class="benchmark-row"><progress class="series-1" max="60" value="32.8" aria-label="sia_reed_solomon: 32.8 GiB/s">32.8 GiB/s</progress><span>32.8</span></div>
<div class="benchmark-row"><progress class="series-2" max="60" value="21.9" aria-label="klauspost: 21.9 GiB/s">21.9 GiB/s</progress><span>21.9</span></div>
<div class="benchmark-row"><progress class="series-3" max="60" value="5.5" aria-label="reed_solomon_erasure: 5.5 GiB/s">5.5 GiB/s</progress><span>5.5</span></div>
</div>
<div class="benchmark-group">
<p>reconstruct −10 data shards</p>
<div class="benchmark-row"><progress class="series-1" max="60" value="8.2" aria-label="sia_reed_solomon: 8.2 GiB/s">8.2 GiB/s</progress><span>8.2</span></div>
<div class="benchmark-row"><progress class="series-2" max="60" value="6.2" aria-label="klauspost: 6.2 GiB/s">6.2 GiB/s</progress><span>6.2</span></div>
<div class="benchmark-row"><progress class="series-3" max="60" value="0.576" aria-label="reed_solomon_erasure: 590 MiB/s">590 MiB/s</progress><span class="unit-mib">590</span></div>
</div>
<div class="benchmark-scale"><span>0</span><span>60</span></div>
</figure>

On c7g.4xlarge (NEON, Graviton 3):

<figure class="benchmark-chart">
<figcaption>NEON throughput</figcaption>
<ul class="benchmark-legend" aria-label="Series">
<li class="series-1">sia_reed_solomon</li>
<li class="series-2">klauspost (Go)</li>
<li class="series-3">reed_solomon_erasure</li>
</ul>
<div class="benchmark-group">
<p>encode</p>
<div class="benchmark-row"><progress class="series-1" max="85" value="28.3" aria-label="sia_reed_solomon: 28.3 GiB/s">28.3 GiB/s</progress><span>28.3</span></div>
<div class="benchmark-row"><progress class="series-2" max="85" value="48.9" aria-label="klauspost: 48.9 GiB/s">48.9 GiB/s</progress><span>48.9</span></div>
<div class="benchmark-row"><progress class="series-3" max="85" value="1.1" aria-label="reed_solomon_erasure: 1.1 GiB/s">1.1 GiB/s</progress><span>1.1</span></div>
</div>
<div class="benchmark-group">
<p>reconstruct −1 data shard</p>
<div class="benchmark-row"><progress class="series-1" max="85" value="58.7" aria-label="sia_reed_solomon: 58.7 GiB/s">58.7 GiB/s</progress><span>58.7</span></div>
<div class="benchmark-row"><progress class="series-2" max="85" value="75.9" aria-label="klauspost: 75.9 GiB/s">75.9 GiB/s</progress><span>75.9</span></div>
<div class="benchmark-row"><progress class="series-3" max="85" value="5.9" aria-label="reed_solomon_erasure: 5.9 GiB/s">5.9 GiB/s</progress><span>5.9</span></div>
</div>
<div class="benchmark-group">
<p>reconstruct −10 data shards</p>
<div class="benchmark-row"><progress class="series-1" max="85" value="10.7" aria-label="sia_reed_solomon: 10.7 GiB/s">10.7 GiB/s</progress><span>10.7</span></div>
<div class="benchmark-row"><progress class="series-2" max="85" value="18.5" aria-label="klauspost: 18.5 GiB/s">18.5 GiB/s</progress><span>18.5</span></div>
<div class="benchmark-row"><progress class="series-3" max="85" value="0.579" aria-label="reed_solomon_erasure: 593 MiB/s">593 MiB/s</progress><span class="unit-mib">593</span></div>
</div>
<div class="benchmark-scale"><span>0</span><span>85</span></div>
</figure>

On native targets we're 6x to over 25x faster than `reed_solomon_erasure` in these benches. 

Klaus's Go still wins `encode` on every machine, and wins outright on NEON. His kernels are generated assembly; we use Rust SIMD intrinsics. But on x86 backends, we beat it on reconstruction, and even on heavy reconstructions we stay ahead (8.2 vs 6.2 GiB/s on GFNI). Reconstruct is the download path: every time a client reads data and a shard is missing, it has to rebuild the original from parity.

The Rust benches and the Go bench harness are both in the repo under `comparisons/` if you want to reproduce any of this.

## Where the speed comes from

Erasure coding is mostly one operation repeated: multiply a shard by a constant in GF(2^8) and XOR it into an accumulator. Addition in the field is a plain XOR, and every byte is independent of the next, so the same work runs across a whole shard with nothing to coordinate between bytes. Scalar code walks a shard one byte at a time while SIMD processes 16 or 32 bytes per instruction.

That multiply is normally a 256-entry table lookup. The split-table method breaks each byte into two 4-bit nibbles, turning it into two 16-entry lookups XORed together. A 16-entry lookup is exactly what a SIMD shuffle does, 16 lanes in one instruction with the table in a register. So the multiply collapses to two shuffles and an XOR. GFNI skips it: `GF2P8AFFINEQB` applies the constant’s 8x8 GF(2) matrix to each byte in one instruction, no tables.

## WASM-SIMD

*That’s great, but wasn’t the point to be faster in a browser?*

Luckily, [WASM-SIMD](https://caniuse.com/wasm-simd) is supported in all browser targets we require. It adds a portable 128-bit vector type and a set of lane-wise instructions over it, including the byte shuffle (`u8x16_swizzle`) split-table depends on. The browser’s engine lowers these to native SIMD instructions so the bytecode reaches the CPU’s vector unit. With that, the browser runs the same shuffle-based multiply as the native SIMD backends: 16 bytes per instruction with the lookups in registers rather than one byte and a dependent load per step.  

As far as we know, no other Rust Reed-Solomon crate has a WASM-SIMD backend. It’s a niche target, but important for our SDKs.

<figure class="benchmark-chart">
<figcaption>WASM-SIMD throughput</figcaption>
<ul class="benchmark-legend" aria-label="Series">
<li class="series-1">sia_reed_solomon</li>
<li class="series-2">reed_solomon_erasure</li>
</ul>
<div class="benchmark-group">
<p>encode</p>
<div class="benchmark-row"><progress class="series-1" max="1.8" value="1.6" aria-label="sia_reed_solomon: 1.6 GiB/s">1.6 GiB/s</progress><span>1.6</span></div>
<div class="benchmark-row"><progress class="series-2" max="1.8" value="0.204" aria-label="reed_solomon_erasure: 209 MiB/s">209 MiB/s</progress><span class="unit-mib">209</span></div>
</div>
<div class="benchmark-group">
<p>reconstruct −1 data shard</p>
<div class="benchmark-row"><progress class="series-1" max="1.8" value="1.6" aria-label="sia_reed_solomon: 1.6 GiB/s">1.6 GiB/s</progress><span>1.6</span></div>
<div class="benchmark-row"><progress class="series-2" max="1.8" value="0.823" aria-label="reed_solomon_erasure: 843 MiB/s">843 MiB/s</progress><span class="unit-mib">843</span></div>
</div>
<div class="benchmark-group">
<p>reconstruct −10 data shards</p>
<div class="benchmark-row"><progress class="series-1" max="1.8" value="0.686" aria-label="sia_reed_solomon: 702 MiB/s">702 MiB/s</progress><span class="unit-mib">702</span></div>
<div class="benchmark-row"><progress class="series-2" max="1.8" value="0.128" aria-label="reed_solomon_erasure: 131 MiB/s">131 MiB/s</progress><span class="unit-mib">131</span></div>
</div>
<div class="benchmark-scale"><span>0</span><span>1.8</span></div>
</figure>

That's roughly 8x faster on encode. The reconstruct gap depends on how much is missing: with one shard gone it's under 2x, but in the worst case, all 10 data shards lost, reed\_solomon\_erasure is over 5x slower.

In the browser, the library runs on the user's hardware with no bigger machine to offload to. On upload, the client encrypts, encodes, then sends the shards to hosts. The first two are CPU-bound, and the send is network-bound, so the faster those finish, the sooner each chunk hits the wire. Download is the same in reverse, fetching shards over the network, then reconstructing and decrypting. SIMD keeps `encode` and `reconstruct` from becoming the bottleneck.

## Get it

It's on [crates.io](https://crates.io/crates/sia_reed_solomon), the API is on [docs.rs](https://docs.rs/sia_reed_solomon), and the source is on [GitHub](https://github.com/SiaFoundation/reed_solomon_rs). MIT licensed.

```rust
use sia_reed_solomon::ReedSolomon;

let rs = ReedSolomon::new(10, 20)?; // 10 data + 20 parity
rs.encode(&mut shards)?;
assert!(rs.verify(&shards)?);

let mut shards: Vec<Option<Vec<u8>>> = shards.into_iter().map(Some).collect(); 
shards[3] = None; 
rs.reconstruct(&mut shards)?;
```

If you're building anything that needs fast, browser-capable erasure coding in Rust, give it a try.
