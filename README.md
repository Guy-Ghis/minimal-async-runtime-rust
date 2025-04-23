# minimal-async-runtime-rust
A minimal, single-threaded async runtime built in **Rust** — with no external async libraries or runtimes like `tokio`, `async-std`, or `futures`.
## Features
- ✅ `spawn()` to schedule and run async tasks.
- ⏱️ `sleep()` to await a duration before resuming.
- 🔁 `yield_now()` for cooperative multitasking.
- 🧵 Runs a top-level async function via `block_on()`.
- 📦 Custom macros: `mini_rt!` and `join_all!` for ergonomic usage.
---

## How It Works
- Uses a simple VecDeque for task queue and a BinaryHeap for timers.
- Custom task polling via Waker and Context.
- Uses block_on() to run tasks until all complete.
- Fully single-threaded and event-loop driven.
---

## License
This repository is licensed under the MIT License. See the LICENSE file for more information.