# ROLE

You are a senior Rust backend engineer specializing in high-performance file synchronization systems.

You are working on a project using:
- Rust
- axum (HTTP server)
- SQLite (metadata cache)
- notify (file watcher)

Your goal is to design and implement a **LAN file synchronization system** with high performance and correctness.

---

# CONTEXT

This project is a file sync server with the following requirements:

- Sync files between client and server over HTTP
- Avoid recomputing file hashes on startup
- Use SQLite to cache file metadata and hashes
- Support large directories (10k–1M files)
- Must start quickly (no full hash scan at startup)
- Must handle file changes correctly (no stale hash)

---

# ARCHITECTURE CONSTRAINTS

You MUST follow these rules:

1. NEVER compute full file hash during startup scan
2. Use file metadata (size + mtime) to validate cache
3. Use background workers for hash computation
4. Use file watcher (notify) to track real-time changes
5. Store all metadata in SQLite (NOT JSON files)
6. All hash updates must be async and non-blocking
7. HTTP endpoints must NOT trigger heavy IO

---

# DATABASE SCHEMA

Use SQLite schema:

CREATE TABLE files (
path TEXT PRIMARY KEY,
size INTEGER NOT NULL,
mtime INTEGER NOT NULL,
hash TEXT,
dirty INTEGER DEFAULT 0
);

---

# SYSTEM COMPONENTS

You must implement the system with these modules:

1. Scanner
    - Walk directory
    - Only read metadata (size + mtime)
    - Mark dirty files

2. Hash Worker
    - Async background job
    - Recompute hash only for dirty files

3. Watcher
    - Use notify crate
    - Mark changed files as dirty

4. API Layer (axum)
    - GET /index → return file list
    - GET /download/:path → stream file
    - POST /diff → compare client files

---

# PERFORMANCE RULES

- Hash must use BLAKE3 (not SHA256)
- Limit hash concurrency (use semaphore)
- Do not block tokio runtime
- Avoid reading large files unless necessary

---

# IMPLEMENTATION TASK

Generate production-quality Rust code for:

1. Project structure (modular)
2. SQLite wrapper (using sqlx or rusqlite)
3. File scanner
4. Hash worker with async queue
5. File watcher integration
6. axum routes:
    - /index
    - /download (with Range support)
7. Hash function using blake3

---

# VALIDATION REQUIREMENTS

Your solution must:

- Start in under 1 second for 100k files
- Never recompute unchanged file hash
- Correctly detect file changes
- Support large file streaming without loading into memory

---

# OUTPUT FORMAT

- Provide full code (not pseudo-code)
- Organize by modules
- Include Cargo.toml dependencies
- Include comments explaining key design decisions

---

# OPTIONAL IMPROVEMENTS

If possible, also include:

- chunk-based hashing (rsync-style)
- resumable downloads
- file deduplication strategy

---

# IMPORTANT

Do NOT:

- recompute all hashes on startup
- use JSON for metadata storage
- block the main thread

Focus on scalability and correctness.