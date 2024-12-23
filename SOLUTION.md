# Solution Documentation
This document is divided into two parts:

1. Single-threaded Server Implementation
2. Multi-threaded Server Implementation
---

## Part 1: Single-threaded Server Implementation

## Problem Analysis and Initial Observations

When running tests before making any changes, they failed with an error indicating that the address was already in use. However, running each test individually succeeded without issue.

![Screenshot from 2024-12-24 01-40-23](https://github.com/user-attachments/assets/b44d709d-bd1f-4142-a08e-f654629d9296)

**Root Cause:**
The issue stemmed from the port being held by the operating system for a brief period after a test finished, preventing the next test from binding to the same port.

**Solution:**
To address this, changes were made to allow port reuse and improve server reliability.

![image](https://github.com/user-attachments/assets/30cd960c-61f2-4bec-ba12-77ed1668e46f)

---

## Problem Analysis and Changes

### 1. Initialization of `TcpListener` with `TcpBuilder`

**Old Code:**
```rust
let listener = TcpListener::bind(addr)?;
```

**New Code:**
```rust
let listener = TcpBuilder::new_v4()?
    .reuse_address(true)?
    .reuse_port(true)?
    .bind(addr)?
    .listen(42)?;
```

**Reason for Change:**
- `reuse_address(true)` ensures the server can restart quickly after a crash without waiting for the OS to release the port.
- `reuse_port(true)` allows multiple instances of the server to bind to the same port in specific scenarios.
- `listen(42)` sets the backlog size to 42, improving connection handling.

---

### 2. Simplified Client Handling Loop

**Old Code:**
```rust
while self.is_running.load(Ordering::SeqCst) {
    if let Err(e) = client.handle() {
        error!("Error handling client: {}", e);
        break;
    }
}
```

**New Code:**
![image](https://github.com/user-attachments/assets/b38e4a49-7edf-4720-a0eb-38c1c160ec21)

- Removed the redundant loop, allowing each client connection to be handled independently.
- Added an info log for successful client handling to improve monitoring.

---

### 3. Improved Buffer Management

**Old Code:**
```rust
let mut buffer = [0; 512];
```

**New Code:**
```rust
let mut buffer = [0; 1024];
```

**Reason for Change:**
- Increased buffer size to handle larger messages without truncation.

---

### 4. Updated Dependencies

Added the `net2` crate in `Cargo.toml`:
```toml
[dependencies]
net2 = "0.2.33"
log = "0.4.2"
prost = "0.13.4"
prost-types = "0.13.4"
```

---

## Summary of Fixes

- Enhanced socket initialization for reliability and flexibility.
- Increased buffer size for larger message handling.
- Improved logging for debugging and traceability.
- Refined client handling for clarity and correctness.
- Standardized error handling across methods.

---
## Part 2: Multi-threaded Server Implementation
![image](https://github.com/user-attachments/assets/0a9ace5d-4480-4ece-8d8d-33eb80cf6c7c)

### Key Changes from Single-Threaded to Multi-Threaded Implementation

1. **Introduction of Threads for Concurrency**
   - Used the `std::thread` module to spawn new threads, allowing the server to handle multiple clients concurrently.

2. **Synchronization Primitives**
   - Used `Mutex` and `RwLock` from the `parking_lot` crate to manage shared resources.
   - Used `Arc` (Atomic Reference Counting) to share ownership of data safely across threads.

3. **Shared State Management**
   - Employed `Arc` and `Mutex` for managing shared resources like connected clients or state data, ensuring thread-safe access.

4. **Improved Error Handling**
   - Enhanced logging and error handling for debugging multi-threaded issues.
   - Used the `log` crate for structured logging.

5. **Graceful Shutdown Support**
   - Introduced an `AtomicBool` with `Arc` for controlled server shutdown, ensuring all threads terminate gracefully.

---
## Test Results

### Passed Tests
The following tests passed successfully:
- `test_client_connection`
- `test_client_echo_message`
- `test_multiple_echo_messages`
- `test_multiple_clients`

### Pending Test
- The fifth test requires further investigation. Potential issues may include thread synchronization or resource handling.

---

## Command Output

```sh
$ cargo test
Running unittests src/lib.rs (target/debug/deps/embedded_recruitment_task-99bad2b7d38176ae)
running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

Running tests/client.rs (target/debug/deps/client-6badbdfadfef8379)
running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

Running tests/client_test.rs (target/debug/deps/client_test-615e9209661642e2)
running 5 tests

test test_client_connection ... ok
test test_client_echo_message ... ok
test test_multiple_echo_messages ... ok
test test_multiple_clients ... ok

Test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.10s
```

